use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use render_core::{
    LodLevel, Mat4, MaterialHandle, MeshHandle, PreparedScene, RasterMesh, SnapshotId, Transform,
};
use wgpu::util::DeviceExt;

use crate::renderer::{color_pipeline, GpuVertex, ResourceStats};
use crate::{GpuContext, GpuError};

struct Geometry {
    source: Arc<RasterMesh>,
    normals: Option<wgpu::Buffer>,
    normal_bytes: u64,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    opaque: bool,
    vertex_bytes: u64,
    index_bytes: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct InstanceSource {
    key: (MeshHandle, LodLevel),
    id: SnapshotId,
    geometry: usize,
    material: MaterialHandle,
    transform: Transform,
}

struct Batch {
    key: (MeshHandle, LodLevel),
    geometry: usize,
    material: MaterialHandle,
    instances: Range<u32>,
}

pub(crate) struct InstancedScene {
    pipeline: wgpu::RenderPipeline,
    lighting: Option<crate::lighting::LightingPass>,
    format: wgpu::TextureFormat,
    cache: BTreeMap<(MeshHandle, LodLevel), Vec<usize>>,
    geometries: Vec<Geometry>,
    sources: Vec<InstanceSource>,
    next_sources: Vec<InstanceSource>,
    batches: Vec<Batch>,
    instances: wgpu::Buffer,
    capacity: usize,
    data: Vec<[f32; 16]>,
    next_data: Vec<[f32; 16]>,
    force_write: bool,
    dirty: bool,
    last_vp: Option<Mat4>,
    reallocations: u64,
    bytes_written: u64,
}

impl InstancedScene {
    pub(crate) fn new(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
    ) -> Self {
        Self {
            pipeline: color_pipeline(gpu, format, layout, shader, true),
            lighting: None,
            format,
            cache: BTreeMap::new(),
            geometries: Vec::new(),
            sources: Vec::new(),
            next_sources: Vec::new(),
            batches: Vec::new(),
            instances: instance_buffer(gpu, 1),
            capacity: 1,
            data: Vec::new(),
            next_data: Vec::new(),
            force_write: true,
            dirty: true,
            last_vp: None,
            reallocations: 1,
            bytes_written: 0,
        }
    }

    fn geometry(
        &mut self,
        gpu: &GpuContext,
        key: (MeshHandle, LodLevel),
        mesh: &Arc<RasterMesh>,
    ) -> Result<usize, GpuError> {
        if let Some(entries) = self.cache.get(&key) {
            for &index in entries {
                let source = &self.geometries[index].source;
                if Arc::ptr_eq(source, mesh) || source.as_ref() == mesh.as_ref() {
                    return Ok(index);
                }
            }
        }
        if let Some(index) = self.geometries.iter().position(|geometry| {
            Arc::ptr_eq(&geometry.source, mesh) || geometry.source.as_ref() == mesh.as_ref()
        }) {
            self.cache.entry(key).or_default().push(index);
            return Ok(index);
        }
        let index_count =
            u32::try_from(mesh.indices.len()).map_err(|e| GpuError::InvalidInput(e.to_string()))?;
        let vertex_bytes = (mesh.vertices.len() as u64 * 28).max(4);
        let index_bytes = (mesh.indices.len() as u64 * 4).max(4);
        if !mesh.indices.len().is_multiple_of(3)
            || mesh
                .vertices
                .iter()
                .any(|v| !v.position.is_finite() || !v.color.is_finite())
            || mesh
                .indices
                .iter()
                .any(|&i| i as usize >= mesh.vertices.len())
            || vertex_bytes.max(index_bytes) > gpu.device.limits().max_buffer_size
        {
            return Err(GpuError::InvalidInput("invalid cached geometry".into()));
        }
        let vertices: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| GpuVertex {
                position: v.position.to_array(),
                color: v.color.to_array(),
            })
            .collect();
        let create = |label, contents: &[u8], usage| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: if contents.is_empty() {
                        &[0; 4]
                    } else {
                        contents
                    },
                    usage,
                })
        };
        let geometry = Geometry {
            source: mesh.clone(),
            normals: None,
            normal_bytes: 0,
            vertices: create(
                "cached mesh vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: create(
                "cached mesh indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count,
            opaque: mesh.vertices.iter().all(|v| v.color.w == 1.0),
            vertex_bytes,
            index_bytes,
        };
        let index = self.geometries.len();
        self.geometries.push(geometry);
        self.cache.entry(key).or_default().push(index);
        Ok(index)
    }

    pub(crate) fn sync(
        &mut self,
        gpu: &GpuContext,
        scene: &PreparedScene,
    ) -> Result<bool, GpuError> {
        gpu.check()?;
        self.next_sources.clear();
        self.batches.clear();
        for draw in scene.draws() {
            let key = (draw.node.mesh, draw.lod);
            let geometry = self.geometry(gpu, key, &draw.mesh)?;
            let first = u32::try_from(self.next_sources.len())
                .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
            let end = first
                .checked_add(1)
                .ok_or_else(|| GpuError::InvalidInput("instance count overflow".into()))?;
            let unchanged = self
                .sources
                .get(first as usize)
                .is_some_and(|source| source.transform == draw.node.transform);
            if !unchanged && !draw.node.transform.matrix().is_finite() {
                return Err(GpuError::InvalidInput(
                    "non-finite instance transform".into(),
                ));
            }
            self.next_sources.push(InstanceSource {
                key,
                id: draw.node.entity,
                geometry,
                material: draw.node.material,
                transform: draw.node.transform,
            });
            if self.geometries[geometry].index_count == 0 {
                continue;
            }
            if let Some(last) = self.batches.last_mut().filter(|last| {
                self.geometries[geometry].opaque
                    && (scene.lighting.mode == render_core::LightingMode::Unlit
                        || scene.material(draw.node.material).base_color.w == 1.0)
                    && last.key == key
                    && last.geometry == geometry
                    && last.material == draw.node.material
                    && last.instances.end == first
            }) {
                last.instances.end = end;
            } else {
                self.batches.push(Batch {
                    key,
                    geometry,
                    material: draw.node.material,
                    instances: first..end,
                });
            }
        }
        let count = self.next_sources.len().max(1);
        if count > self.capacity {
            let capacity = count
                .checked_next_power_of_two()
                .ok_or_else(|| GpuError::InvalidInput("instance capacity overflow".into()))?;
            if capacity as u64 * 64 > gpu.device.limits().max_buffer_size {
                return Err(GpuError::InvalidInput(
                    "instance buffer exceeds device limit".into(),
                ));
            }
            self.instances = instance_buffer(gpu, capacity);
            self.capacity = capacity;
            self.force_write = true;
            self.reallocations += 1;
        }
        if scene.lighting.mode != render_core::LightingMode::Unlit {
            if self.lighting.is_none() {
                self.lighting = Some(crate::lighting::LightingPass::new(gpu, self.format)?);
            }
            for source in &self.next_sources {
                let geometry = &mut self.geometries[source.geometry];
                if geometry.normals.is_none() {
                    let normals: Vec<_> = geometry
                        .source
                        .resolved_normals()
                        .into_iter()
                        .map(|n| n.to_array())
                        .collect();
                    geometry.normal_bytes = (normals.len() as u64 * 12).max(4);
                    if geometry.normal_bytes > gpu.device.limits().max_buffer_size {
                        return Err(GpuError::InvalidInput(
                            "normal buffer exceeds device limit".into(),
                        ));
                    }
                    geometry.normals = Some(gpu.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("cached mesh normals"),
                            contents: if normals.is_empty() {
                                &[0; 4]
                            } else {
                                bytemuck::cast_slice(&normals)
                            },
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                }
            }
            self.lighting
                .as_mut()
                .expect("initialized lighting")
                .sync(gpu, scene)?;
        } else if let Some(lighting) = &mut self.lighting {
            lighting.active = false;
        }
        let changed = self.sources != self.next_sources;
        self.dirty |= changed;
        std::mem::swap(&mut self.sources, &mut self.next_sources);
        gpu.check()?;
        Ok(changed)
    }

    pub(crate) fn set_transform(
        &mut self,
        index: usize,
        transform: Transform,
    ) -> Result<(), GpuError> {
        let source = self
            .sources
            .get_mut(index)
            .ok_or_else(|| GpuError::InvalidInput("draw index out of range".into()))?;
        self.dirty |= source.transform != transform;
        source.transform = transform;
        if let Some(lighting) = &mut self.lighting {
            lighting.set_transform(index, transform);
        }
        Ok(())
    }

    pub(crate) fn update(&mut self, gpu: &GpuContext, vp: Mat4) -> Result<(), GpuError> {
        if let Some(lighting) = self.lighting.as_mut().filter(|lighting| lighting.active) {
            return lighting.update(gpu, vp);
        }
        if !self.dirty && !self.force_write && self.last_vp == Some(vp) {
            return Ok(());
        }
        self.next_data.clear();
        for source in &self.sources {
            let mvp = vp * source.transform.matrix();
            if !mvp.is_finite() {
                return Err(GpuError::InvalidInput("non-finite instance MVP".into()));
            }
            self.next_data.push(mvp.to_cols_array());
        }
        let first = self
            .next_data
            .iter()
            .enumerate()
            .position(|(i, m)| self.force_write || self.data.get(i) != Some(m));
        if let Some(first) = first {
            let last = self
                .next_data
                .iter()
                .enumerate()
                .rposition(|(i, m)| self.force_write || self.data.get(i) != Some(m))
                .expect("dirty instance");
            let bytes = bytemuck::cast_slice(&self.next_data[first..=last]);
            gpu.queue
                .write_buffer(&self.instances, first as u64 * 64, bytes);
            self.bytes_written += bytes.len() as u64;
        }
        std::mem::swap(&mut self.data, &mut self.next_data);
        self.force_write = false;
        self.dirty = false;
        self.last_vp = Some(vp);
        Ok(())
    }

    pub(crate) fn encode(&self, pass: &mut wgpu::RenderPass<'_>) {
        let lighting = self.lighting.as_ref().filter(|lighting| lighting.active);
        if let Some(lighting) = lighting {
            lighting.bind(pass);
        } else {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(1, self.instances.slice(..));
        }
        for batch in &self.batches {
            let geometry = &self.geometries[batch.geometry];
            pass.set_vertex_buffer(0, geometry.vertices.slice(..));
            if lighting.is_some() {
                pass.set_vertex_buffer(
                    2,
                    geometry.normals.as_ref().expect("cached normals").slice(..),
                );
            }
            pass.set_index_buffer(geometry.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..geometry.index_count, 0, batch.instances.clone());
        }
    }

    pub(crate) fn resources(&self, base: ResourceStats) -> ResourceStats {
        ResourceStats {
            vertex_bytes: base.vertex_bytes
                + self
                    .geometries
                    .iter()
                    .map(|g| g.vertex_bytes + g.normal_bytes)
                    .sum::<u64>(),
            index_bytes: base.index_bytes
                + self.geometries.iter().map(|g| g.index_bytes).sum::<u64>(),
            uniform_bytes: base.uniform_bytes + if self.lighting.is_some() { 288 } else { 0 },
            draw_count: self.batches.len(),
            instance_count: self.sources.len(),
            instance_bytes: self.capacity as u64 * 64
                + self
                    .lighting
                    .as_ref()
                    .map_or(0, |lighting| lighting.instance_bytes()),
            geometry_uploads: base.geometry_uploads + self.geometries.len() as u64,
            buffer_rebuilds: base.buffer_rebuilds
                + self.geometries.len() as u64
                + self.reallocations
                + self
                    .geometries
                    .iter()
                    .filter(|geometry| geometry.normals.is_some())
                    .count() as u64
                + self
                    .lighting
                    .as_ref()
                    .map_or(0, |lighting| lighting.reallocations),
            dynamic_bytes_written: base.dynamic_bytes_written
                + self.bytes_written
                + self
                    .lighting
                    .as_ref()
                    .map_or(0, |lighting| lighting.bytes_written),
        }
    }
}

fn instance_buffer(gpu: &GpuContext, capacity: usize) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("reusable MVP instances"),
        size: capacity as u64 * 64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

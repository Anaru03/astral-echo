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
        Ok(())
    }

    pub(crate) fn update(&mut self, gpu: &GpuContext, vp: Mat4) -> Result<(), GpuError> {
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
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(1, self.instances.slice(..));
        for batch in &self.batches {
            let geometry = &self.geometries[batch.geometry];
            pass.set_vertex_buffer(0, geometry.vertices.slice(..));
            pass.set_index_buffer(geometry.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..geometry.index_count, 0, batch.instances.clone());
        }
    }

    pub(crate) fn resources(&self, base: ResourceStats) -> ResourceStats {
        ResourceStats {
            vertex_bytes: base.vertex_bytes
                + self.geometries.iter().map(|g| g.vertex_bytes).sum::<u64>(),
            index_bytes: base.index_bytes
                + self.geometries.iter().map(|g| g.index_bytes).sum::<u64>(),
            uniform_bytes: base.uniform_bytes,
            draw_count: self.batches.len(),
            instance_count: self.sources.len(),
            instance_bytes: self.capacity as u64 * 64,
            geometry_uploads: base.geometry_uploads + self.geometries.len() as u64,
            buffer_rebuilds: base.buffer_rebuilds
                + self.geometries.len() as u64
                + self.reallocations,
            dynamic_bytes_written: base.dynamic_bytes_written + self.bytes_written,
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

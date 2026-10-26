use std::num::NonZeroU64;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use render_core::{
    game_over_layout, Camera, Mat4, MaterialHandle, MeshHandle, PreparedScene, RasterMesh,
    RasterVertex, SnapshotId, SnapshotScene, Transform, Vec3, Vec4, GAME_OVER_GLYPHS,
};
use wgpu::util::DeviceExt;

use crate::{block_on, GpuContext, GpuError};

pub const CLEAR_RGBA: [u8; 4] = [8, 10, 18, 255];
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

pub struct PackedScene {
    pub vertices: Vec<GpuVertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<(Range<u32>, Transform)>,
}

pub fn game_over_overlay_scene(width: u32, height: u32) -> Vec<(RasterMesh, Transform)> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let (left, top, scale) = game_over_layout(width, height);
    let color = Vec4::new(1.0, 0.25, 0.19, 1.0);
    let to_ndc = |x: u32, y: u32| {
        Vec3::new(
            (x as f32 / width as f32) * 2.0 - 1.0,
            1.0 - (y as f32 / height as f32) * 2.0,
            0.0,
        )
    };
    let mut result = Vec::new();
    for (glyph_index, glyph) in GAME_OVER_GLYPHS.iter().enumerate() {
        let word_gap = u32::from(glyph_index >= 4) * 2 * scale;
        let glyph_left = left + glyph_index as u32 * 6 * scale + word_gap;
        for (row, bits) in glyph.iter().copied().enumerate() {
            for column in 0..5u32 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x0 = glyph_left + column * scale;
                let y0 = top + row as u32 * scale;
                let x1 = x0 + scale;
                let y1 = y0 + scale;
                result.push((
                    RasterMesh::new(
                        vec![
                            RasterVertex::new(to_ndc(x0, y1), color),
                            RasterVertex::new(to_ndc(x1, y1), color),
                            RasterVertex::new(to_ndc(x1, y0), color),
                            RasterVertex::new(to_ndc(x0, y0), color),
                        ],
                        vec![0, 1, 2, 0, 2, 3],
                    ),
                    Transform::IDENTITY,
                ));
            }
        }
    }
    result
}

impl PackedScene {
    pub fn new(scene: &[(RasterMesh, Transform)]) -> Result<Self, GpuError> {
        let mut result = Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            draws: Vec::new(),
        };
        for (mesh, transform) in scene {
            if !mesh.indices.len().is_multiple_of(3) || !transform.matrix().is_finite() {
                return Err(GpuError::InvalidInput(
                    "invalid triangle list or transform".into(),
                ));
            }
            let base = u32::try_from(result.vertices.len())
                .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
            let first = u32::try_from(result.indices.len())
                .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
            for vertex in &mesh.vertices {
                if !vertex.position.is_finite() || !vertex.color.is_finite() {
                    return Err(GpuError::InvalidInput("non-finite vertex".into()));
                }
                result.vertices.push(GpuVertex {
                    position: vertex.position.to_array(),
                    color: vertex.color.to_array(),
                });
            }
            for &index in &mesh.indices {
                if index as usize >= mesh.vertices.len() {
                    return Err(GpuError::InvalidInput("vertex index out of range".into()));
                }
                result.indices.push(
                    base.checked_add(index)
                        .ok_or_else(|| GpuError::InvalidInput("vertex index overflow".into()))?,
                );
            }
            let end = u32::try_from(result.indices.len())
                .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
            if first != end {
                result.draws.push((first..end, *transform));
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResourceStats {
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub uniform_bytes: u64,
    pub draw_count: usize,
    pub instance_count: usize,
    pub instance_bytes: u64,
    pub geometry_uploads: u64,
    pub buffer_rebuilds: u64,
    pub dynamic_bytes_written: u64,
    pub shadow_map_bytes: u64,
    pub shadow_map_allocations: u64,
    pub shadow_draw_count: usize,
}

pub struct GpuRenderer {
    pub(crate) gpu: Arc<GpuContext>,
    pipeline: wgpu::RenderPipeline,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    instanced: Option<crate::instancing::InstancedScene>,
    timer: Option<crate::timing::GpuTimer>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    draws: Vec<(Range<u32>, Transform)>,
    uniform_data: Vec<u8>,
    stride: u32,
    format: wgpu::TextureFormat,
    stats: ResourceStats,
    prepared_meshes: Vec<Arc<RasterMesh>>,
    prepared_keys: Vec<(
        SnapshotId,
        MeshHandle,
        MaterialHandle,
        render_core::LodLevel,
    )>,
    snapshot_keys: Option<Vec<(SnapshotId, MeshHandle, MaterialHandle)>>,
}

impl GpuRenderer {
    pub fn new(
        gpu: Arc<GpuContext>,
        format: wgpu::TextureFormat,
        scene: &[(RasterMesh, Transform)],
    ) -> Result<Self, GpuError> {
        block_on(Self::new_async(gpu, format, scene))
    }

    pub(crate) async fn new_async(
        gpu: Arc<GpuContext>,
        format: wgpu::TextureFormat,
        scene: &[(RasterMesh, Transform)],
    ) -> Result<Self, GpuError> {
        gpu.check()?;
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba16Float
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err(GpuError::InvalidInput("unsupported color format".into()));
        }
        let packed = PackedScene::new(scene)?;
        let stride = gpu
            .device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(64);
        let stats = ResourceStats {
            vertex_bytes: (packed.vertices.len() * std::mem::size_of::<GpuVertex>()).max(4) as u64,
            index_bytes: (packed.indices.len() * 4).max(4) as u64,
            uniform_bytes: packed.draws.len().max(1) as u64 * u64::from(stride),
            draw_count: packed.draws.len(),
            instance_count: packed.draws.len(),
            instance_bytes: 0,
            geometry_uploads: packed.draws.len() as u64,
            buffer_rebuilds: 1,
            dynamic_bytes_written: 0,
            shadow_map_bytes: 0,
            shadow_map_allocations: 0,
            shadow_draw_count: 0,
        };
        let max = gpu.device.limits().max_buffer_size;
        if [stats.vertex_bytes, stats.index_bytes, stats.uniform_bytes]
            .into_iter()
            .any(|n| n > max)
            || stats.uniform_bytes > u64::from(u32::MAX)
        {
            return Err(GpuError::InvalidInput(
                "scene exceeds device buffer limits".into(),
            ));
        }
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
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
        let vertices = create(
            "persistent vertices",
            bytemuck::cast_slice(&packed.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let indices = create(
            "persistent indices",
            bytemuck::cast_slice(&packed.indices),
            wgpu::BufferUsages::INDEX,
        );
        let uniforms = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("persistent MVPs"),
            size: stats.uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("MVP layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(64),
                    },
                    count: None,
                }],
            });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MVP bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniforms,
                    offset: 0,
                    size: NonZeroU64::new(64),
                }),
            }],
        });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("color pipeline layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Astral Echo vertex colors"),
                source: wgpu::ShaderSource::Wgsl(include_str!("vertex_color.wgsl").into()),
            });
        let pipeline = color_pipeline(&gpu, format, &pipeline_layout, &shader, false);
        if let Some(error) = scope.pop().await {
            return Err(GpuError::Backend(error.to_string()));
        }
        gpu.check()?;
        Ok(Self {
            gpu,
            pipeline,
            pipeline_layout,
            shader,
            instanced: None,
            timer: None,
            vertices,
            indices,
            uniforms,
            bind_group,
            draws: packed.draws,
            uniform_data: vec![0; stats.uniform_bytes as usize],
            stride,
            format,
            stats,
            snapshot_keys: None,
            prepared_keys: Vec::new(),
            prepared_meshes: Vec::new(),
        })
    }

    pub fn enable_gpu_timing(&mut self) -> bool {
        if self.timer.is_none() {
            self.timer = crate::timing::GpuTimer::new(&self.gpu);
        }
        self.timer.is_some()
    }

    pub fn read_gpu_timings(&self) -> Result<Option<crate::FrameGpuTimings>, GpuError> {
        self.timer
            .as_ref()
            .map_or(Ok(None), |timer| timer.read(&self.gpu))
    }

    pub fn lighting_cold_setup_ms(&self) -> Option<[f64; 3]> {
        self.instanced
            .as_ref()
            .and_then(|scene| scene.lighting_cold_setup_ms())
    }

    pub fn resources(&self) -> ResourceStats {
        if let Some(instanced) = &self.instanced {
            instanced.resources(self.stats)
        } else {
            self.stats
        }
    }

    pub fn from_snapshot(gpu: Arc<GpuContext>, scene: &SnapshotScene) -> Result<Self, GpuError> {
        let mut renderer = Self::new(gpu, wgpu::TextureFormat::Rgba8Unorm, &scene.owned_draws())?;
        renderer.snapshot_keys = Some(
            scene
                .visible_nodes()
                .map(|n| (n.entity, n.mesh, n.material))
                .collect(),
        );
        Ok(renderer)
    }

    pub fn from_prepared(gpu: Arc<GpuContext>, scene: &PreparedScene) -> Result<Self, GpuError> {
        let mut renderer = Self::new(gpu, wgpu::TextureFormat::Rgba8Unorm, &[])?;
        renderer.sync_prepared(scene)?;
        Ok(renderer)
    }

    pub fn from_prepared_hdr(
        gpu: Arc<GpuContext>,
        scene: &PreparedScene,
    ) -> Result<Self, GpuError> {
        let mut renderer = Self::new(gpu, wgpu::TextureFormat::Rgba16Float, &[])?;
        renderer.sync_prepared(scene)?;
        Ok(renderer)
    }

    pub fn sync_prepared(&mut self, scene: &PreparedScene) -> Result<bool, GpuError> {
        if self.instanced.is_none() {
            self.instanced = Some(crate::instancing::InstancedScene::new(
                &self.gpu,
                self.format,
                &self.pipeline_layout,
                &self.shader,
            ));
        }
        self.instanced
            .as_mut()
            .expect("initialized instancing")
            .sync(&self.gpu, scene)
    }

    pub fn sync_prepared_reference(&mut self, scene: &PreparedScene) -> Result<bool, GpuError> {
        let keys: Vec<_> = scene
            .draws()
            .map(|d| (d.node.entity, d.node.mesh, d.node.material, d.lod))
            .collect();
        let rebuild = self.instanced.is_some()
            || self.snapshot_keys.is_some()
            || self.prepared_keys != keys
            || self.draws.len() != scene.visible
            || !scene
                .draws()
                .map(|d| &d.mesh)
                .zip(&self.prepared_meshes)
                .all(|(a, b)| Arc::ptr_eq(a, b));
        if rebuild {
            let mut renderer = Self::new(self.gpu.clone(), self.format, &scene.owned_draws())?;
            renderer.prepared_keys = keys;
            renderer.prepared_meshes = scene.draws().map(|d| d.mesh.clone()).collect();
            renderer.stats.geometry_uploads += self.stats.geometry_uploads;
            renderer.stats.buffer_rebuilds += self.stats.buffer_rebuilds;
            renderer.stats.dynamic_bytes_written += self.stats.dynamic_bytes_written;
            *self = renderer;
        } else {
            for (index, draw) in scene.draws().enumerate() {
                self.set_transform(index, draw.node.transform)?;
            }
        }
        Ok(rebuild)
    }

    pub fn sync_snapshot(&mut self, scene: &SnapshotScene) -> Result<bool, GpuError> {
        let keys = || {
            scene
                .visible_nodes()
                .map(|n| (n.entity, n.mesh, n.material))
        };
        let rebuild = self.instanced.is_some()
            || self
                .snapshot_keys
                .as_ref()
                .is_none_or(|existing| !keys().eq(existing.iter().copied()));
        if rebuild {
            let mut renderer = Self::new(self.gpu.clone(), self.format, &scene.owned_draws())?;
            renderer.snapshot_keys = Some(keys().collect());
            renderer.stats.geometry_uploads += self.stats.geometry_uploads;
            renderer.stats.buffer_rebuilds += self.stats.buffer_rebuilds;
            renderer.stats.dynamic_bytes_written += self.stats.dynamic_bytes_written;
            *self = renderer;
        } else {
            for (index, node) in scene.visible_nodes().enumerate() {
                self.set_transform(index, node.transform)?;
            }
        }
        Ok(rebuild)
    }

    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn set_transform(&mut self, draw: usize, transform: Transform) -> Result<(), GpuError> {
        if !transform.matrix().is_finite() {
            return Err(GpuError::InvalidInput("non-finite transform".into()));
        }
        if let Some(instanced) = &mut self.instanced {
            return instanced.set_transform(draw, transform);
        }
        let entry = self
            .draws
            .get_mut(draw)
            .ok_or_else(|| GpuError::InvalidInput("draw index out of range".into()))?;
        entry.1 = transform;
        Ok(())
    }

    pub fn render_to_view(
        &mut self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera: &Camera,
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.render_to_view_matrix(color, depth, camera.view_projection_matrix())
    }

    pub fn render_to_view_matrix(
        &mut self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.render_to_view_matrix_with_clear(color, depth, vp, CLEAR_RGBA)
    }

    pub fn render_to_view_matrix_with_clear(
        &mut self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
        clear: [u8; 4],
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.gpu.check()?;
        if !vp.is_finite() {
            return Err(GpuError::InvalidInput("non-finite camera matrix".into()));
        }
        if let Some(instanced) = &mut self.instanced {
            instanced.update(&self.gpu, vp)?;
        } else {
            for (index, (_, model)) in self.draws.iter().enumerate() {
                let mvp = vp * model.matrix();
                if !mvp.is_finite() {
                    return Err(GpuError::InvalidInput("non-finite MVP matrix".into()));
                }
                let mvp = mvp.to_cols_array();
                let offset = index * self.stride as usize;
                self.uniform_data[offset..offset + 64].copy_from_slice(bytemuck::cast_slice(&mvp));
            }
            self.gpu
                .queue
                .write_buffer(&self.uniforms, 0, &self.uniform_data);
            self.stats.dynamic_bytes_written += self.uniform_data.len() as u64;
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("color frame"),
            });
        if let Some(instanced) = &self.instanced {
            instanced.encode_shadow(
                &mut encoder,
                self.timer.as_ref().map(|timer| timer.writes(true)),
            );
        }
        {
            let channel = |v: u8| {
                let v = f64::from(v) / 255.0;
                if self.format.is_srgb() {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                } else {
                    v
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("color and depth"),
                timestamp_writes: self.timer.as_ref().map(|timer| timer.writes(false)),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: channel(clear[0]),
                            g: channel(clear[1]),
                            b: channel(clear[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            if let Some(instanced) = &self.instanced {
                pass.set_bind_group(0, &self.bind_group, &[0]);
                instanced.encode(&mut pass);
            } else {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
                for (index, (indices, _)) in self.draws.iter().enumerate() {
                    pass.set_bind_group(0, &self.bind_group, &[index as u32 * self.stride]);
                    pass.draw_indexed(indices.clone(), 0, 0..1);
                }
            }
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(
                &mut encoder,
                self.instanced
                    .as_ref()
                    .is_some_and(|scene| scene.shadows_active()),
            );
        }
        Ok(self.gpu.queue.submit([encoder.finish()]))
    }

    pub fn clear_view(
        &mut self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: [u8; 4],
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.gpu.check()?;
        let channel = |v: u8| f64::from(v) / 255.0;
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("overlay clear"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: channel(clear[0]),
                            g: channel(clear[1]),
                            b: channel(clear[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        Ok(self.gpu.queue.submit([encoder.finish()]))
    }
}

pub(crate) fn color_pipeline(
    gpu: &GpuContext,
    format: wgpu::TextureFormat,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    instanced: bool,
) -> wgpu::RenderPipeline {
    let instance_layouts = [
        wgpu::VertexBufferLayout {
            array_stride: 28,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
        },
        wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4],
        },
    ];
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Astral Echo indexed color/depth"),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(if instanced { "vs_instanced" } else { "vs_main" }),
                compilation_options: Default::default(),
                buffers: if instanced {
                    &instance_layouts
                } else {
                    &instance_layouts[..1]
                },
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if format.is_srgb() {
                    "fs_srgb"
                } else {
                    "fs_main"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

pub(crate) fn validate_extent(gpu: &GpuContext, width: u32, height: u32) -> Result<(), GpuError> {
    let limit = gpu.device.limits().max_texture_dimension_2d;
    if width == 0 || height == 0 || width > limit || height > limit {
        return Err(GpuError::InvalidInput(format!(
            "invalid target {width}x{height}; device limit {limit}"
        )));
    }
    Ok(())
}

pub(crate) struct DepthTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl DepthTarget {
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Result<Self, GpuError> {
        validate_extent(gpu, width, height)?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("persistent depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self { texture, view })
    }
}
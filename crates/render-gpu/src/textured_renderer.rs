use std::sync::Arc;

use bytemuck::cast_slice;
use render_core::{Mat4, RasterMesh, Transform};
use wgpu::util::DeviceExt;

use crate::{GpuContext, GpuError, GpuTexture, TexturedVertex};

pub struct TexturedRenderer {
    gpu: Arc<GpuContext>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    index_count: u32,
    model: Transform,
    format: wgpu::TextureFormat,
}

impl TexturedRenderer {
    pub fn new(
        gpu: Arc<GpuContext>,
        format: wgpu::TextureFormat,
        mesh: &RasterMesh,
        texture: &GpuTexture,
    ) -> Result<Self, GpuError> {
        gpu.check()?;

        if mesh.vertices.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            return Err(GpuError::InvalidInput("invalid textured mesh".into()));
        }

        if !mesh.vertices.iter().all(|vertex| {
            vertex.position.is_finite() && vertex.color.is_finite() && vertex.uv.is_finite()
        }) {
            return Err(GpuError::InvalidInput("non-finite textured vertex".into()));
        }

        if mesh
            .indices
            .iter()
            .any(|&index| index as usize >= mesh.vertices.len())
        {
            return Err(GpuError::InvalidInput(
                "textured mesh index out of range".into(),
            ));
        }

        let index_count = u32::try_from(mesh.indices.len())
            .map_err(|_| GpuError::InvalidInput("too many texture indices".into()))?;

        let vertices: Vec<TexturedVertex> =
            mesh.vertices.iter().map(TexturedVertex::from).collect();

        let vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Textured vertex buffer"),
                contents: cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Textured index buffer"),
                contents: cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Textured MVP uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Grupo 0: matriz MVP
        let camera_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Textured camera layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(64),
                    },
                    count: None,
                }],
            });

        let camera_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Textured camera bind group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Grupo 1: textura PNG y sampler
        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Textured material layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let texture_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Textured material bind group"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Textured pipeline layout"),
                bind_group_layouts: &[Some(&camera_layout), Some(&texture_layout)],
                immediate_size: 0,
            });

        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Astral Echo textured shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("textured.wgsl").into()),
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Astral Echo textured pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[TexturedVertex::layout()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        gpu.check()?;

        Ok(Self {
            gpu,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            camera_bind_group,
            texture_bind_group,
            index_count,
            model: Transform::IDENTITY,
            format,
        })
    }

    pub fn set_transform(&mut self, transform: Transform) -> Result<(), GpuError> {
        if !transform.matrix().is_finite() {
            return Err(GpuError::InvalidInput("non-finite model transform".into()));
        }

        self.model = transform;
        Ok(())
    }

    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn render_to_view_matrix(
        &self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.render_to_view_matrix_internal(color, depth, vp, true)
    }

    pub fn render_to_view_matrix_over(
        &self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.render_to_view_matrix_internal(color, depth, vp, false)
    }

    fn render_to_view_matrix_internal(
        &self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
        clear: bool,
    ) -> Result<wgpu::SubmissionIndex, GpuError> {
        self.gpu.check()?;

        let mvp = vp * self.model.matrix();

        if !mvp.is_finite() {
            return Err(GpuError::InvalidInput("non-finite MVP matrix".into()));
        }

        self.gpu
            .queue
            .write_buffer(&self.uniform_buffer, 0, cast_slice(&mvp.to_cols_array()));

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Textured render encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Textured render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if clear {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.03,
                                g: 0.04,
                                b: 0.07,
                                a: 1.0,
                            })
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if clear {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);

            pass.set_bind_group(0, &self.camera_bind_group, &[]);

            pass.set_bind_group(1, &self.texture_bind_group, &[]);

            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }

        Ok(self.gpu.queue.submit([encoder.finish()]))
    }
}

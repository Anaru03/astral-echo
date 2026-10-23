use crate::{GpuContext, GpuError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostProcessPreset {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostProcessConfig {
    pub enabled: bool,
    pub exposure: f32,
    pub bloom: bool,
    pub bloom_threshold: f32,
    pub bloom_strength: f32,
    pub fxaa: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exposure: 1.0,
            bloom: false,
            bloom_threshold: 1.0,
            bloom_strength: 0.08,
            fxaa: false,
        }
    }
}

impl PostProcessConfig {
    pub fn preset(preset: PostProcessPreset) -> Self {
        match preset {
            PostProcessPreset::Low => Self::default(),
            PostProcessPreset::Medium => Self {
                enabled: true,
                bloom: true,
                ..Self::default()
            },
            PostProcessPreset::High => Self {
                enabled: true,
                bloom: true,
                fxaa: true,
                bloom_strength: 0.14,
                ..Self::default()
            },
        }
    }
    pub fn validate(self) -> Result<(), GpuError> {
        if !self.exposure.is_finite()
            || self.exposure <= 0.0
            || !self.bloom_threshold.is_finite()
            || self.bloom_threshold < 0.0
            || !self.bloom_strength.is_finite()
            || self.bloom_strength < 0.0
        {
            return Err(GpuError::InvalidInput(
                "invalid post-process configuration".into(),
            ));
        }
        Ok(())
    }
}

pub fn aces_tonemap(color: [f32; 3], exposure: f32) -> [f32; 3] {
    let x = [
        color[0] * exposure,
        color[1] * exposure,
        color[2] * exposure,
    ];
    let map = |v: f32| ((v * (2.51 * v + 0.03)) / (v * (2.43 * v + 0.59) + 0.14)).clamp(0.0, 1.0);
    [map(x[0]), map(x[1]), map(x[2])]
}

pub(crate) struct PostProcessPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    layout: wgpu::BindGroupLayout,
    source: wgpu::Texture,
    source_view: wgpu::TextureView,
    source_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    config: PostProcessConfig,
    pub allocations: u64,
}

impl PostProcessPass {
    pub fn new(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        source_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("postprocess source"),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("postprocess parameters"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("hdr tone mapping bloom fxaa"),
                source: wgpu::ShaderSource::Wgsl(include_str!("postprocess.wgsl").into()),
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("postprocess"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("postprocess"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_postprocess"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        let (source, source_view) = source_texture(gpu, source_format, width, height);
        let bind_group = bind(gpu, &layout, &source_view, &sampler, &uniform);
        Ok(Self {
            pipeline,
            bind_group,
            uniform,
            sampler,
            layout,
            source,
            source_view,
            source_format,
            width,
            height,
            config: PostProcessConfig::default(),
            allocations: 1,
        })
    }
    pub fn resources(&self) -> (u32, u32, u64) {
        let _ = (
            &self.sampler,
            &self.layout,
            &self.source,
            self.source_format,
        );
        (self.width, self.height, self.allocations)
    }
    pub fn set_config(&mut self, config: PostProcessConfig) -> Result<(), GpuError> {
        config.validate()?;
        self.config = config;
        Ok(())
    }
    pub fn source_view(&self) -> &wgpu::TextureView {
        &self.source_view
    }
    pub fn config(&self) -> PostProcessConfig {
        self.config
    }
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
    pub fn encode(
        &self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        let mut data = [0.0f32; 8];
        data[0] = self.config.exposure;
        data[1] = self.config.bloom_threshold;
        data[2] = self.config.bloom_strength;
        data[3] = f32::from(self.config.bloom);
        data[4] = f32::from(self.config.fxaa);
        gpu.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&data));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hdr postprocess"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn source_texture(
    gpu: &GpuContext,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("persistent HDR source"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}
fn bind(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("postprocess source"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

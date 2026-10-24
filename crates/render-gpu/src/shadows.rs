use crate::{block_on, GpuContext, GpuError};
use render_core::{Mat4, ShadowConfig, Transform, Vec3};

pub(crate) struct ShadowPass {
    pub active: bool,
    pub depth_pipeline: wgpu::RenderPipeline,
    color_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    pub view: wgpu::TextureView,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pub instances: wgpu::Buffer,
    capacity: usize,
    resolution: u32,
    data: Vec<[f32; 16]>,
    next_data: Vec<[f32; 16]>,
    uniform_data: Option<[f32; 24]>,
    pub allocations: u64,
    pub reallocations: u64,
    pub bytes_written: u64,
}

impl ShadowPass {
    pub fn new(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        lighting_layout: &wgpu::BindGroupLayout,
        config: ShadowConfig,
    ) -> Result<Self, GpuError> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow sampling"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(96),
                        },
                        count: None,
                    },
                ],
            });
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow environment"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shadow lighting"),
                source: wgpu::ShaderSource::Wgsl(
                    format!(
                        "{}\n{}",
                        include_str!("lighting.wgsl"),
                        include_str!("shadows.wgsl")
                    )
                    .into(),
                ),
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow lighting"),
                bind_group_layouts: &[Some(lighting_layout), Some(&layout)],
                immediate_size: 0,
            });
        let color_pipeline =
            crate::lighting::lighting_pipeline(gpu, format, &pipeline_layout, &shader, true);
        let depth_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shadow depth"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shadow_depth.wgsl").into()),
            });
        let depth_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow depth"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let depth_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("instanced directional shadow"),layout:Some(&depth_layout),
            vertex:wgpu::VertexState {module:&depth_shader,entry_point:Some("vs_depth"),compilation_options:Default::default(),buffers:&[
                wgpu::VertexBufferLayout {array_stride:28,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0 => Float32x3]},
                wgpu::VertexBufferLayout {array_stride:64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![3 => Float32x4,4 => Float32x4,5 => Float32x4,6 => Float32x4]},
            ]},
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},
            depth_stencil:Some(wgpu::DepthStencilState {format:crate::renderer::DEPTH_FORMAT,depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::Less),stencil:Default::default(),bias:Default::default()}),
            multisample:Default::default(),fragment:None,multiview_mask:None,cache:None,
        });
        let resolution = config.resolution as u32;
        let (texture, view, bind_group) = shadow_map(gpu, resolution, &layout, &sampler, &uniform);
        if let Some(error) = block_on(scope.pop()) {
            return Err(GpuError::Backend(error.to_string()));
        }
        Ok(Self {
            active: true,
            depth_pipeline,
            color_pipeline,
            layout,
            sampler,
            uniform,
            view,
            texture,
            bind_group,
            instances: instance_buffer(gpu, 1),
            capacity: 1,
            resolution,
            data: Vec::new(),
            next_data: Vec::new(),
            uniform_data: None,
            allocations: 1,
            reallocations: 1,
            bytes_written: 0,
        })
    }

    pub fn sync(
        &mut self,
        gpu: &GpuContext,
        config: ShadowConfig,
        vp: Mat4,
        direction: Vec3,
        instances_changed: bool,
        transforms: impl Iterator<Item = Transform>,
    ) -> Result<(), GpuError> {
        self.active = true;
        if self.resolution != config.resolution as u32 {
            (self.texture, self.view, self.bind_group) = shadow_map(
                gpu,
                config.resolution as u32,
                &self.layout,
                &self.sampler,
                &self.uniform,
            );
            self.resolution = config.resolution as u32;
            self.allocations += 1;
        }
        let mut uniform = [0.0; 24];
        uniform[..16].copy_from_slice(&vp.to_cols_array());
        uniform[16..20].copy_from_slice(&[
            config.depth_bias,
            config.slope_bias,
            config.normal_bias,
            1.0 / self.resolution as f32,
        ]);
        uniform[20..24].copy_from_slice(&direction.extend(f32::from(config.pcf)).to_array());
        let matrix_changed = self
            .uniform_data
            .is_none_or(|previous| previous[..16] != uniform[..16]);
        if self.uniform_data != Some(uniform) {
            gpu.queue
                .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
            self.uniform_data = Some(uniform);
            self.bytes_written += 96;
        }
        if instances_changed || matrix_changed {
            self.update_instances(gpu, vp, transforms)?;
        }
        Ok(())
    }

    pub fn update_instances(
        &mut self,
        gpu: &GpuContext,
        vp: Mat4,
        transforms: impl Iterator<Item = Transform>,
    ) -> Result<(), GpuError> {
        self.next_data.clear();
        self.next_data
            .extend(transforms.map(|t| (vp * t.matrix()).to_cols_array()));
        let next = &self.next_data;
        if next.iter().flatten().any(|v| !v.is_finite()) {
            return Err(GpuError::InvalidInput("non-finite shadow transform".into()));
        }
        let mut force = false;
        if next.len() > self.capacity {
            let capacity = next
                .len()
                .checked_next_power_of_two()
                .ok_or_else(|| GpuError::InvalidInput("shadow capacity overflow".into()))?;
            if capacity as u64 * 64 > gpu.device.limits().max_buffer_size {
                return Err(GpuError::InvalidInput(
                    "shadow buffer exceeds device limit".into(),
                ));
            }
            self.instances = instance_buffer(gpu, capacity);
            self.capacity = capacity;
            self.reallocations += 1;
            force = true;
        }
        let first = next
            .iter()
            .enumerate()
            .position(|(i, v)| force || self.data.get(i) != Some(v));
        if let Some(first) = first {
            let last = next
                .iter()
                .enumerate()
                .rposition(|(i, v)| force || self.data.get(i) != Some(v))
                .expect("dirty shadow instance");
            let bytes = bytemuck::cast_slice(&next[first..=last]);
            gpu.queue
                .write_buffer(&self.instances, first as u64 * 64, bytes);
            self.bytes_written += bytes.len() as u64;
        }
        std::mem::swap(&mut self.data, &mut self.next_data);
        Ok(())
    }

    pub fn view_projection(&self) -> Mat4 {
        let uniform = self.uniform_data.expect("synced shadow environment");
        Mat4::from_cols_array(uniform[..16].try_into().expect("shadow matrix"))
    }

    pub fn bind_color(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.color_pipeline);
        pass.set_bind_group(1, &self.bind_group, &[]);
    }

    pub fn texture_bytes(&self) -> u64 {
        u64::from(self.resolution).pow(2) * 4
    }
    pub fn instance_bytes(&self) -> u64 {
        self.capacity as u64 * 64
    }
}

fn instance_buffer(gpu: &GpuContext, capacity: usize) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("persistent shadow instances"),
        size: capacity as u64 * 64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn shadow_map(
    gpu: &GpuContext,
    resolution: u32,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("persistent directional shadow map"),
        size: wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::renderer::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow sampling"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
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
    });
    (texture, view, group)
}

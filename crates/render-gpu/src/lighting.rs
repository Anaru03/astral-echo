use crate::{block_on, GpuContext, GpuError};
use bytemuck::{Pod, Zeroable};
use render_core::{
    normal_matrix, LightingEnvironment, Mat4, MaterialInfo, PreparedScene, RenderLightKind,
    Transform,
};

pub(crate) const INSTANCE_BYTES: u64 = 208;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
struct InstanceData {
    rows: [[f32; 4]; 13],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightData {
    vector: [f32; 4],
    color: [f32; 4],
    range: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EnvironmentData {
    camera: [f32; 4],
    ambient: [f32; 4],
    counts: [f32; 4],
    lights: [LightData; 5],
}

pub(crate) struct LightingPass {
    pub active: bool,
    pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
    pub cold_setup: [f64; 3],
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    environment: Option<LightingEnvironment>,
    srgb: bool,
    sources: Vec<(Transform, MaterialInfo)>,
    next_sources: Vec<(Transform, MaterialInfo)>,
    data: Vec<InstanceData>,
    next_data: Vec<InstanceData>,
    dirty: bool,
    force_write: bool,
    last_vp: Option<Mat4>,
    hdr: bool,
    pub reallocations: u64,
    pub bytes_written: u64,
}

impl LightingPass {
    pub fn new(gpu: &GpuContext, format: wgpu::TextureFormat) -> Result<Self, GpuError> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("lighting environment layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(288),
                    },
                    count: None,
                }],
            });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting environment"),
            size: 288,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting environment"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("lighting pipeline"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let shader_start = std::time::Instant::now();
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Lambert Blinn GGX"),
                source: wgpu::ShaderSource::Wgsl(include_str!("lighting.wgsl").into()),
            });
        let shader_ms = shader_start.elapsed().as_secs_f64() * 1000.0;
        let pipeline_start = std::time::Instant::now();
        let pipeline = lighting_pipeline(gpu, format, &pipeline_layout, &shader, false);
        let pipeline_ms = pipeline_start.elapsed().as_secs_f64() * 1000.0;
        let validation_start = std::time::Instant::now();
        if let Some(error) = block_on(scope.pop()) {
            return Err(GpuError::Backend(error.to_string()));
        }
        Ok(Self {
            active: true,
            layout,
            cold_setup: [
                shader_ms,
                pipeline_ms,
                validation_start.elapsed().as_secs_f64() * 1000.0,
            ],
            pipeline,
            bind_group,
            uniform,
            instances: instance_buffer(gpu, 1),
            capacity: 1,
            environment: None,
            srgb: format.is_srgb(),
            sources: Vec::new(),
            next_sources: Vec::new(),
            data: Vec::new(),
            next_data: Vec::new(),
            dirty: true,
            force_write: true,
            last_vp: None,
            hdr: format == wgpu::TextureFormat::Rgba16Float,
            reallocations: 1,
            bytes_written: 0,
        })
    }

    pub fn sync(&mut self, gpu: &GpuContext, scene: &PreparedScene) -> Result<(), GpuError> {
        if !scene.lighting.camera_position.is_finite() || !scene.lighting.ambient.is_finite() {
            return Err(GpuError::InvalidInput(
                "non-finite lighting environment".into(),
            ));
        }
        self.active = true;
        self.next_sources.clear();
        self.next_sources.extend(scene.draws().map(|draw| {
            (
                draw.node.transform,
                scene.material(draw.node.material).sanitized(),
            )
        }));
        self.dirty |= self.sources != self.next_sources;
        std::mem::swap(&mut self.sources, &mut self.next_sources);
        if self.sources.len() > self.capacity {
            let capacity = self
                .sources
                .len()
                .checked_next_power_of_two()
                .ok_or_else(|| GpuError::InvalidInput("lighting capacity overflow".into()))?;
            if capacity as u64 * INSTANCE_BYTES > gpu.device.limits().max_buffer_size {
                return Err(GpuError::InvalidInput(
                    "lighting buffer exceeds device limit".into(),
                ));
            }
            self.instances = instance_buffer(gpu, capacity);
            self.capacity = capacity;
            self.force_write = true;
            self.reallocations += 1;
        }
        if self.environment.as_ref() != Some(&scene.lighting) {
            let mut data = EnvironmentData::zeroed();
            data.camera = scene
                .lighting
                .camera_position
                .extend(scene.lighting.mode as u32 as f32)
                .to_array();
            data.ambient = scene
                .lighting
                .ambient
                .max(render_core::Vec3::ZERO)
                .extend(f32::from(self.srgb))
                .to_array();
            data.counts[0] = scene.lighting.lights().len() as f32;
            data.counts[1] = f32::from(self.hdr);
            for (out, light) in data.lights.iter_mut().zip(scene.lighting.lights()) {
                out.color = light.color.extend(light.intensity).to_array();
                match light.kind {
                    RenderLightKind::Directional { direction } => {
                        out.vector = direction.extend(0.0).to_array()
                    }
                    RenderLightKind::Point { position, range } => {
                        out.vector = position.extend(1.0).to_array();
                        out.range[0] = range;
                    }
                }
            }
            gpu.queue
                .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&data));
            self.bytes_written += 288;
            self.environment = Some(scene.lighting.clone());
        }
        Ok(())
    }

    pub fn set_transform(&mut self, index: usize, transform: Transform) {
        if let Some(source) = self.sources.get_mut(index) {
            self.dirty |= source.0 != transform;
            source.0 = transform;
        }
    }

    pub fn update(&mut self, gpu: &GpuContext, vp: Mat4) -> Result<(), GpuError> {
        if !self.dirty && !self.force_write && self.last_vp == Some(vp) {
            return Ok(());
        }
        self.next_data.clear();
        for (transform, material) in &self.sources {
            let model = transform.matrix();
            let mvp = vp * model;
            if !mvp.is_finite() {
                return Err(GpuError::InvalidInput("non-finite lighting MVP".into()));
            }
            let normal = normal_matrix(model);
            let mut rows = [[0.0; 4]; 13];
            rows[..4].copy_from_slice(&mvp.to_cols_array_2d());
            for index in 0..3 {
                rows[4 + index] = model.row(index).to_array();
                rows[7 + index] = normal.row(index).to_array();
            }
            rows[10] = material.base_color.to_array();
            rows[11] = [material.metallic, material.roughness, 0.0, 0.0];
            rows[12] = material.emissive.extend(0.0).to_array();
            self.next_data.push(InstanceData { rows });
        }
        let first = self
            .next_data
            .iter()
            .enumerate()
            .position(|(i, v)| self.force_write || self.data.get(i) != Some(v));
        if let Some(first) = first {
            let last = self
                .next_data
                .iter()
                .enumerate()
                .rposition(|(i, v)| self.force_write || self.data.get(i) != Some(v))
                .expect("dirty lighting instance");
            let bytes = bytemuck::cast_slice(&self.next_data[first..=last]);
            gpu.queue
                .write_buffer(&self.instances, first as u64 * INSTANCE_BYTES, bytes);
            self.bytes_written += bytes.len() as u64;
        }
        std::mem::swap(&mut self.data, &mut self.next_data);
        self.force_write = false;
        self.dirty = false;
        self.last_vp = Some(vp);
        Ok(())
    }

    pub fn bind(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.slice(..));
    }

    pub fn instance_bytes(&self) -> u64 {
        self.capacity as u64 * INSTANCE_BYTES
    }
}

fn instance_buffer(gpu: &GpuContext, capacity: usize) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("reusable lit instances"),
        size: capacity as u64 * INSTANCE_BYTES,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn lighting_pipeline(
    gpu: &GpuContext,
    format: wgpu::TextureFormat,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    shadow: bool,
) -> wgpu::RenderPipeline {
    gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("instanced lighting"),layout:Some(pipeline_layout),
            vertex:wgpu::VertexState {module:shader,entry_point:Some(if shadow { "vs_shadow_lighting" } else { "vs_lighting" }),compilation_options:Default::default(),buffers:&[
                wgpu::VertexBufferLayout {array_stride:28,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0 => Float32x3,1 => Float32x4]},
                wgpu::VertexBufferLayout {array_stride:INSTANCE_BYTES,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![3 => Float32x4,4 => Float32x4,5 => Float32x4,6 => Float32x4,7 => Float32x4,8 => Float32x4,9 => Float32x4,10 => Float32x4,11 => Float32x4,12 => Float32x4,13 => Float32x4,14 => Float32x4,15 => Float32x4]},
                wgpu::VertexBufferLayout {array_stride:12,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![2 => Float32x3]},
            ]},
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},
            depth_stencil:Some(wgpu::DepthStencilState {format:crate::renderer::DEPTH_FORMAT,depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::Less),stencil:Default::default(),bias:Default::default()}),
            multisample:Default::default(),
            fragment:Some(wgpu::FragmentState {module:shader,entry_point:Some(if shadow { "fs_shadow_lighting" } else { "fs_lighting" }),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:None,write_mask:wgpu::ColorWrites::ALL})]}),
            multiview_mask:None,cache:None,
        })
}
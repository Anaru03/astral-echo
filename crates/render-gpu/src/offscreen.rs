use std::sync::{mpsc, Arc};

use render_core::{Camera, Mat4, PreparedScene, SnapshotScene};

use crate::postprocess::{PostProcessConfig, PostProcessPass};
use crate::renderer::{validate_extent, DepthTarget};
use crate::{GpuContext, GpuError, GpuImage, GpuRenderer, ReadbackLayout};

pub struct OffscreenTarget {
    gpu: Arc<GpuContext>,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth: DepthTarget,
    color_readback: wgpu::Buffer,
    depth_readback: wgpu::Buffer,
    layout: ReadbackLayout,
    postprocess: Option<PostProcessPass>,
}

impl OffscreenTarget {
    pub fn new(gpu: Arc<GpuContext>, width: u32, height: u32) -> Result<Self, GpuError> {
        validate_extent(&gpu, width, height)?;
        let layout = ReadbackLayout::new(width, height)?;
        if layout.buffer_size > gpu.device.limits().max_buffer_size {
            return Err(GpuError::InvalidInput(
                "readback exceeds buffer limit".into(),
            ));
        }
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("persistent offscreen color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&Default::default());
        let depth = DepthTarget::new(&gpu, width, height)?;
        let create = |label| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: layout.buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };
        let color_readback = create("persistent color staging");
        let depth_readback = create("persistent depth staging");
        Ok(Self {
            gpu,
            color,
            color_view,
            depth,
            color_readback,
            depth_readback,
            layout,
            postprocess: None,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.layout.width, self.layout.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        if self.size() != (width, height) {
            let config = self.postprocess.as_ref().map(|pass| pass.config());
            *self = Self::new(self.gpu.clone(), width, height)?;
            if let Some(config) = config {
                self.set_postprocess(config)?;
            }
        }
        Ok(())
    }

    pub fn set_postprocess(&mut self, config: PostProcessConfig) -> Result<(), GpuError> {
        config.validate()?;
        if config.enabled {
            if self.postprocess.is_none() {
                self.postprocess = Some(PostProcessPass::new(
                    &self.gpu,
                    wgpu::TextureFormat::Rgba8Unorm,
                    self.layout.width,
                    self.layout.height,
                )?);
            }
            self.postprocess
                .as_mut()
                .expect("postprocess")
                .set_config(config)?;
        } else if let Some(pass) = &mut self.postprocess {
            pass.set_config(config)?;
        }
        Ok(())
    }

    pub fn postprocess_config(&self) -> Option<PostProcessConfig> {
        self.postprocess.as_ref().map(|pass| pass.config())
    }
    pub fn postprocess_resources(&self) -> Option<(u32, u32, u64)> {
        self.postprocess.as_ref().map(|pass| pass.resources())
    }

    pub fn render(&self, renderer: &mut GpuRenderer, camera: &Camera) -> Result<(), GpuError> {
        self.render_matrix(renderer, camera.view_projection_matrix())
    }

    pub fn render_snapshot(
        &self,
        renderer: &mut GpuRenderer,
        scene: &SnapshotScene,
    ) -> Result<(), GpuError> {
        self.validate_renderer(renderer)?;
        renderer.sync_snapshot(scene)?;
        self.render_matrix(renderer, scene.view_projection())
    }

    pub fn render_prepared(
        &self,
        renderer: &mut GpuRenderer,
        scene: &PreparedScene,
        view_projection: Mat4,
    ) -> Result<(), GpuError> {
        self.validate_renderer(renderer)?;
        renderer.sync_prepared(scene)?;
        self.render_matrix(renderer, view_projection)
    }

    fn validate_renderer(&self, renderer: &GpuRenderer) -> Result<(), GpuError> {
        if renderer.color_format() != wgpu::TextureFormat::Rgba8Unorm {
            return Err(GpuError::InvalidInput(
                "offscreen target requires Rgba8Unorm pipeline".into(),
            ));
        }
        if !Arc::ptr_eq(&self.gpu, &renderer.gpu) {
            return Err(GpuError::InvalidInput(
                "target and renderer belong to different contexts".into(),
            ));
        }
        Ok(())
    }

    pub fn render_matrix(&self, renderer: &mut GpuRenderer, vp: Mat4) -> Result<(), GpuError> {
        self.validate_renderer(renderer)?;
        let Some(pass) = self.postprocess.as_ref().filter(|pass| pass.is_enabled()) else {
            renderer.render_to_view_matrix(&self.color_view, &self.depth.view, vp)?;
            return Ok(());
        };
        renderer.render_to_view_matrix(pass.source_view(), &self.depth.view, vp)?;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        pass.encode(&self.gpu, &mut encoder, &self.color_view);
        self.gpu.queue.submit([encoder.finish()]);
        Ok(())
    }

    pub fn readback(&self) -> Result<GpuImage, GpuError> {
        self.gpu.check()?;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        for (texture, aspect, buffer) in [
            (&self.color, wgpu::TextureAspect::All, &self.color_readback),
            (
                &self.depth.texture,
                wgpu::TextureAspect::DepthOnly,
                &self.depth_readback,
            ),
        ] {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.layout.bytes_per_row),
                        rows_per_image: Some(self.layout.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.layout.width,
                    height: self.layout.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.gpu.queue.submit([encoder.finish()]);
        let map = |buffer: &wgpu::Buffer| -> Result<Vec<u8>, GpuError> {
            let (tx, rx) = mpsc::sync_channel(1);
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
            self.gpu.wait()?;
            rx.recv_timeout(std::time::Duration::from_secs(20))
                .map_err(|e| GpuError::Readback(e.to_string()))?
                .map_err(|e| GpuError::Readback(e.to_string()))?;
            let mapped = buffer.slice(..).get_mapped_range();
            let result = self.layout.unpack(&mapped);
            drop(mapped);
            buffer.unmap();
            result
        };
        let rgba = map(&self.color_readback)?;
        let depth_bytes = map(&self.depth_readback)?;
        let depth = depth_bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes([v[0], v[1], v[2], v[3]]))
            .collect();
        self.gpu.check()?;
        Ok(GpuImage {
            width: self.layout.width,
            height: self.layout.height,
            rgba,
            depth,
        })
    }
}

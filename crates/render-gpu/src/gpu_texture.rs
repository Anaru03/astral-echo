
use crate::texture_loader::TextureImage;
use crate::{GpuContext, GpuError};

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

impl GpuTexture {
    pub fn from_image(
        gpu: &GpuContext,
        image: &TextureImage,
    ) -> Result<Self, GpuError> {
        if image.width == 0 || image.height == 0 {
            return Err(GpuError::InvalidInput(
                "texture dimensions must be positive".into(),
            ));
        }

        let expected = (image.width as usize)
            .checked_mul(image.height as usize)
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                GpuError::InvalidInput(
                    "texture dimensions overflow".into(),
                )
            })?;

        if image.pixels.len() != expected {
            return Err(GpuError::InvalidInput(
                "invalid RGBA8 texture buffer".into(),
            ));
        }

        let limits = gpu.device.limits();

        if image.width > limits.max_texture_dimension_2d
            || image.height > limits.max_texture_dimension_2d
        {
            return Err(GpuError::InvalidInput(
                "texture exceeds GPU dimension limits".into(),
            ));
        }

        let texture = gpu.device.create_texture(
            &wgpu::TextureDescriptor {
                label: Some("Astral Echo model texture"),
                size: wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );

        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(
            &wgpu::TextureViewDescriptor::default(),
        );

        let sampler = gpu.device.create_sampler(
            &wgpu::SamplerDescriptor {
                label: Some("Astral Echo texture sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            },
        );

        Ok(Self {
            texture,
            view,
            sampler,
            width: image.width,
            height: image.height,
        })
    }
}

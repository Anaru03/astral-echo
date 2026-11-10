use std::path::Path;
use std::sync::Arc;

use render_core::{load_textured_obj, Mat4};

use crate::{texture_loader::TextureImage, GpuContext, GpuError, GpuTexture, TexturedRenderer};

pub struct TexturedModelRenderer {
    parts: Vec<TexturedRenderer>,
    textures: Vec<GpuTexture>,
}

impl TexturedModelRenderer {
    pub fn load(
        gpu: Arc<GpuContext>,
        format: wgpu::TextureFormat,
        path: impl AsRef<Path>,
    ) -> Result<Self, GpuError> {
        let model = load_textured_obj(path.as_ref())
            .map_err(|error| GpuError::InvalidInput(error.to_string()))?;

        let mut textures = Vec::new();
        let mut material_textures = Vec::new();

        for material in &model.materials {
            let index = if let Some(path) = &material.diffuse_texture {
                let image = TextureImage::load_png(path)
                    .map_err(|error| GpuError::InvalidInput(error.to_string()))?;

                let texture = GpuTexture::from_image(&gpu, &image)?;
                textures.push(texture);
                Some(textures.len() - 1)
            } else {
                None
            };

            material_textures.push(index);
        }

        let mut parts = Vec::new();

        for part in &model.meshes {
            let material_index = part.material_index.ok_or_else(|| {
                GpuError::InvalidInput(format!("mesh '{}' has no material", part.name))
            })?;

            let texture_index = material_textures
                .get(material_index)
                .and_then(|index| *index)
                .ok_or_else(|| {
                    GpuError::InvalidInput(format!("mesh '{}' has no diffuse texture", part.name))
                })?;

            let renderer =
                TexturedRenderer::new(gpu.clone(), format, &part.mesh, &textures[texture_index])?;

            parts.push(renderer);
        }

        if parts.is_empty() {
            return Err(GpuError::InvalidInput(
                "textured model contains no renderable parts".into(),
            ));
        }

        Ok(Self { parts, textures })
    }

    pub fn render_to_view_matrix(
        &self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vp: Mat4,
    ) -> Result<(), GpuError> {
        for (index, part) in self.parts.iter().enumerate() {
            if index == 0 {
                part.render_to_view_matrix(color, depth, vp)?;
            } else {
                part.render_to_view_matrix_over(color, depth, vp)?;
            }
        }

        Ok(())
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
}

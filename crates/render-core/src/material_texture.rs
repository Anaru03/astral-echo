
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::MaterialHandle;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextureResource {
    path: PathBuf,
    image: Arc<DecodedTexture>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterialTextureRegistry {
    textures: BTreeMap<MaterialHandle, TextureResource>,
}

impl MaterialTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asocia una imagen PNG con un material.
    pub fn register(
        &mut self,
        material: MaterialHandle,
        path: impl Into<PathBuf>,
    ) {
        if let Err(error) = self.try_register(material, path) {
            eprintln!("textura no cargada: {error}");
        }
    }

    pub fn try_register(
        &mut self,
        material: MaterialHandle,
        path: impl Into<PathBuf>,
    ) -> Result<(), String> {
        let path = path.into();
        if self
            .textures
            .get(&material)
            .is_some_and(|texture| texture.path == path)
        {
            return Ok(());
        }
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder
            .read_info()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let mut pixels = vec![
            0;
            reader.output_buffer_size().ok_or_else(|| format!(
                "{}: tamaño de imagen inválido",
                path.display()
            ))?
        ];
        let info = reader
            .next_frame(&mut pixels)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        pixels.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => pixels,
            png::ColorType::Rgb => pixels
                .chunks_exact(3)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect(),
            png::ColorType::Grayscale => pixels
                .into_iter()
                .flat_map(|value| [value, value, value, 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => pixels
                .chunks_exact(2)
                .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                .collect(),
            png::ColorType::Indexed => {
                return Err(format!("{}: indexed PNG no soportado", path.display()))
            }
        };
        let image = Arc::new(DecodedTexture {
            width: info.width,
            height: info.height,
            pixels: Arc::from(rgba),
        });
        self.textures.insert(material, TextureResource { path, image });
        Ok(())
    }

    /// Obtiene la ruta de la textura de un material.
    pub fn get(
        &self,
        material: MaterialHandle,
    ) -> Option<&Path> {
        self.textures.get(&material).map(|texture| texture.path.as_path())
    }

    pub fn image(&self, material: MaterialHandle) -> Option<Arc<DecodedTexture>> {
        self.textures.get(&material).map(|texture| texture.image.clone())
    }

    pub fn image_ref(&self, material: MaterialHandle) -> Option<&DecodedTexture> {
        self.textures.get(&material).map(|texture| texture.image.as_ref())
    }

    /// Elimina una textura registrada.
    pub fn remove(
        &mut self,
        material: MaterialHandle,
    ) -> Option<PathBuf> {
        self.textures.remove(&material).map(|texture| texture.path)
    }

    /// Comprueba si un material tiene textura.
    pub fn contains(
        &self,
        material: MaterialHandle,
    ) -> bool {
        self.textures.contains_key(&material)
    }

    /// Cantidad de materiales con textura.
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Itera sobre las texturas registradas.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (MaterialHandle, &Path)> {
        self.textures
            .iter()
            .map(|(handle, texture)| (*handle, texture.path.as_path()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texture_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("astral-echo-{name}-{}.png", std::process::id()));
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255, 0, 0, 255]).unwrap();
        writer.finish().unwrap();
        path
    }

    #[test]
    fn registers_multiple_material_textures() {
        let mut registry = MaterialTextureRegistry::new();

        let wings = MaterialHandle::new(100);
        let core = MaterialHandle::new(101);
        let path = texture_path("multiple");

        registry.register(wings, &path);
        registry.register(core, &path);

        assert_eq!(registry.len(), 2);
        assert!(registry.contains(wings));
        assert!(registry.contains(core));

        assert_eq!(
            registry.get(wings),
            Some(path.as_path())
        );

        assert_eq!(
            registry.get(core),
            Some(path.as_path())
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn removes_registered_texture() {
        let mut registry = MaterialTextureRegistry::new();
        let material = MaterialHandle::new(100);
        let path = texture_path("remove");

        registry.register(material, &path);

        assert!(registry.remove(material).is_some());
        assert!(!registry.contains(material));
        assert!(registry.is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn repeated_requests_reuse_decoded_texture_data() {
        let path = std::env::temp_dir().join(format!(
            "astral-echo-texture-{}.png",
            std::process::id()
        ));
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        writer.finish().unwrap();

        let material = MaterialHandle::new(101);
        let mut registry = MaterialTextureRegistry::new();
        registry.try_register(material, &path).unwrap();
        let first = registry.image(material).unwrap();
        registry.try_register(material, &path).unwrap();
        let second = registry.image(material).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!((first.width, first.height), (2, 1));
        std::fs::remove_file(path).unwrap();
    }
}

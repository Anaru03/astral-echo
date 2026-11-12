use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::MaterialHandle;

/// Registro de texturas asociado a materiales.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterialTextureRegistry {
    textures: BTreeMap<MaterialHandle, PathBuf>,
}

impl MaterialTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asocia una imagen PNG con un material.
    pub fn register(&mut self, material: MaterialHandle, path: impl Into<PathBuf>) {
        self.textures.insert(material, path.into());
    }

    /// Obtiene la ruta de la textura de un material.
    pub fn get(&self, material: MaterialHandle) -> Option<&Path> {
        self.textures.get(&material).map(PathBuf::as_path)
    }

    /// Elimina una textura registrada.
    pub fn remove(&mut self, material: MaterialHandle) -> Option<PathBuf> {
        self.textures.remove(&material)
    }

    /// Comprueba si un material tiene textura.
    pub fn contains(&self, material: MaterialHandle) -> bool {
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
    pub fn iter(&self) -> impl Iterator<Item = (MaterialHandle, &Path)> {
        self.textures
            .iter()
            .map(|(handle, path)| (*handle, path.as_path()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_multiple_material_textures() {
        let mut registry = MaterialTextureRegistry::new();

        let wings = MaterialHandle::new(100);
        let core = MaterialHandle::new(101);

        registry.register(wings, "textures/wings.png");
        registry.register(core, "textures/core.png");

        assert_eq!(registry.len(), 2);
        assert!(registry.contains(wings));
        assert!(registry.contains(core));

        assert_eq!(registry.get(wings), Some(Path::new("textures/wings.png")));

        assert_eq!(registry.get(core), Some(Path::new("textures/core.png")));
    }

    #[test]
    fn removes_registered_texture() {
        let mut registry = MaterialTextureRegistry::new();
        let material = MaterialHandle::new(100);

        registry.register(material, "textures/example.png");

        assert!(registry.remove(material).is_some());
        assert!(!registry.contains(material));
        assert!(registry.is_empty());
    }
}

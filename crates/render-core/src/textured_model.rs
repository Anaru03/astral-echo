
use std::path::{Path, PathBuf};

use crate::model_loader::build_mesh;
use crate::{ObjLoadError, RasterMesh};

/// Material importado desde un archivo MTL.
#[derive(Clone, Debug)]
pub struct TexturedMaterial {
    pub name: String,
    pub diffuse_texture: Option<PathBuf>,
    pub info: crate::MaterialInfo,
}

/// Submalla asociada a un material.
#[derive(Clone, Debug)]
pub struct TexturedMesh {
    pub name: String,
    pub mesh: RasterMesh,
    pub material_index: Option<usize>,
}

/// Modelo OBJ con sus materiales y submallas.
#[derive(Clone, Debug)]
pub struct TexturedModel {
    pub meshes: Vec<TexturedMesh>,
    pub materials: Vec<TexturedMaterial>,
}

/// Carga un OBJ conservando geometria, UVs y materiales.
pub fn load_textured_obj(
    path: impl AsRef<Path>,
) -> Result<TexturedModel, ObjLoadError> {
    let path = path.as_ref();

    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ignore_points: true,
        ignore_lines: true,
        ..Default::default()
    };

    let (models, materials_result) =
        tobj::load_obj(path, &options)
            .map_err(ObjLoadError::Parse)?;

    let materials = materials_result
        .map_err(ObjLoadError::Parse)?;

    let base = path.parent().unwrap_or_else(|| Path::new("."));

    let materials: Vec<TexturedMaterial> = materials
        .into_iter()
        .map(|material| {
            let diffuse_texture = material
                .diffuse_texture
                .filter(|name| !name.trim().is_empty())
                .map(|name| base.join(name));

            let diffuse = material.diffuse.unwrap_or([1.0, 1.0, 1.0]);
            let emissive = material.emissive.unwrap_or([0.0, 0.0, 0.0]);
            let alpha = material.dissolve.unwrap_or(1.0);
            let shininess = material.shininess.unwrap_or(32.0).max(0.0);

            let roughness = (2.0 / (shininess + 2.0)).sqrt();

            let info = crate::MaterialInfo::new(
                crate::Vec4::new(diffuse[0], diffuse[1], diffuse[2], alpha),
                roughness,
                0.0,
            )
            .with_emissive(crate::Vec3::new(
                emissive[0], emissive[1], emissive[2],
            ));

            TexturedMaterial {
                name: material.name,
                diffuse_texture,
                info,
            }
        })
        .collect();

    let mut meshes = Vec::new();

    for model in models {
        let material_index = model.mesh.material_id;

        if let Some(index) = material_index {
            if index >= materials.len() {
                return Err(ObjLoadError::Invalid(
                    "material index outside material list".into(),
                ));
            }
        }

        let mesh = build_mesh(
            std::iter::once(model.mesh),
        )?;

        meshes.push(TexturedMesh {
            name: model.name,
            mesh,
            material_index,
        });
    }

    if meshes.is_empty() {
        return Err(ObjLoadError::Empty);
    }

    Ok(TexturedModel {
        meshes,
        materials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textured_model_can_store_multiple_materials() {
        let model = TexturedModel {
            meshes: Vec::new(),
            materials: vec![
                TexturedMaterial {
                    name: "Alas".into(),
                    diffuse_texture: Some(
                        PathBuf::from("textures/wings.png"),
                    ),
                    info: crate::MaterialInfo::fixture_metal(),
                },
                TexturedMaterial {
                    name: "Nucleo".into(),
                    diffuse_texture: Some(
                        PathBuf::from("textures/core.png"),
                    ),
                    info: crate::MaterialInfo::fixture_rough(),
                },
            ],
        };

        assert_eq!(model.materials.len(), 2);
        assert_eq!(model.materials[0].name, "Alas");
        assert_eq!(model.materials[1].name, "Nucleo");
    }
}


/// Una submalla registrada, lista para crear un SnapshotNode.
#[derive(Clone, Debug)]
pub struct RegisteredModelPart {
    pub name: String,
    pub mesh: crate::MeshHandle,
    pub material: crate::MaterialHandle,
}

/// Registra las submallas y sus PNG en una SnapshotScene.
/// Usa un rango exclusivo de IDs por modelo; no reutilizarlo entre modelos.
/// La entidad del juego debe crear un nodo por cada parte retornada.
pub fn register_textured_model(
    scene: &mut crate::SnapshotScene,
    path: impl AsRef<Path>,
    first_mesh_id: u32,
    first_material_id: u32,
) -> Result<Vec<RegisteredModelPart>, String> {
    let model = load_textured_obj(path).map_err(|e| e.to_string())?;
    let mut parts = Vec::with_capacity(model.meshes.len());
    for (i, part) in model.meshes.into_iter().enumerate() {
        let offset = u32::try_from(i).map_err(|e| e.to_string())?;
        let mesh_id = first_mesh_id.checked_add(offset)
            .ok_or_else(|| "mesh ID overflow".to_string())?;
        let material_offset = part.material_index
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(offset);
        let material_id = first_material_id.checked_add(material_offset)
            .ok_or_else(|| "material ID overflow".to_string())?;
        let mesh = crate::MeshHandle::new(mesh_id);
        let material = crate::MaterialHandle::new(material_id);
        if let Some(index) = part.material_index {
            scene.set_material(material, model.materials[index].info);
            if let Some(texture) = model.materials[index].diffuse_texture.as_ref() {
                if texture.is_file() {
                    scene.register_material_texture(material, texture.clone());
                } else {
                    eprintln!("textura ausente: {}", texture.display());
                }
            }
        }
        scene.register_mesh(mesh, material, part.mesh);
        parts.push(RegisteredModelPart { name: part.name, mesh, material });
    }
    Ok(parts)
}


/// Expande un nodo en submallas registradas.
///
/// Conserva la transformacion y visibilidad del nodo original.
/// Cada pieza recibe un identificador estable dentro del modelo.
pub fn expand_model_parts(
    original: &crate::SnapshotNode,
    parts: &[RegisteredModelPart],
    parent: u64,
) -> Vec<crate::SnapshotNode> {
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| crate::SnapshotNode {
            entity: crate::SnapshotId::ModelPart {
                parent,
                part: u32::try_from(index)
                    .expect("demasiadas submallas"),
            },
            transform: original.transform,
            mesh: part.mesh,
            material: part.material,
            visible: original.visible,
        })
        .collect()
}

#[cfg(test)]
mod model_expansion_tests {
    use super::*;

    #[test]
    fn model_parts_have_unique_ids_and_preserve_transform() {
        let original = crate::SnapshotNode {
            entity: crate::SnapshotId::Destination(7),
            transform: crate::Transform::IDENTITY,
            mesh: crate::MeshHandle::new(1),
            material: crate::MaterialHandle::new(1),
            visible: true,
        };

        let parts = vec![
            RegisteredModelPart {
                name: "Wings".into(),
                mesh: crate::MeshHandle::new(100),
                material: crate::MaterialHandle::new(200),
            },
            RegisteredModelPart {
                name: "Core".into(),
                mesh: crate::MeshHandle::new(101),
                material: crate::MaterialHandle::new(201),
            },
        ];

        let expanded = expand_model_parts(&original, &parts, 7);

        assert_eq!(expanded.len(), 2);
        assert_ne!(expanded[0].entity, expanded[1].entity);
        assert_eq!(expanded[0].transform, original.transform);
        assert_eq!(expanded[1].transform, original.transform);
        assert_eq!(expanded[0].material, parts[0].material);
        assert_eq!(expanded[1].material, parts[1].material);
    }

    #[test]
    fn empty_model_has_no_parts() {
        let node = crate::SnapshotNode {
            entity: crate::SnapshotId::Destination(1),
            transform: crate::Transform::IDENTITY,
            mesh: crate::MeshHandle::new(1),
            material: crate::MaterialHandle::new(1),
            visible: true,
        };

        let result = expand_model_parts(&node, &[], 42);
        assert!(result.is_empty());
    }
}

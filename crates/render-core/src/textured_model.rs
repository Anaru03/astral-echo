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
pub fn load_textured_obj(path: impl AsRef<Path>) -> Result<TexturedModel, ObjLoadError> {
    let path = path.as_ref();

    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ignore_points: true,
        ignore_lines: true,
        ..Default::default()
    };

    let (models, materials_result) = tobj::load_obj(path, &options).map_err(ObjLoadError::Parse)?;

    let materials = materials_result.map_err(ObjLoadError::Parse)?;

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
            .with_emissive(crate::Vec3::new(emissive[0], emissive[1], emissive[2]));

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

        let mesh = build_mesh(std::iter::once(model.mesh))?;
        if model.name == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_01"
            || model.name == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02"
            || model.name == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_03"
        {
            let mut upper = Vec::new();
            let mut lower = Vec::new();
            for triangle in mesh.indices.chunks_exact(3) {
                let normal = (mesh.vertices[triangle[0] as usize].normal
                    + mesh.vertices[triangle[1] as usize].normal
                    + mesh.vertices[triangle[2] as usize].normal)
                    .normalize_or_zero();
                let centroid = (mesh.vertices[triangle[0] as usize].position
                    + mesh.vertices[triangle[1] as usize].position
                    + mesh.vertices[triangle[2] as usize].position)
                    / 3.0;
                let awakening_bounds = centroid.x >= -28.11
                    && centroid.x <= -11.89
                    && centroid.y >= -21.11
                    && centroid.y <= -10.89;
                let enfermeria_bounds = centroid.x >= -28.11
                    && centroid.x <= 8.11
                    && centroid.y >= 10.89
                    && centroid.y <= 21.11;
                if normal.z < -0.5
                    && (model.name != "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02"
                        || awakening_bounds)
                    && (model.name != "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_03"
                        || enfermeria_bounds)
                {
                    lower.extend_from_slice(triangle);
                } else {
                    upper.extend_from_slice(triangle);
                }
            }
            meshes.push(TexturedMesh {
                name: model.name.clone(),
                mesh: crate::RasterMesh::new(mesh.vertices.clone(), upper),
                material_index,
            });
            let suffix = match model.name.as_str() {
                "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02" => {
                    "__Cara_inferior_despertar"
                }
                "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_03" => {
                    "__Cara_inferior_enfermeria"
                }
                _ => "__Cara_inferior_clonacion",
            };
            meshes.push(TexturedMesh {
                name: format!("{}{}", model.name, suffix),
                mesh: crate::RasterMesh::new(mesh.vertices, lower),
                material_index,
            });
        } else {
            meshes.push(TexturedMesh {
                name: model.name,
                mesh,
                material_index,
            });
        }
    }

    if meshes.is_empty() {
        return Err(ObjLoadError::Empty);
    }

    Ok(TexturedModel { meshes, materials })
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
                    diffuse_texture: Some(PathBuf::from("textures/wings.png")),
                    info: crate::MaterialInfo::fixture_metal(),
                },
                TexturedMaterial {
                    name: "Nucleo".into(),
                    diffuse_texture: Some(PathBuf::from("textures/core.png")),
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
    for (i, mut part) in model.meshes.into_iter().enumerate() {
        let offset = u32::try_from(i).map_err(|e| e.to_string())?;
        let mesh_id = first_mesh_id
            .checked_add(offset)
            .ok_or_else(|| "mesh ID overflow".to_string())?;
        let habitat_ring_wall = part
            .name
            .strip_prefix("ECHO7_30_Exterior_Unificado__Drum_doble_altura_")
            .and_then(|suffix| suffix.parse::<u32>().ok())
            .is_some_and(|segment| (1..=88).contains(&segment))
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let lower_corridor_wall = part
            .name
            .strip_prefix("ECHO7_02_Pasillos__Pared_pasillo_")
            .and_then(|suffix| suffix.parse::<u32>().ok())
            .is_some_and(|segment| (1..=20).contains(&segment))
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let upper_corridor_wall = part
            .name
            .strip_prefix("ECHO7_26_Pasillos_Superiores__Pared_pasillo_")
            .and_then(|suffix| suffix.parse::<u32>().ok())
            .is_some_and(|segment| (1..=20).contains(&segment))
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let cloning_wall = part.name.starts_with("ECHO7_03_Clonacion__Pared_presion_")
            || part.name.starts_with("ECHO7_03_Clonacion__Pared_frente_");
        let cloning_wall = cloning_wall
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let cloning_ceiling = part.name == "ECHO7_03_Clonacion__Dintel_envolvente_01"
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let cloning_losa_underface = (part.name
            == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_01__Cara_inferior_clonacion"
            || part.name
                == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02__Cara_inferior_despertar")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let enfermeria_losa_underface = part.name
            == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_03__Cara_inferior_enfermeria"
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let cloning_cabinet = part.name == "ECHO7_16_Equipo_Clonacion__Banco_genetico_armario_01"
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let cloning_green_panel = (part
            .name
            .starts_with("ECHO7_16_Equipo_Clonacion__Cajon_genetico_")
            || part.name == "ECHO7_16_Equipo_Clonacion__Panel_R04_BANCO_GENETICO_01")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "panel_verde_original.png")
                });
        let awakening_surface = (part.name.starts_with("ECHO7_04_Despertar__Pared_presion_")
            || part.name.starts_with("ECHO7_04_Despertar__Pared_frente_")
            || part.name == "ECHO7_04_Despertar__Dintel_envolvente_01")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let awakening_capsule_shell = (part.name.starts_with("ECHO7_04_Despertar__Capsula_base_")
            || part
                .name
                .starts_with("ECHO7_04_Despertar__Capsula_cabecero_")
            || part
                .name
                .starts_with("ECHO7_04_Despertar__Capsula_lateral_")
            || part.name.starts_with("ECHO7_04_Despertar__Capsula_pie_"))
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let awakening_bench_base = (part.name == "ECHO7_04_Despertar__Banco_base_01"
            || part.name == "ECHO7_04_Despertar__Banco_base_02")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let awakening_planter = (part.name == "ECHO7_04_Despertar__Jardinera_01"
            || part.name == "ECHO7_04_Despertar__Jardinera_02")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let awakening_door_leaf = (part.name == "ECHO7_04_Despertar__Hoja_puerta_aparcada_01"
            || part.name == "ECHO7_04_Despertar__Hoja_puerta_aparcada_02")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "panel_verde_original.png")
                });
        let enfermeria_surface = (part
            .name
            .starts_with("ECHO7_05_Enfermeria_Laboratorio__Pared_presion_")
            || part
                .name
                .starts_with("ECHO7_05_Enfermeria_Laboratorio__Pared_frente_")
            || part.name == "ECHO7_05_Enfermeria_Laboratorio__Dintel_envolvente_01")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let enfermeria_equipment_marfil = (part
            .name
            .starts_with("ECHO7_05_Enfermeria_Laboratorio__Mesa_")
            || part
                .name
                .starts_with("ECHO7_05_Enfermeria_Laboratorio__Armario_")
            || part
                .name
                .starts_with("ECHO7_05_Enfermeria_Laboratorio__Muestra_laboratorio_")
            || part
                .name
                .starts_with("ECHO7_05_Enfermeria_Laboratorio__Tapa_muestra_")
            || part.name == "ECHO7_05_Enfermeria_Laboratorio__Terminal_capsula_pie_01"
            || part.name == "ECHO7_05_Enfermeria_Laboratorio__Terminal_capsula_pie_02")
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "base_marfil.png")
                });
        let enfermeria_equipment_green = (part
            .name
            .starts_with("ECHO7_05_Enfermeria_Laboratorio__CRT_carcasa_")
            || part
                .name
                .starts_with("ECHO7_05_Enfermeria_Laboratorio__Puerta_armario_"))
            && part
                .material_index
                .and_then(|index| model.materials.get(index))
                .and_then(|material| material.diffuse_texture.as_ref())
                .is_some_and(|texture| {
                    texture
                        .file_name()
                        .is_some_and(|name| name == "panel_verde_original.png")
                });
        let panel_verde_original = part
            .material_index
            .and_then(|index| model.materials.get(index))
            .and_then(|material| material.diffuse_texture.as_ref())
            .is_some_and(|texture| {
                texture
                    .file_name()
                    .is_some_and(|name| name == "panel_verde_original.png")
            });
        let repeat_wall = habitat_ring_wall
            || lower_corridor_wall
            || upper_corridor_wall
            || cloning_wall
            || cloning_ceiling
            || cloning_losa_underface
            || enfermeria_losa_underface
            || cloning_cabinet
            || cloning_green_panel
            || awakening_surface
            || awakening_capsule_shell
            || awakening_bench_base
            || awakening_planter
            || awakening_door_leaf
            || enfermeria_surface
            || enfermeria_equipment_marfil
            || enfermeria_equipment_green
            || panel_verde_original;
        if repeat_wall {
            for vertex in &mut part.mesh.vertices {
                if habitat_ring_wall {
                    let angle = vertex.position.y.atan2(vertex.position.x);
                    let radius = vertex.position.x.hypot(vertex.position.y);
                    vertex.uv = crate::Vec2::new(angle * radius * 0.35, vertex.position.z * 0.35);
                } else if cloning_ceiling || cloning_losa_underface || enfermeria_losa_underface {
                    vertex.uv =
                        crate::Vec2::new(vertex.position.x * 0.35, vertex.position.y * 0.35);
                } else if cloning_cabinet || cloning_green_panel || awakening_door_leaf {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.18;
                } else if enfermeria_equipment_marfil {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.20;
                } else if enfermeria_equipment_green {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.18;
                } else if panel_verde_original {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.18;
                } else if awakening_capsule_shell && part.name.contains("__Capsula_lateral_") {
                    let angle = vertex.position.y.atan2(vertex.position.x);
                    let radius = vertex.position.x.hypot(vertex.position.y);
                    vertex.uv = crate::Vec2::new(angle * radius * 0.24, vertex.position.z * 0.24);
                } else if awakening_capsule_shell {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.24;
                } else if awakening_planter {
                    let angle = vertex.position.y.atan2(vertex.position.x);
                    let radius = vertex.position.x.hypot(vertex.position.y);
                    vertex.uv = crate::Vec2::new(angle * radius * 0.20, vertex.position.z * 0.20);
                } else if awakening_bench_base {
                    let normal = vertex.normal.abs();
                    let uv = if normal.z >= normal.x && normal.z >= normal.y {
                        crate::Vec2::new(vertex.position.x, vertex.position.y)
                    } else if normal.x >= normal.y {
                        crate::Vec2::new(vertex.position.y, vertex.position.z)
                    } else {
                        crate::Vec2::new(vertex.position.x, vertex.position.z)
                    };
                    vertex.uv = uv * 0.20;
                } else {
                    let longitudinal = if vertex.normal.x.abs() > vertex.normal.y.abs() {
                        vertex.position.y
                    } else {
                        vertex.position.x
                    };
                    vertex.uv = crate::Vec2::new(longitudinal * 0.35, vertex.position.z * 0.35);
                }
            }
        }
        let material_offset = if repeat_wall {
            u32::try_from(model.materials.len() + i).map_err(|e| e.to_string())?
        } else {
            part.material_index
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(offset)
        };
        let material_id = first_material_id
            .checked_add(material_offset)
            .ok_or_else(|| "material ID overflow".to_string())?;
        let mesh = crate::MeshHandle::new(mesh_id);
        let material = crate::MaterialHandle::new(material_id);
        if let Some(index) = part.material_index {
            scene.set_material(material, model.materials[index].info);
            if let Some(texture) = model.materials[index].diffuse_texture.as_ref() {
                let texture_path = if repeat_wall {
                    if cloning_green_panel || awakening_door_leaf || panel_verde_original {
                        texture.with_file_name("panel_verde_seamless.png")
                    } else {
                        texture.with_file_name("base_marfil_seamless.png")
                    }
                } else {
                    texture.clone()
                };
                if texture_path.is_file() {
                    scene
                        .try_register_material_texture(material, texture_path)
                        .map_err(|error| format!("{error}"))?;
                } else {
                    eprintln!("textura ausente: {}", texture_path.display());
                }
            }
            if repeat_wall {
                scene
                    .set_material_texture_address_mode(material, crate::TextureAddressMode::Repeat);
                println!(
                    "ECHO7 habitat texture object={} material={} handle={} address=Repeat path={}",
                    part.name,
                    model.materials[index].name,
                    material.0,
                    scene
                        .texture_path(material)
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "<missing>".to_owned()),
                );
            }
        }
        scene.register_mesh(mesh, material, part.mesh);
        parts.push(RegisteredModelPart {
            name: part.name,
            mesh,
            material,
        });
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
                part: u32::try_from(index).expect("demasiadas submallas"),
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

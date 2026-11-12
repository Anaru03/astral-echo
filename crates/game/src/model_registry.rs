use render_core::{MaterialHandle, MeshHandle, VisualAssets};

pub struct ModelDefinition {
    pub path: &'static str,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

pub fn registered_models() -> Vec<ModelDefinition> {
    vec![
        ModelDefinition {
            path: "characters/astronauta/astronauta.obj",
            mesh: MeshHandle::ASTRONAUT,
            material: VisualAssets::PLAYER,
        },
        ModelDefinition {
            path: "echo_7/componentes/ECHO7_Exterior.obj",
            mesh: MeshHandle::SHIP_PLACEHOLDER,
            material: VisualAssets::SHIP,
        },
        ModelDefinition {
            path: "echo_7/componentes/ECHO7_Interior_Inferior.obj",
            mesh: MeshHandle::ECHO7_INTERIOR_LOWER,
            material: VisualAssets::SHIP,
        },
        ModelDefinition {
            path: "echo_7/componentes/ECHO7_Interior_Superior.obj",
            mesh: MeshHandle::ECHO7_INTERIOR_UPPER,
            material: VisualAssets::SHIP,
        },
        ModelDefinition {
            path: "echo_7/componentes/ECHO7_Nucleo_Entre_Cubiertas.obj",
            mesh: MeshHandle::ECHO7_INTERIOR_CORE,
            material: VisualAssets::SHIP,
        },
        ModelDefinition {
            path: "station/station.obj",
            mesh: MeshHandle::STATION,
            material: VisualAssets::STATION,
        },
        ModelDefinition {
            path: "shooter/shooter.obj",
            mesh: MeshHandle::ENEMY_PLACEHOLDER,
            material: VisualAssets::ENEMY,
        },
        ModelDefinition {
            path: "charger/charger.obj",
            mesh: MeshHandle::ENEMY_PLACEHOLDER,
            material: VisualAssets::ENEMY_CHARGER,
        },
        ModelDefinition {
            path: "sun/sun.obj",
            mesh: MeshHandle::SUN_MODEL,
            material: VisualAssets::SUN,
        },
        ModelDefinition {
            path: "shielded/shielded.obj",
            mesh: MeshHandle::ENEMY_PLACEHOLDER,
            material: VisualAssets::ENEMY_SHIELDED,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo7_interior_core_registers_for_visual_snapshot() {
        let definition = registered_models()
            .into_iter()
            .find(|definition| definition.mesh == MeshHandle::ECHO7_INTERIOR_CORE)
            .unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models")
            .join(definition.path);
        let mut scene = render_core::SnapshotScene::default();
        let parts = render_core::register_textured_model(&mut scene, path, 100_000, 100_000)
            .expect("ECHO-7 interior core should register");
        assert!(!parts.is_empty());
    }

    #[test]
    fn echo7_component_strategy_registers_structure_and_three_interior_sources() {
        let definitions = registered_models();
        let paths: Vec<_> = definitions
            .iter()
            .filter(|definition| {
                matches!(
                    definition.mesh,
                    MeshHandle::SHIP_PLACEHOLDER
                        | MeshHandle::ECHO7_INTERIOR_LOWER
                        | MeshHandle::ECHO7_INTERIOR_UPPER
                        | MeshHandle::ECHO7_INTERIOR_CORE
                )
            })
            .map(|definition| definition.path)
            .collect();
        assert_eq!(paths[0], "echo_7/componentes/ECHO7_Exterior.obj");
        assert_eq!(paths.len(), 4);
        assert!(paths.iter().all(|path| !path.contains("montajes")));
    }

    #[test]
    fn echo7_component_interiors_require_materials_and_uvs() {
        for path in [
            "echo_7/componentes/ECHO7_Interior_Inferior.obj",
            "echo_7/componentes/ECHO7_Interior_Superior.obj",
            "echo_7/componentes/ECHO7_Nucleo_Entre_Cubiertas.obj",
        ] {
            let full_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/models")
                .join(path);
            let model = render_core::load_textured_obj(&full_path)
                .expect("component interior must load with its MTL");
            assert!(!model.materials.is_empty());
            assert!(model.meshes.iter().all(|mesh| {
                mesh.material_index.is_some()
                    && mesh
                        .mesh
                        .vertices
                        .iter()
                        .any(|vertex| vertex.uv != render_core::Vec2::ZERO)
            }));
        }
    }
}

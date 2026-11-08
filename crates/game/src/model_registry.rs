use render_core::{MaterialHandle, MeshHandle, VisualAssets};

pub struct ModelDefinition {
    pub path: &'static str,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

pub fn registered_models() -> Vec<ModelDefinition> {
    vec![
        ModelDefinition {
            path: "echo_7/echo-7.obj",
            mesh: MeshHandle::SHIP_PLACEHOLDER,
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
            path: "shielded/shielded.obj",
            mesh: MeshHandle::ENEMY_PLACEHOLDER,
            material: VisualAssets::ENEMY_SHIELDED,
        },
    ]
}

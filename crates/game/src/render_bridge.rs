use engine::EntityId;
use gameplay::GameplayWorld;
use render_core::{
    from_engine, Camera, MaterialHandle, MeshHandle, RenderSnapshot, SnapshotLight, SnapshotNode,
    Transform,
};

pub const SHIP_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ENEMY_MESH: MeshHandle = MeshHandle::ENEMY_PLACEHOLDER;
pub const PLAYER_MESH: MeshHandle = MeshHandle::CUBE;
pub const PLACEHOLDER_MESH: MeshHandle = MeshHandle::CUBE;

pub const SHIP_MATERIAL: MaterialHandle = MaterialHandle::new(0);
pub const ENEMY_MATERIAL: MaterialHandle = MaterialHandle::new(1);
pub const PLAYER_MATERIAL: MaterialHandle = MaterialHandle::new(2);
pub const PLACEHOLDER_MATERIAL: MaterialHandle = MaterialHandle::new(3);

#[derive(Clone, Copy, Debug)]
pub struct BridgeParams {
    pub destinations: &'static [gameplay::DestinationId],
    pub include_collectibles: bool,
}

impl Default for BridgeParams {
    fn default() -> Self {
        Self {
            destinations: &[],
            include_collectibles: true,
        }
    }
}

pub fn snapshot_from_world(world: &GameplayWorld, params: BridgeParams) -> RenderSnapshot {
    let camera = Camera::demo();
    let tick = world.time().tick;
    let mut snapshot = RenderSnapshot::new(
        tick,
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );

    if let Some(pos) = world.ship_position() {
        snapshot.push_node(SnapshotNode::new(
            EntityId::new(0),
            Transform::from_translation(from_engine(pos)),
            SHIP_MESH,
            SHIP_MATERIAL,
            true,
        ));
    }

    for player in world.players() {
        snapshot.push_node(SnapshotNode::new(
            player.entity_id,
            Transform::from_translation(from_engine(player.position)),
            PLAYER_MESH,
            PLAYER_MATERIAL,
            player.is_active,
        ));
    }

    for enemy in world.enemies() {
        if enemy.is_player {
            continue;
        }
        let visible = enemy.active && !enemy.is_defeated();
        snapshot.push_node(SnapshotNode::new(
            enemy.entity,
            Transform::from_translation(from_engine(enemy.position)),
            ENEMY_MESH,
            ENEMY_MATERIAL,
            visible,
        ));
    }

    for id in params.destinations {
        if let Some(dest) = world.destination(*id) {
            snapshot.push_node(SnapshotNode::new(
                EntityId::new(9_000 + dest.id.value() as u64),
                Transform::from_translation(from_engine(dest.position)),
                PLACEHOLDER_MESH,
                PLACEHOLDER_MATERIAL,
                true,
            ));
        }
    }

    if params.include_collectibles {
        for c in world.collectibles() {
            if !c.active {
                continue;
            }
            snapshot.push_node(SnapshotNode::new(
                c.entity,
                Transform::from_translation(from_engine(c.position)),
                PLACEHOLDER_MESH,
                PLACEHOLDER_MATERIAL,
                true,
            ));
        }
    }

    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_point()));
    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_sun()));

    snapshot
}

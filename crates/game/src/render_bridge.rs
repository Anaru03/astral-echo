use gameplay::GameplayWorld;
use render_core::{
    from_engine, Camera, MaterialHandle, MeshHandle, Quat, RenderSnapshot, SnapshotLight,
    SnapshotNode, Transform, Vec3, VisualAssets,
};

pub const SHIP_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ENEMY_MESH: MeshHandle = MeshHandle::ENEMY_PLACEHOLDER;
pub const PLAYER_MESH: MeshHandle = MeshHandle::CUBE;
pub const PLACEHOLDER_MESH: MeshHandle = MeshHandle::CUBE;
pub const DESTINATION_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const COLLECTIBLE_MESH: MeshHandle = MeshHandle::COLLECTIBLE_PLACEHOLDER;

pub const SHIP_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ENEMY_MATERIAL: MaterialHandle = VisualAssets::ENEMY;
pub const PLAYER_MATERIAL: MaterialHandle = VisualAssets::PLAYER;
pub const PLACEHOLDER_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const DESTINATION_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const COLLECTIBLE_MATERIAL: MaterialHandle = VisualAssets::COLLECTIBLE;

#[derive(Clone, Copy, Debug)]
pub struct BridgeParams<'a> {
    pub destinations: &'a [gameplay::DestinationId],
    pub include_collectibles: bool,
}

impl Default for BridgeParams<'_> {
    fn default() -> Self {
        Self {
            destinations: &[],
            include_collectibles: true,
        }
    }
}

pub fn snapshot_from_world(world: &GameplayWorld, params: BridgeParams<'_>) -> RenderSnapshot {
    snapshot_from_world_with_camera(world, params, &Camera::demo())
}

pub fn snapshot_from_world_with_camera(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
) -> RenderSnapshot {
    let tick = world.time().tick;
    let mut snapshot = RenderSnapshot::new(
        tick,
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );

    if let (Some(ship), Some(pos)) = (world.navigation().ship_id(), world.ship_position()) {
        let orientation = world.ship_orientation().unwrap_or_default();
        snapshot.push_node(SnapshotNode::new(
            ship,
            visual_transform(pos, orientation.yaw_degrees, orientation.pitch_degrees),
            SHIP_MESH,
            SHIP_MATERIAL,
            world.entities().contains(ship),
        ));
    }

    for player in world.players() {
        snapshot.push_node(SnapshotNode::new(
            player.entity_id,
            visual_transform(
                player.position,
                player.orientation.yaw_degrees,
                player.orientation.pitch_degrees,
            ),
            PLAYER_MESH,
            PLAYER_MATERIAL,
            player.is_active && world.entities().contains(player.entity_id),
        ));
    }

    for enemy in world.enemies() {
        if enemy.is_player {
            continue;
        }
        let visible =
            enemy.active && !enemy.is_defeated() && world.entities().contains(enemy.entity);
        snapshot.push_node(SnapshotNode::new(
            enemy.entity,
            Transform::from_translation(from_engine(enemy.position)),
            ENEMY_MESH,
            ENEMY_MATERIAL,
            visible,
        ));
    }

    for dest in world.destinations() {
        if params.destinations.is_empty() || params.destinations.contains(&dest.id) {
            snapshot.push_node(SnapshotNode::destination(
                dest.id.value(),
                Transform::from_translation(from_engine(dest.position)),
                DESTINATION_MESH,
                DESTINATION_MATERIAL,
                dest.discovery != gameplay::DiscoveryState::Unknown,
            ));
        }
    }

    if params.include_collectibles {
        for c in world.collectibles() {
            snapshot.push_node(SnapshotNode::new(
                c.entity,
                Transform::from_translation(from_engine(c.position)),
                COLLECTIBLE_MESH,
                COLLECTIBLE_MATERIAL,
                c.active && world.entities().contains(c.entity),
            ));
        }
    }

    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_point()));
    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_sun()));

    snapshot
}

fn visual_transform(position: engine::Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::new(
        from_engine(position),
        Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(pitch.to_radians()),
        Vec3::ONE,
    )
}

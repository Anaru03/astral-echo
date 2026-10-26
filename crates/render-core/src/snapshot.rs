use engine::{EntityId, Tick};

use crate::material::MaterialHandle;
use crate::mesh::MeshHandle;
use crate::scene::RenderLight;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotNode {
    pub entity: SnapshotId,
    pub transform: Transform,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub visible: bool,
}

impl SnapshotNode {
    pub fn new(
        entity: EntityId,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
        visible: bool,
    ) -> Self {
        Self {
            entity: SnapshotId::Entity(entity),
            transform,
            mesh,
            material,
            visible,
        }
    }

    pub fn destination(
        id: u32,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
        visible: bool,
    ) -> Self {
        Self {
            entity: SnapshotId::Destination(id),
            transform,
            mesh,
            material,
            visible,
        }
    }

    pub fn derelict(
        id: u32,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
        visible: bool,
    ) -> Self {
        Self {
            entity: SnapshotId::Derelict(id),
            transform,
            mesh,
            material,
            visible,
        }
    }

    pub fn hazard(
        id: u32,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
        visible: bool,
    ) -> Self {
        Self {
            entity: SnapshotId::Hazard(id),
            transform,
            mesh,
            material,
            visible,
        }
    }

    pub fn visual_event(
        id: u64,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Self {
        Self {
            entity: SnapshotId::VisualEvent(id),
            transform,
            mesh,
            material,
            visible: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SnapshotId {
    Entity(EntityId),
    Destination(u32),
    Derelict(u32),
    Hazard(u32),
    VisualEvent(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotLight {
    pub light: RenderLight,
}

impl SnapshotLight {
    pub fn new(light: RenderLight) -> Self {
        Self { light }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderSnapshot {
    pub tick: Tick,
    pub simulation_time: engine::Duration,
    pub nodes: Vec<SnapshotNode>,
    pub visual_events: Vec<RenderVisualEvent>,
    pub lights: Vec<SnapshotLight>,
    pub camera_position: crate::math::Vec3,
    pub view: crate::math::Mat4,
    pub projection: crate::math::Mat4,
    pub overlay: RenderOverlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderVisualEventKind {
    Attack,
    EnemyDefeated,
    ItemCollected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderVisualEvent {
    pub id: u64,
    pub kind: RenderVisualEventKind,
    pub source: Option<EntityId>,
    pub target: Option<EntityId>,
    pub position: Option<crate::math::Vec3>,
    pub emitted_at: engine::Duration,
    pub expires_at: engine::Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderOverlay {
    pub fade_to_black: f32,
    pub game_over: bool,
    pub hud: HudState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HudFlightState {
    #[default]
    Free,
    Atmospheric,
    Landing,
    Landed,
    Impacted,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HudAutopilotState {
    #[default]
    Inactive,
    Planned,
    InTransit,
    Completed,
    Interrupted,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HudState {
    pub flight: HudFlightState,
    pub autopilot: HudAutopilotState,
    pub target: Option<u32>,
    pub active_enemies: u32,
    pub landing_available: bool,
}

pub const GAME_OVER_GLYPHS: [[u8; 7]; 8] = [
    [14, 16, 16, 23, 17, 17, 14],
    [14, 17, 17, 31, 17, 17, 17],
    [17, 27, 21, 21, 17, 17, 17],
    [31, 16, 16, 30, 16, 16, 31],
    [14, 17, 17, 17, 17, 17, 14],
    [17, 17, 10, 4, 4, 4, 4],
    [31, 16, 16, 30, 16, 16, 31],
    [30, 17, 17, 30, 20, 18, 17],
];

pub fn game_over_layout(width: u32, height: u32) -> (u32, u32, u32) {
    const COLUMNS: u32 = 8 * 5 + 7 + 2;
    let scale = (width / (COLUMNS + 2)).min(height / 9).max(1);
    let text_width = COLUMNS * scale;
    let text_height = 7 * scale;
    (
        width.saturating_sub(text_width) / 2,
        height.saturating_sub(text_height) / 2,
        scale,
    )
}

impl RenderSnapshot {
    pub fn new(
        tick: Tick,
        camera_position: crate::math::Vec3,
        view: crate::math::Mat4,
        projection: crate::math::Mat4,
    ) -> Self {
        Self {
            tick,
            simulation_time: engine::Duration(0),
            nodes: Vec::new(),
            visual_events: Vec::new(),
            lights: Vec::new(),
            camera_position,
            view,
            projection,
            overlay: RenderOverlay::default(),
        }
    }

    pub fn view_projection(&self) -> crate::math::Mat4 {
        self.projection * self.view
    }

    pub fn push_node(&mut self, node: SnapshotNode) {
        if let Some(slot) = self.nodes.iter_mut().find(|n| n.entity == node.entity) {
            *slot = node;
        } else {
            self.nodes.push(node);
        }
    }

    pub fn push_light(&mut self, light: SnapshotLight) {
        self.lights.push(light);
    }

    pub fn minimal_demo() -> Self {
        use crate::camera::Camera;
        let camera = Camera::demo();
        let mut snap = Self::new(
            Tick(1),
            camera.position(),
            camera.view_matrix(),
            camera.projection_matrix(),
        );
        snap.push_node(SnapshotNode::new(
            EntityId::new(1),
            Transform::IDENTITY,
            MeshHandle::SHIP_PLACEHOLDER,
            MaterialHandle::new(0),
            true,
        ));
        snap.push_node(SnapshotNode::new(
            EntityId::new(2),
            Transform::from_translation(crate::math::Vec3::new(18.0, 0.0, -18.0)),
            MeshHandle::ENEMY_PLACEHOLDER,
            MaterialHandle::new(1),
            true,
        ));
        snap.push_light(SnapshotLight::new(RenderLight::demo_point()));
        snap
    }
}
use engine::{EntityId, Tick};

use crate::material::MaterialHandle;
use crate::mesh::MeshHandle;
use crate::scene::RenderLight;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotNode {
    pub entity: EntityId,
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
            entity,
            transform,
            mesh,
            material,
            visible,
        }
    }
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
    pub nodes: Vec<SnapshotNode>,
    pub lights: Vec<SnapshotLight>,
    pub camera_position: crate::math::Vec3,
    pub view: crate::math::Mat4,
    pub projection: crate::math::Mat4,
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
            nodes: Vec::new(),
            lights: Vec::new(),
            camera_position,
            view,
            projection,
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

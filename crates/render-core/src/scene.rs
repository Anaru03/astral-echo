use engine::EntityId;

use crate::bounds::Aabb;
use crate::camera::Camera;
use crate::material::MaterialHandle;
use crate::math::Vec3;
use crate::mesh::MeshHandle;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderNode {
    pub entity: EntityId,
    pub transform: Transform,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub visible: bool,
    pub bounds: Option<Aabb>,
}

impl RenderNode {
    pub fn new(
        entity: EntityId,
        transform: Transform,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Self {
        Self {
            entity,
            transform,
            mesh,
            material,
            visible: true,
            bounds: None,
        }
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    pub fn with_bounds(mut self, bounds: Aabb) -> Self {
        self.bounds = Some(bounds);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderLightKind {
    Directional { direction: Vec3 },
    Point { position: Vec3, range: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderLight {
    pub color: Vec3,
    pub intensity: f32,
    pub kind: RenderLightKind,
    pub casts_shadow: bool,
}

impl RenderLight {
    pub fn demo_point() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
            kind: RenderLightKind::Point {
                position: Vec3::new(8.0, 26.0, 18.0),
                range: 180.0,
            },
            casts_shadow: false,
        }
    }

    pub fn demo_sun() -> Self {
        Self {
            color: Vec3::new(1.0, 0.98, 0.95),
            intensity: 1.2,
            kind: RenderLightKind::Directional {
                direction: Vec3::new(-0.3, -1.0, -0.2).normalize(),
            },
            casts_shadow: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderCamera {
    pub camera: Camera,
}

impl RenderCamera {
    pub fn new(camera: Camera) -> Self {
        Self { camera }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderScene {
    pub nodes: Vec<RenderNode>,
    pub lights: Vec<RenderLight>,
    pub camera: Option<RenderCamera>,
}

impl RenderScene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_node(&mut self, node: RenderNode) {
        if let Some(slot) = self.nodes.iter_mut().find(|n| n.entity == node.entity) {
            *slot = node;
        } else {
            self.nodes.push(node);
        }
    }

    pub fn push_light(&mut self, light: RenderLight) {
        self.lights.push(light);
    }

    pub fn set_camera(&mut self, camera: RenderCamera) {
        self.camera = Some(camera);
    }

    pub fn visible_nodes(&self) -> impl Iterator<Item = &RenderNode> {
        self.nodes.iter().filter(|n| n.visible)
    }
}

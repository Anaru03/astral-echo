use engine::{Duration, EntityId, Vec3};
use crate::types::PlayerId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orientation {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
}

impl Default for Orientation {
    fn default() -> Self {
        Self {
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementParameters {
    pub max_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
}

impl Default for MovementParameters {
    fn default() -> Self {
        Self {
            max_speed: 5.0,
            acceleration: 20.0,
            deceleration: 24.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub entity_id: EntityId,
    pub position: Vec3,
    pub orientation: Orientation,
    pub velocity: Vec3,
    pub is_active: bool,
    pub movement: MovementParameters,
}

impl Player {
    pub fn new(entity_id: EntityId) -> Self {
        Self {
            entity_id,
            position: Vec3::ZERO,
            orientation: Orientation::default(),
            velocity: Vec3::ZERO,
            is_active: true,
            movement: MovementParameters::default(),
        }
    }

    pub fn id(&self) -> PlayerId {
        PlayerId(self.entity_id)
    }
}

pub(crate) fn duration_seconds(delta: Duration) -> f32 {
    delta.0 as f32 / 1000.0
}

pub(crate) fn is_finite_vec3(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

pub(crate) fn magnitude_squared(value: Vec3) -> f32 {
    value.x * value.x + value.y * value.y + value.z * value.z
}

pub(crate) fn normalize_or_zero(value: Vec3) -> Vec3 {
    let magnitude = magnitude_squared(value).sqrt();
    if magnitude > 0.0 {
        Vec3 {
            x: value.x / magnitude,
            y: value.y / magnitude,
            z: value.z / magnitude,
        }
    } else {
        Vec3::ZERO
    }
}

pub(crate) fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let delta = Vec3 {
        x: target.x - current.x,
        y: target.y - current.y,
        z: target.z - current.z,
    };
    let distance = magnitude_squared(delta).sqrt();
    if distance <= max_delta || distance == 0.0 {
        target
    } else {
        Vec3 {
            x: current.x + delta.x / distance * max_delta,
            y: current.y + delta.y / distance * max_delta,
            z: current.z + delta.z / distance * max_delta,
        }
    }
}
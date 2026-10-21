use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NetworkVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl NetworkVec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn is_valid(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NetworkOrientation {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
}

impl NetworkOrientation {
    pub fn new(yaw_degrees: f32, pitch_degrees: f32) -> Self {
        Self {
            yaw_degrees,
            pitch_degrees,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.yaw_degrees.is_finite() && self.pitch_degrees.is_finite()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PlayerTransform {
    pub position: NetworkVec3,
    pub orientation: NetworkOrientation,
    pub velocity: NetworkVec3,
}

impl PlayerTransform {
    pub fn new(
        position: NetworkVec3,
        orientation: NetworkOrientation,
        velocity: NetworkVec3,
    ) -> Self {
        Self {
            position,
            orientation,
            velocity,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.position.is_valid() && self.orientation.is_valid() && self.velocity.is_valid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_position() {
        let position = NetworkVec3::new(1.0, 2.0, 3.0);
        assert!(position.is_valid());
    }

    #[test]
    fn test_invalid_position() {
        let position = NetworkVec3::new(f32::NAN, 2.0, 3.0);
        assert!(!position.is_valid());
    }

    #[test]
    fn test_valid_orientation() {
        let orientation = NetworkOrientation::new(90.0, 45.0);
        assert!(orientation.is_valid());
    }

    #[test]
    fn test_invalid_orientation() {
        let orientation = NetworkOrientation::new(f32::INFINITY, 0.0);
        assert!(!orientation.is_valid());
    }

    #[test]
    fn test_valid_transform() {
        let transform = PlayerTransform::new(
            NetworkVec3::new(1.0, 2.0, 3.0),
            NetworkOrientation::new(90.0, 0.0),
            NetworkVec3::new(0.0, 0.0, 1.0),
        );

        assert!(transform.is_valid());
    }

    #[test]
    fn test_invalid_transform() {
        let transform = PlayerTransform::new(
            NetworkVec3::new(1.0, 2.0, 3.0),
            NetworkOrientation::new(0.0, f32::NAN),
            NetworkVec3::new(0.0, 0.0, 0.0),
        );

        assert!(!transform.is_valid());
    }
}

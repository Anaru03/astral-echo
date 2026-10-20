use crate::math::{perspective_rh_wgpu, Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Perspective {
        fov_y_rad: f32,
        aspect: f32,
        near: f32,
        far: f32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraError {
    InvalidFov,
    InvalidAspect,
    InvalidPlanes,
    InvalidUp,
}

impl Projection {
    fn validate(self) -> Result<(), CameraError> {
        match self {
            Projection::Perspective {
                fov_y_rad,
                aspect,
                near,
                far,
            } => {
                if !(fov_y_rad > 0.0 && fov_y_rad < core::f32::consts::PI) {
                    return Err(CameraError::InvalidFov);
                }
                if !(aspect > 0.0 && aspect.is_finite()) {
                    return Err(CameraError::InvalidAspect);
                }
                if !(near > 0.0 && far > near && near.is_finite() && far.is_finite()) {
                    return Err(CameraError::InvalidPlanes);
                }
                Ok(())
            }
        }
    }

    fn matrix(self) -> Mat4 {
        match self {
            Projection::Perspective {
                fov_y_rad,
                aspect,
                near,
                far,
            } => perspective_rh_wgpu(fov_y_rad, aspect, near, far),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    position: Vec3,
    target: Vec3,
    up: Vec3,
    projection: Projection,
}

impl Camera {
    pub fn new(
        position: Vec3,
        target: Vec3,
        up: Vec3,
        projection: Projection,
    ) -> Result<Self, CameraError> {
        projection.validate()?;
        if up.length_squared() < f32::EPSILON || !up.is_finite() {
            return Err(CameraError::InvalidUp);
        }
        Ok(Self {
            position,
            target,
            up: up.normalize(),
            projection,
        })
    }

    pub fn demo() -> Self {
        Self {
            position: Vec3::new(0.0, 38.0, 62.0),
            target: Vec3::new(18.0, 0.0, -18.0),
            up: Vec3::Y,
            projection: Projection::Perspective {
                fov_y_rad: 60.0_f32.to_radians(),
                aspect: 16.0 / 9.0,
                near: 0.1,
                far: 500.0,
            },
        }
    }

    pub fn position(self) -> Vec3 {
        self.position
    }

    pub fn projection(self) -> Projection {
        self.projection
    }

    pub fn set_aspect(&mut self, aspect: f32) -> Result<(), CameraError> {
        match &mut self.projection {
            Projection::Perspective { aspect: a, .. } => {
                if !(aspect > 0.0 && aspect.is_finite()) {
                    return Err(CameraError::InvalidAspect);
                }
                *a = aspect;
                Ok(())
            }
        }
    }

    pub fn view_matrix(self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn projection_matrix(self) -> Mat4 {
        self.projection.matrix()
    }

    pub fn view_projection_matrix(self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

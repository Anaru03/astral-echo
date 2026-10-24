pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

#[inline]
pub fn from_engine(v: engine::Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

#[inline]
pub fn to_engine(v: Vec3) -> engine::Vec3 {
    engine::Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

pub fn normal_matrix(model: Mat4) -> Mat4 {
    let determinant = model.determinant();
    if determinant.is_finite() && determinant != 0.0 {
        let matrix = model.inverse().transpose();
        if matrix.is_finite() {
            return matrix;
        }
    }
    Mat4::ZERO
}

pub fn transform_normal(model: Mat4, normal: Vec3) -> Vec3 {
    normalize_direction(normal_matrix(model).transform_vector3(normal))
}

pub fn perspective_rh_wgpu(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (0.5 * fov_y_rad).tan();
    Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, far / (near - far), -1.0),
        Vec4::new(0.0, 0.0, -(far * near) / (far - near), 0.0),
    )
}

pub fn normalize_direction(vector: Vec3) -> Vec3 {
    if !vector.is_finite() {
        return Vec3::ZERO;
    }
    let scale = vector.abs().max_element();
    if scale > 0.0 {
        (vector / scale).normalize_or_zero()
    } else {
        Vec3::ZERO
    }
}

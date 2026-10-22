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

pub fn transform_normal(model: Mat4, normal: Vec3) -> Vec3 {
    let normal_matrix = model.inverse().transpose();
    let transformed = normal_matrix.transform_vector3(normal);
    if transformed.length_squared() > f32::EPSILON {
        transformed.normalize()
    } else {
        Vec3::Z
    }
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

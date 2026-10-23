use crate::framebuffer::Color;

pub fn float_to_u8(value: f32) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    let clamped = value.clamp(0.0, 1.0);
    (clamped * 255.0).round() as u8
}

pub fn vec4_to_rgba8(color: glam::Vec4) -> Color {
    Color::new(
        float_to_u8(color.x),
        float_to_u8(color.y),
        float_to_u8(color.z),
        float_to_u8(color.w),
    )
}

pub fn vec3_to_rgb8(color: glam::Vec3) -> Color {
    Color::new(
        float_to_u8(color.x),
        float_to_u8(color.y),
        float_to_u8(color.z),
        255,
    )
}

pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mix = |x: u8, y: u8| -> u8 {
        let v = f32::from(x) + (f32::from(y) - f32::from(x)) * t;
        (v.round().clamp(0.0, 255.0)) as u8
    };
    Color::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b), mix(a.a, b.a))
}

use glam::Vec3;
use render_cpu::{
    float_to_u8, lerp_color, vec3_to_rgb8, vec4_to_rgba8, write_png_bytes, Color, Framebuffer,
    RasterError,
};

#[test]
fn framebuffer_rejects_zero_dimensions() {
    assert_eq!(
        Framebuffer::new(0, 10).unwrap_err(),
        RasterError::InvalidFramebufferSize
    );
    assert_eq!(
        Framebuffer::new(10, 0).unwrap_err(),
        RasterError::InvalidFramebufferSize
    );
}

#[test]
fn framebuffer_starts_cleared_to_black_with_max_depth() {
    let f = Framebuffer::new(4, 3).expect("valid framebuffer");
    assert_eq!(f.width(), 4);
    assert_eq!(f.height(), 3);
    assert_eq!(f.pixel_count(), 12);
    assert_eq!(f.color_bytes().len(), 48);
    assert!(f.color_bytes().iter().all(|b| *b == 0));
    assert_eq!(f.depth_at(0, 0), Some(1.0));
    assert_eq!(f.depth_at(3, 2), Some(1.0));
}

#[test]
fn framebuffer_out_of_range_access_returns_none() {
    let mut f = Framebuffer::new(4, 3).expect("valid framebuffer");
    assert!(f.pixel(4, 0).is_none());
    assert!(f.pixel(0, 3).is_none());
    assert!(f.depth_at(4, 0).is_none());
    assert!(!f.set_pixel(4, 0, Color::WHITE));
}

#[test]
fn framebuffer_set_and_read_pixel_roundtrips() {
    let mut f = Framebuffer::new(4, 3).expect("valid framebuffer");
    assert!(f.set_pixel(2, 1, Color::new(10, 20, 30, 40)));
    assert_eq!(f.pixel(2, 1), Some(Color::new(10, 20, 30, 40)));
}

#[test]
fn framebuffer_clear_resets_color_and_depth() {
    let mut f = Framebuffer::new(4, 3).expect("valid framebuffer");
    f.set_pixel(1, 1, Color::new(255, 255, 255, 255));
    f.set_depth(1, 1, 0.25);
    f.clear(Color::new(1, 2, 3, 4));
    assert_eq!(f.pixel(1, 1), Some(Color::new(1, 2, 3, 4)));
    assert_eq!(f.depth_at(1, 1), Some(1.0));
}

#[test]
fn framebuffer_resize_reallocates_buffers() {
    let mut f = Framebuffer::new(4, 3).expect("valid framebuffer");
    f.set_pixel(1, 1, Color::WHITE);
    f.resize(8, 6).expect("valid resize");
    assert_eq!(f.width(), 8);
    assert_eq!(f.height(), 6);
    assert_eq!(f.pixel_count(), 48);
    assert_eq!(f.color_bytes().len(), 192);
    assert_eq!(f.pixel(1, 1), Some(Color::TRANSPARENT));
}

#[test]
fn framebuffer_resize_rejects_invalid_dimensions() {
    let mut f = Framebuffer::new(4, 3).expect("valid framebuffer");
    assert!(f.resize(0, 3).is_err());
    assert!(f.resize(4, 0).is_err());
    assert_eq!(f.width(), 4);
}

#[test]
fn float_to_u8_clamps_and_rounds() {
    assert_eq!(float_to_u8(0.0), 0);
    assert_eq!(float_to_u8(1.0), 255);
    assert_eq!(float_to_u8(0.5), 128);
    assert_eq!(float_to_u8(-1.0), 0);
    assert_eq!(float_to_u8(2.0), 255);
    assert_eq!(float_to_u8(f32::NAN), 0);
    assert_eq!(float_to_u8(f32::INFINITY), 0);
}

#[test]
fn vec4_to_rgba8_preserves_alpha() {
    let c = vec4_to_rgba8(glam::Vec4::new(1.0, 0.0, 0.5, 1.0));
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 128);
    assert_eq!(c.a, 255);
}

#[test]
fn vec3_to_rgb8_is_opaque() {
    let c = vec3_to_rgb8(Vec3::new(0.0, 1.0, 0.0));
    assert_eq!(c.g, 255);
    assert_eq!(c.a, 255);
}

#[test]
fn lerp_color_interpolates_endpoints() {
    let a = Color::new(0, 0, 0, 0);
    let b = Color::new(255, 255, 255, 255);
    assert_eq!(lerp_color(a, b, 0.0), a);
    assert_eq!(lerp_color(a, b, 1.0), b);
    assert_eq!(lerp_color(a, b, 0.5), Color::new(128, 128, 128, 128));
    assert_eq!(lerp_color(a, b, -5.0), a);
    assert_eq!(lerp_color(a, b, 5.0), b);
}

#[test]
fn png_bytes_are_reproducible_and_decodable() {
    let mut f = Framebuffer::new(8, 6).expect("valid framebuffer");
    f.set_pixel(3, 2, Color::new(255, 128, 0, 255));
    let a = write_png_bytes(&f).expect("encodes");
    let b = write_png_bytes(&f).expect("encodes");
    assert_eq!(a, b, "png encoding must be deterministic");
    assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n");

    let decoder = png::Decoder::new(std::io::Cursor::new(&a));
    let mut reader = decoder.read_info().expect("decodes header");
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("decodes frame");
    assert_eq!(info.width, 8);
    assert_eq!(info.height, 6);
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let offset = (2 * 8 + 3) * 4;
    assert_eq!(&buf[offset..offset + 4], &[255, 128, 0, 255]);
}

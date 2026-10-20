pub mod clip;
pub mod color;
pub mod error;
pub mod framebuffer;
pub mod geometry;
pub mod image;
pub mod pipeline;
pub mod rasterizer;
pub mod viewport;

pub use clip::{
    clip_polygon, clip_polygon_near, clip_triangle, clip_triangle_near, ClipPlane, ClipVertex,
};
pub use color::{float_to_u8, lerp_color, vec3_to_rgb8, vec4_to_rgba8};
pub use error::{ImageError, RasterError};
pub use framebuffer::{Color, Framebuffer};
pub use geometry::{RasterMesh, RasterVertex};
pub use image::{write_png, write_png_bytes};
pub use pipeline::assembly::{AssemblyStats, Topology, Triangle};
pub use pipeline::vertex::VertexStage;
pub use rasterizer::{RasterConfig, RasterStats, Rasterizer, Winding};
pub use viewport::Viewport;

use std::path::Path;

use glam::{Mat4, Vec4};
use render_core::{Camera, Transform};

pub fn model_matrix(transform: Transform) -> Mat4 {
    transform.matrix()
}

pub fn mvp(camera: &Camera, transform: Transform) -> Mat4 {
    camera.projection_matrix() * camera.view_matrix() * transform.matrix()
}

pub fn transform_vertex(mvp: &Mat4, position: glam::Vec3, color: glam::Vec4) -> ClipVertex {
    ClipVertex::new(
        *mvp * Vec4::new(position.x, position.y, position.z, 1.0),
        color,
    )
}

pub fn render_mesh(
    framebuffer: &mut Framebuffer,
    viewport: Viewport,
    config: RasterConfig,
    mesh: &RasterMesh,
    camera: &Camera,
    transform: Transform,
) -> Result<RasterStats, RasterError> {
    let mvp = mvp(camera, transform);
    let mut rasterizer = Rasterizer::new(framebuffer, viewport, config);
    rasterizer.draw_mesh(mesh, mvp)?;
    Ok(rasterizer.stats)
}

pub fn render_to_image(framebuffer: &Framebuffer, path: &Path) -> Result<(), ImageError> {
    write_png(path, framebuffer)
}

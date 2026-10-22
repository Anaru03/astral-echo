pub use render_core::{
    demo_scene, grid_mesh, terrain_height, GRID_SUBDIVISIONS, TERRAIN_EXTENT, TERRAIN_HEIGHT,
};

pub fn demo_camera(aspect: f32) -> Result<render_core::Camera, crate::error::RasterError> {
    render_core::demo_camera(aspect).map_err(|_| crate::error::RasterError::InvalidViewport)
}

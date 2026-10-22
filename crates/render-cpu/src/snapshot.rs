use render_core::{PreparedScene, SnapshotScene};

use crate::{
    Color, CpuPipeline, Framebuffer, PipelineConfig, PipelineStats, RasterError, RasterMode,
    TileConfig, Topology, Viewport,
};

pub fn render_snapshot_scene(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
) -> Result<PipelineStats, RasterError> {
    let viewport = Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline = CpuPipeline::new(framebuffer, viewport, PipelineConfig::default())
        .with_tile_config(TileConfig {
            tile_size: 16,
            mode,
        });
    pipeline.clear(Color::new(8, 10, 18, 255));
    let vp = scene.view_projection();
    for (node, mesh) in scene.draws() {
        pipeline.draw_mesh(mesh, vp * node.transform.matrix(), Topology::TriangleList)?;
    }
    Ok(pipeline.stats)
}

pub fn render_prepared_scene(
    framebuffer: &mut Framebuffer,
    scene: &PreparedScene,
    view_projection: render_core::Mat4,
    mode: RasterMode,
) -> Result<PipelineStats, RasterError> {
    let viewport = Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline = CpuPipeline::new(framebuffer, viewport, PipelineConfig::default())
        .with_tile_config(TileConfig {
            tile_size: 16,
            mode,
        });
    pipeline.clear(Color::new(8, 10, 18, 255));
    for draw in scene.draws() {
        pipeline.draw_mesh(
            &draw.mesh,
            view_projection * draw.node.transform.matrix(),
            Topology::TriangleList,
        )?;
    }
    Ok(pipeline.stats)
}

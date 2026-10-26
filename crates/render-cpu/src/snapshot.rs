use render_core::{PreparedScene, PreparedSceneStats, SnapshotScene};

use crate::{
    Color, CpuPipeline, Framebuffer, FragmentShader, PipelineConfig, PipelineStats, RasterError,
    RasterMode,
    TileConfig, Topology, Viewport,
};

pub fn render_snapshot_scene(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
) -> Result<PipelineStats, RasterError> {
    let (_, stats) = render_snapshot_scene_with_stats(framebuffer, scene, mode)?;
    Ok(stats)
}

pub fn render_snapshot_scene_with_stats(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
) -> Result<(PreparedSceneStats, PipelineStats), RasterError> {
    let prepared = scene.prepare(render_core::CullingConfig {
        width: framebuffer.width(),
        height: framebuffer.height(),
        ..Default::default()
    });
    let prepared_stats = prepared.stats();
    let pipeline_stats = render_prepared_scene_with_clear(
        framebuffer,
        &prepared,
        scene.view_projection(),
        mode,
        overlay_clear_color(scene.overlay()),
        scene.overlay().game_over,
    )?;
    Ok((prepared_stats, pipeline_stats))
}

pub fn render_snapshot_scene_with_shader_with_stats(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    shader: FragmentShader,
) -> Result<(PreparedSceneStats, PipelineStats), RasterError> {
    let time = (scene.snapshot().tick.0 % 4096) as f32;
    render_snapshot_scene_with_shader_at_time_with_stats(
        framebuffer,
        scene,
        mode,
        shader,
        time,
    )
}

pub fn render_snapshot_scene_with_shader_at_time_with_stats(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    shader: FragmentShader,
    time: f32,
) -> Result<(PreparedSceneStats, PipelineStats), RasterError> {
    let prepared = scene.prepare(render_core::CullingConfig {
        width: framebuffer.width(),
        height: framebuffer.height(),
        ..Default::default()
    });
    let prepared_stats = prepared.stats();
    let pipeline_stats = render_prepared_scene_with_shader(
        framebuffer,
        &prepared,
        scene.view_projection(),
        mode,
        overlay_clear_color(scene.overlay()),
        scene.overlay().game_over,
        shader,
        time,
    )?;
    Ok((prepared_stats, pipeline_stats))
}

pub fn render_snapshot_scene_with_shader_at_time(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    shader: FragmentShader,
    time: f32,
) -> Result<PipelineStats, RasterError> {
    let (_, stats) = render_snapshot_scene_with_shader_at_time_with_stats(
        framebuffer,
        scene,
        mode,
        shader,
        time,
    )?;
    Ok(stats)
}

pub fn render_snapshot_scene_with_shader(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    shader: FragmentShader,
) -> Result<PipelineStats, RasterError> {
    let (_, stats) = render_snapshot_scene_with_shader_with_stats(
        framebuffer,
        scene,
        mode,
        shader,
    )?;
    Ok(stats)
}

pub fn render_snapshot_scene_reference(
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
    render_prepared_scene_with_clear(
        framebuffer,
        scene,
        view_projection,
        mode,
        Color::new(8, 10, 18, 255),
        false,
    )
}

fn render_prepared_scene_with_clear(
    framebuffer: &mut Framebuffer,
    scene: &PreparedScene,
    view_projection: render_core::Mat4,
    mode: RasterMode,
    clear: Color,
    game_over: bool,
) -> Result<PipelineStats, RasterError> {
    render_prepared_scene_with_shader(
        framebuffer,
        scene,
        view_projection,
        mode,
        clear,
        game_over,
        FragmentShader::VertexColor,
        0.0,
    )
}

fn render_prepared_scene_with_shader(
    framebuffer: &mut Framebuffer,
    scene: &PreparedScene,
    view_projection: render_core::Mat4,
    mode: RasterMode,
    clear: Color,
    game_over: bool,
    shader: FragmentShader,
    time: f32,
) -> Result<PipelineStats, RasterError> {
    let viewport = Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline = CpuPipeline::new(framebuffer, viewport, PipelineConfig::default())
        .with_fragment_shader(shader)
        .with_fragment_time(time)
        .with_tile_config(TileConfig {
            tile_size: 16,
            mode,
        });
    pipeline.clear(clear);
    if game_over {
        return Ok(pipeline.stats);
    }
    pipeline.draw_prepared_scene(scene, view_projection)?;
    Ok(pipeline.stats)
}

fn overlay_clear_color(overlay: render_core::RenderOverlay) -> Color {
    let fade = overlay.fade_to_black.clamp(0.0, 1.0);
    let scale = 1.0 - fade;
    Color::new(
        (8.0 * scale) as u8,
        (10.0 * scale) as u8,
        (18.0 * scale) as u8,
        255,
    )
}
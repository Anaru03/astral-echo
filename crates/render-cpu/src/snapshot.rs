use render_core::{
    game_over_layout, PreparedScene, PreparedSceneStats, SnapshotScene, GAME_OVER_GLYPHS,
};

use crate::{
    Color, CpuPipeline, CpuPipelineScratch, Framebuffer, FragmentShader, PipelineConfig,
    PipelineStats, RasterError, RasterMode,
    TileConfig, Topology, Viewport,
};

fn cpu_culling_config(framebuffer: &Framebuffer) -> render_core::CullingConfig {
    let mut config = render_core::CullingConfig {
        width: framebuffer.width(),
        height: framebuffer.height(),
        ..Default::default()
    };
    if config.width <= 640 && config.height <= 360 {
        config.low_threshold_px = f32::MAX;
        config.high_threshold_px = f32::MAX;
    }
    config
}

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
    render_snapshot_scene_with_stats_and_scratch(
        framebuffer,
        scene,
        mode,
        &mut CpuPipelineScratch::default(),
    )
}

pub fn render_snapshot_scene_with_stats_and_scratch(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    scratch: &mut CpuPipelineScratch,
) -> Result<(PreparedSceneStats, PipelineStats), RasterError> {
    let prepared = scene.prepare(cpu_culling_config(framebuffer));
    let prepared_stats = prepared.stats();
    let pipeline_stats = render_prepared_scene_with_scratch(
        framebuffer,
        &prepared,
        scene.view_projection(),
        mode,
        overlay_clear_color(scene.overlay()),
        scene.overlay().game_over,
        FragmentShader::VertexColor,
        0.0,
        PipelineConfig::default(),
        scratch,
    )?;
    compose_hud_overlay(framebuffer, &scene.overlay().hud);
    Ok((prepared_stats, pipeline_stats))
}

pub fn compose_hud_overlay(framebuffer: &mut Framebuffer, hud: &render_core::HudState) {
    let overlay = render_core::hud_overlay(hud, framebuffer.width(), framebuffer.height());
    let width = framebuffer.width() as usize;
    let height = framebuffer.height() as usize;
    let (pixels, _) = framebuffer.color_depth_slices_mut();
    for rect in std::iter::once(overlay.background).chain(overlay.glyphs) {
        let x0 = (rect.x as usize).min(width);
        let y0 = (rect.y as usize).min(height);
        let x1 = (rect.x as usize)
            .saturating_add(rect.width as usize)
            .min(width);
        let y1 = (rect.y as usize)
            .saturating_add(rect.height as usize)
            .min(height);
        if x0 >= x1 || y0 >= y1 {
            continue;
        }
        if rect.color[3] == 0 {
            continue;
        }
        if rect.color[3] == 255 {
            for y in y0..y1 {
                let start = (y * width + x0) * 4;
                let end = (y * width + x1) * 4;
                for pixel in pixels[start..end].chunks_exact_mut(4) {
                    pixel.copy_from_slice(&rect.color);
                }
            }
            continue;
        }
        let alpha = f32::from(rect.color[3]) / 255.0;
        let inverse_alpha = 1.0 - alpha;
        for y in y0..y1 {
            let start = (y * width + x0) * 4;
            let end = (y * width + x1) * 4;
            for pixel in pixels[start..end].chunks_exact_mut(4) {
                pixel[0] = (f32::from(rect.color[0]) * alpha
                    + f32::from(pixel[0]) * inverse_alpha) as u8;
                pixel[1] = (f32::from(rect.color[1]) * alpha
                    + f32::from(pixel[1]) * inverse_alpha) as u8;
                pixel[2] = (f32::from(rect.color[2]) * alpha
                    + f32::from(pixel[2]) * inverse_alpha) as u8;
                pixel[3] = 255;
            }
        }
    }
}

pub fn render_snapshot_scene_with_timing(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
) -> Result<(PreparedSceneStats, PipelineStats, u64), RasterError> {
    let preparation_started = std::time::Instant::now();
    let prepared = scene.prepare(cpu_culling_config(framebuffer));
    let preparation_ns = preparation_started.elapsed().as_nanos() as u64;
    let prepared_stats = prepared.stats();
    let pipeline_stats = render_prepared_scene_with_config(
        framebuffer,
        &prepared,
        scene.view_projection(),
        mode,
        overlay_clear_color(scene.overlay()),
        scene.overlay().game_over,
        PipelineConfig {
            timing: true,
            ..Default::default()
        },
    )?;
    compose_hud_overlay(framebuffer, &scene.overlay().hud);
    Ok((prepared_stats, pipeline_stats, preparation_ns))
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
    let prepared = scene.prepare(cpu_culling_config(framebuffer));
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
        PipelineConfig::default(),
    )?;
    compose_hud_overlay(framebuffer, &scene.overlay().hud);
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
    let stats = pipeline.stats;
    drop(pipeline);
    compose_hud_overlay(framebuffer, &scene.overlay().hud);
    Ok(stats)
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
    render_prepared_scene_with_config(
        framebuffer,
        scene,
        view_projection,
        mode,
        clear,
        game_over,
        PipelineConfig::default(),
    )
}

fn render_prepared_scene_with_config(
    framebuffer: &mut Framebuffer,
    scene: &PreparedScene,
    view_projection: render_core::Mat4,
    mode: RasterMode,
    clear: Color,
    game_over: bool,
    config: PipelineConfig,
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
        config,
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
    config: PipelineConfig,
) -> Result<PipelineStats, RasterError> {
    render_prepared_scene_with_scratch(
        framebuffer,
        scene,
        view_projection,
        mode,
        clear,
        game_over,
        shader,
        time,
        config,
        &mut CpuPipelineScratch::default(),
    )
}

fn render_prepared_scene_with_scratch(
    framebuffer: &mut Framebuffer,
    scene: &PreparedScene,
    view_projection: render_core::Mat4,
    mode: RasterMode,
    clear: Color,
    game_over: bool,
    shader: FragmentShader,
    time: f32,
    config: PipelineConfig,
    scratch: &mut CpuPipelineScratch,
) -> Result<PipelineStats, RasterError> {
    let viewport = Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline = CpuPipeline::new_with_scratch(
        framebuffer,
        viewport,
        config,
        std::mem::take(scratch),
    )
        .with_fragment_shader(shader)
        .with_fragment_time(time)
        .with_tile_config(TileConfig {
            tile_size: 16,
            mode,
        });
    pipeline.clear(clear);
    if game_over {
        let stats = pipeline.stats;
        *scratch = pipeline.into_scratch();
        draw_game_over(framebuffer);
        return Ok(stats);
    }
    let draw_result = pipeline.draw_prepared_scene(scene, view_projection);
    let stats = pipeline.stats;
    *scratch = pipeline.into_scratch();
    draw_result?;
    Ok(stats)
}

fn draw_game_over(framebuffer: &mut Framebuffer) {
    let (left, top, scale) = game_over_layout(framebuffer.width(), framebuffer.height());
    let color = Color::new(255, 64, 48, 255);
    for (glyph_index, glyph) in GAME_OVER_GLYPHS.iter().enumerate() {
        let word_gap = u32::from(glyph_index >= 4) * 2 * scale;
        let glyph_left = left + glyph_index as u32 * 6 * scale + word_gap;
        for (row, bits) in glyph.iter().copied().enumerate() {
            for column in 0..5u32 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x = glyph_left + column * scale;
                let y = top + row as u32 * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        framebuffer.set_pixel(x + dx, y + dy, color);
                    }
                }
            }
        }
    }
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

use render_core::{
    game_over_layout, PreparedScene, PreparedSceneStats, SnapshotScene, GAME_OVER_GLYPHS,
};

use crate::{
    Color, CpuPipeline, FragmentShader, Framebuffer, PipelineConfig, PipelineStats, RasterError,
    RasterMode, TileConfig, Topology, Viewport,
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

pub fn diagnostic_fragment_shader() -> Option<FragmentShader> {
    let target = std::env::var("ASTRAL_ECHO_CPU_DIAGNOSTIC_HANDLE")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(render_core::MaterialHandle::new)
        .unwrap_or_else(|| render_core::MaterialHandle::new(100_830));
    match std::env::var("ASTRAL_ECHO_CPU_DIAGNOSTIC").ok().as_deref() {
        Some("magenta") => Some(FragmentShader::DiagnosticMagenta(target)),
        Some("normalized") => Some(FragmentShader::DiagnosticTextureNormalized(target)),
        Some("repeat") => Some(FragmentShader::DiagnosticTextureRepeat(target)),
        _ => None,
    }
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
    if !scene.overlay().game_over {
        compose_hud_overlay(framebuffer, scene, &scene.overlay().hud);
    }
    Ok((prepared_stats, pipeline_stats))
}

pub fn compose_hud_overlay(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    hud: &render_core::HudState,
) {
    let overlay = render_core::hud_overlay(hud, framebuffer.width(), framebuffer.height());
    for rect in std::iter::once(overlay.background).chain(overlay.glyphs) {
        for y in rect.y..rect.y.saturating_add(rect.height) {
            for x in rect.x..rect.x.saturating_add(rect.width) {
                let Some(destination) = framebuffer.pixel(x, y) else {
                    continue;
                };
                let alpha = f32::from(rect.color[3]) / 255.0;
                let blend = |source: u8, target: u8| {
                    (f32::from(source) * alpha + f32::from(target) * (1.0 - alpha)) as u8
                };
                framebuffer.set_pixel(
                    x,
                    y,
                    Color::new(
                        blend(rect.color[0], destination.r),
                        blend(rect.color[1], destination.g),
                        blend(rect.color[2], destination.b),
                        255,
                    ),
                );
            }
        }
    }
    if hud.inventory_open {
        let scale = (framebuffer.width() / 480).clamp(1, 3);
        let margin = 12 * scale;
        let char_width = 7 * scale;
        let max_chars = std::iter::once("INVENTARIO".chars().count())
            .chain(
                hud.inventory_items
                    .iter()
                    .map(|item| item.chars().count() + 4),
            )
            .max()
            .unwrap_or(12);
        let inventory_width = (max_chars as u32 * char_width + margin * 2)
            .min(framebuffer.width().saturating_sub(margin * 2));
        let inventory_x = framebuffer.width().saturating_sub(margin + inventory_width);
        for (index, item) in hud.inventory_items.iter().enumerate() {
            let Some(item_id) = item
                .strip_prefix("ITEM ")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u32>().ok())
            else {
                continue;
            };
            let material = render_core::MaterialHandle::new(500 + item_id);
            let Some(texture) = scene.texture_image(material) else {
                continue;
            };
            let size = (8 * scale).min(16);
            let x = inventory_x + margin / 2;
            let y = margin + margin / 2 + 8 * scale + index as u32 * 8 * scale;
            for py in 0..size {
                for px in 0..size {
                    let sx = (px * texture.width / size).min(texture.width.saturating_sub(1));
                    let sy = (py * texture.height / size).min(texture.height.saturating_sub(1));
                    let offset = ((sy * texture.width + sx) * 4) as usize;
                    if offset + 3 >= texture.pixels.len() {
                        continue;
                    }
                    let alpha = texture.pixels[offset + 3];
                    if alpha == 0 {
                        continue;
                    }
                    framebuffer.set_pixel(
                        x + px,
                        y + py,
                        Color::new(
                            texture.pixels[offset],
                            texture.pixels[offset + 1],
                            texture.pixels[offset + 2],
                            alpha,
                        ),
                    );
                }
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
    if !scene.overlay().game_over {
        compose_hud_overlay(framebuffer, scene, &scene.overlay().hud);
    }
    Ok((prepared_stats, pipeline_stats, preparation_ns))
}

pub fn render_snapshot_scene_with_shader_with_stats(
    framebuffer: &mut Framebuffer,
    scene: &SnapshotScene,
    mode: RasterMode,
    shader: FragmentShader,
) -> Result<(PreparedSceneStats, PipelineStats), RasterError> {
    let time = (scene.snapshot().tick.0 % 4096) as f32;
    render_snapshot_scene_with_shader_at_time_with_stats(framebuffer, scene, mode, shader, time)
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
    if !scene.overlay().game_over {
        compose_hud_overlay(framebuffer, scene, &scene.overlay().hud);
    }
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
    let (_, stats) =
        render_snapshot_scene_with_shader_with_stats(framebuffer, scene, mode, shader)?;
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
    if !scene.overlay().game_over {
        compose_hud_overlay(framebuffer, scene, &scene.overlay().hud);
    }
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
        diagnostic_fragment_shader().unwrap_or(FragmentShader::VertexColor),
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
    let viewport = Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline = CpuPipeline::new(framebuffer, viewport, config)
        .with_fragment_shader(shader)
        .with_fragment_time(time)
        .with_tile_config(TileConfig {
            tile_size: 16,
            mode,
        });
    pipeline.clear(clear);
    if game_over {
        let stats = pipeline.stats;
        drop(pipeline);
        draw_game_over(framebuffer);
        return Ok(stats);
    }
    pipeline.draw_prepared_scene(scene, view_projection)?;
    Ok(pipeline.stats)
}

fn draw_game_over(framebuffer: &mut Framebuffer) {
    for rect in
        render_core::game_over_ui::game_over_rects(framebuffer.width(), framebuffer.height())
    {
        let color = Color::new(rect.color[0], rect.color[1], rect.color[2], rect.color[3]);
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                framebuffer.set_pixel(x, y, color);
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

use std::error::Error;
use std::num::NonZeroU64;
use std::path::PathBuf;

use render_cpu::{
    demo_camera, demo_scene, mvp, write_png, Color, CpuPipeline, PipelineConfig, TileConfig,
    Topology, Viewport,
};
use render_present::{parse_raster_mode, raster_mode_name, run_window, WindowConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let mut config = WindowConfig::default();
    let mut png = None;
    let mut args = std::env::args().skip(1);
    while let Some(option) = args.next() {
        if option == "--help" {
            println!("cpu_window [--preset low|medium|high] [--raster sequential|parallel|auto] [--frames N] [--png PATH]");
            println!("Keys: 1/2/3 presets, S/P/A raster, Escape close");
            return Ok(());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {option}"))?;
        match option.as_str() {
            "--preset" => config.preset = value.parse()?,
            "--raster" => config.raster_mode = parse_raster_mode(&value)?,
            "--frames" => config.max_frames = Some(value.parse::<NonZeroU64>()?),
            "--png" => png = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option: {option}").into()),
        }
    }
    println!(
        "{} {}x{} {} limit={} FPS",
        config.preset,
        config.preset.resolution().0,
        config.preset.resolution().1,
        raster_mode_name(config.raster_mode),
        config.preset.fps()
    );
    let scene = demo_scene();
    let report = run_window(config, move |framebuffer, mode| {
        let width = framebuffer.width();
        let height = framebuffer.height();
        let camera = demo_camera(width as f32 / height as f32)?;
        let viewport = Viewport::from_size(width, height)?;
        let mut pipeline = CpuPipeline::new(framebuffer, viewport, PipelineConfig::default())
            .with_tile_config(TileConfig {
                tile_size: 16,
                mode,
            });
        pipeline.clear(Color::new(8, 10, 18, 255));
        for (mesh, transform) in &scene {
            pipeline.draw_mesh(mesh, mvp(&camera, *transform), Topology::TriangleList)?;
        }
        Ok(())
    })?;
    println!(
        "frames={} render_avg_ms={:.3} present_avg_ms={:.3} elapsed_s={:.3}",
        report.metrics.frames,
        report.metrics.average_render().as_secs_f64() * 1000.0,
        report.metrics.average_presentation().as_secs_f64() * 1000.0,
        report.elapsed.as_secs_f64()
    );
    if let Some(path) = png {
        write_png(&path, &report.framebuffer)?;
        println!("PNG: {}", path.display());
    }
    Ok(())
}

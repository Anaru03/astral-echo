#[path = "visual_support/mod.rs"]
mod support;

use std::error::Error;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use render_core::SnapshotScene;
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};

fn main() -> Result<(), Box<dyn Error>> {
    let mut backend = String::from("cpu");
    let mut window = false;
    let mut frames = None;
    let mut output = None;
    let mut args = std::env::args().skip(1);
    while let Some(option) = args.next() {
        match option.as_str() {
            "--window" => window = true,
            "--backend" => backend = args.next().ok_or("missing backend")?,
            "--frames" => {
                frames = Some(
                    args.next()
                        .ok_or("missing frame count")?
                        .parse::<NonZeroU64>()?,
                )
            }
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing PNG path")?)),
            "--help" => {
                println!(
                    "gameplay_visual [--backend cpu|gpu] [--window] [--frames N] [--output PATH]"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option {option}").into()),
        }
    }
    if backend != "cpu" && backend != "gpu" {
        return Err("backend must be cpu or gpu".into());
    }
    if window && output.is_some() {
        return Err("PNG output uses offscreen mode; omit --window".into());
    }
    let world = support::fixture_world();
    let tick = world.time().tick;
    println!("F5B read-only world tick={} ship={:?} players={} enemies={} destinations={} collectibles={}",tick.0,world.navigation().ship_id(),world.players().count(),world.enemies().count(),world.destinations().count(),world.collectibles().count());
    if window {
        if backend == "cpu" {
            let mut scene = SnapshotScene::default();
            let config = render_present::WindowConfig {
                max_frames: frames,
                title: "Astral Echo - Gameplay CPU".into(),
                ..Default::default()
            };
            let report = render_present::run_window(config, move |framebuffer, mode| {
                scene
                    .update(&support::snapshot(
                        &world,
                        framebuffer.width(),
                        framebuffer.height(),
                    ))
                    .map_err(|_| render_cpu::RasterError::InvalidClip)?;
                render_cpu::render_snapshot_scene(framebuffer, &scene, mode)?;
                Ok(())
            })?;
            println!(
                "CPU window frames={} render_avg_ms={:.3} present_avg_ms={:.3}",
                report.metrics.frames,
                report.metrics.average_render().as_secs_f64() * 1000.0,
                report.metrics.average_presentation().as_secs_f64() * 1000.0
            );
        } else {
            match render_gpu::run_snapshot_window(
                move |width, height| Ok(support::snapshot(&world, width, height)),
                frames.map(NonZeroU64::get),
            ) {
                Ok(report) => println!("GPU window {report:?}"),
                Err(GpuError::Unavailable(reason)) => println!("SKIP GPU: {reason}"),
                Err(error) => return Err(error.into()),
            }
        }
        return Ok(());
    }
    let (width, height) = (640, 360);
    let mut scene = SnapshotScene::default();
    scene.update(&support::snapshot(&world, width, height))?;
    let count = frames.map(NonZeroU64::get).unwrap_or(1);
    let path = output.unwrap_or_else(|| PathBuf::from(format!("target/f5b-output/{backend}.png")));
    if backend == "cpu" {
        let mut framebuffer = render_cpu::Framebuffer::new(width, height)?;
        let start = Instant::now();
        for _ in 0..count {
            render_cpu::render_snapshot_scene(
                &mut framebuffer,
                &scene,
                render_cpu::RasterMode::Secuencial,
            )?;
        }
        let render_ms = start.elapsed().as_secs_f64() * 1000.0 / count as f64;
        render_cpu::write_png(&path, &framebuffer)?;
        println!("CPU frames={count} render_avg_ms={render_ms:.3}");
    } else {
        let gpu = match GpuContext::new(Default::default()) {
            Ok(gpu) => Arc::new(gpu),
            Err(GpuError::Unavailable(reason)) => {
                println!("SKIP GPU: {reason}");
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        println!("ADAPTER {:?}", gpu.adapter_info());
        let mut renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene)?;
        let target = OffscreenTarget::new(gpu.clone(), width, height)?;
        let start = Instant::now();
        for _ in 0..count {
            target.render_snapshot(&mut renderer, &scene)?;
            gpu.wait()?;
        }
        let render_ms = start.elapsed().as_secs_f64() * 1000.0 / count as f64;
        let start = Instant::now();
        let image = target.readback()?;
        let readback_ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("GPU frames={count} submit_to_completion_avg_ms={render_ms:.3} readback_ms={readback_ms:.3}");
        println!(
            "CPU/GPU comparison {:?}",
            support::compare(&support::cpu_image(&scene, width, height), &image)
        );
        image.write_png(&path)?;
    }
    assert_eq!(tick, world.time().tick);
    println!("PNG={} gameplay_tick_unchanged={}", path.display(), tick.0);
    Ok(())
}

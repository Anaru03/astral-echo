#[path = "visual_support/mod.rs"]
mod support;

use std::error::Error;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use game::render_backend::{select_backend, BackendDecision, BackendRequest};
use render_core::SnapshotScene;
use render_gpu::{GpuContext, GpuRenderer, OffscreenTarget};

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
                    "gameplay_visual [--backend cpu|gpu|auto] [--window] [--frames N] [--output PATH]"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option {option}").into()),
        }
    }
    let requested = BackendRequest::parse(&backend)?;
    let decision = select_backend(requested, || {
        GpuContext::new(Default::default())
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    println!(
        "backend requested={} selected={} reason={}",
        backend,
        decision.selected(),
        decision.reason().unwrap_or("none")
    );
    backend = decision.selected().into();
    if window && output.is_some() {
        return Err("PNG output uses offscreen mode; omit --window".into());
    }
    let world = support::fixture_world();
    let tick = world.time().tick;
    println!("F5B read-only world tick={} ship={:?} players={} enemies={} destinations={} collectibles={}",tick.0,world.navigation().ship_id(),world.players().count(),world.enemies().count(),world.destinations().count(),world.collectibles().count());
    if window {
        if matches!(decision, BackendDecision::Cpu { .. }) {
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
            let report = render_gpu::run_snapshot_window(
                move |width, height| Ok(support::snapshot(&world, width, height)),
                frames.map(NonZeroU64::get),
            )?;
            println!("GPU window {report:?}");
        }
        return Ok(());
    }
    let (width, height) = (640, 360);
    let mut scene = SnapshotScene::default();
    scene.update(&support::snapshot(&world, width, height))?;
    let count = frames.map(NonZeroU64::get).unwrap_or(1);
    let path = output.unwrap_or_else(|| PathBuf::from(format!("target/f5b-output/{backend}.png")));
    if matches!(decision, BackendDecision::Cpu { .. }) {
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
        let gpu = Arc::new(GpuContext::new(Default::default())?);
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

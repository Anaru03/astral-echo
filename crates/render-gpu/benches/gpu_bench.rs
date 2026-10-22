#[path = "../examples/support/mod.rs"]
mod support;

use render_gpu::{GpuContext, GpuError, GpuOptions, GpuRenderer, OffscreenTarget};
use std::sync::Arc;
use std::time::Instant;

fn stats(samples: &mut [f64]) -> (f64, f64, f64, f64) {
    samples.sort_by(f64::total_cmp);
    (
        samples.iter().sum::<f64>() / samples.len() as f64,
        samples[(samples.len() * 95).div_ceil(100) - 1],
        samples[0],
        samples[samples.len() - 1],
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let setup = Instant::now();
    let gpu = match GpuContext::new(GpuOptions::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(e)) => {
            println!("SKIP GPU benchmarks: {e}");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    println!("ADAPTER {:?}", gpu.adapter_info());
    let scene = render_core::demo_scene();
    let mut renderer = GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene)?;
    println!(
        "setup_ms={:.3}, resources={:?}",
        setup.elapsed().as_secs_f64() * 1000.0,
        renderer.resources()
    );
    println!("F3, warmup=5 samples=50, ms; CPU=original sequential, persistent framebuffer; GPU=host submit-to-completion; both exclude initialization/readback/PNG; not hardware timestamps");
    println!("resolution,cpu_avg,cpu_p95,cpu_min,cpu_max,gpu_avg,gpu_p95,gpu_min,gpu_max,speedup,readback_avg_ms,target_staging_bytes");
    for (w, h) in [(320, 180), (640, 360), (1280, 720)] {
        let camera = render_core::demo_camera(w as f32 / h as f32).unwrap();
        let target = OffscreenTarget::new(gpu.clone(), w, h)?;
        let mut framebuffer = render_cpu::Framebuffer::new(w, h)?;
        for _ in 0..5 {
            target.render(&mut renderer, &camera)?;
            gpu.wait()?;
            support::draw_cpu(&mut framebuffer, &scene, &camera);
            std::hint::black_box(&framebuffer);
        }
        let cpu_image = support::cpu_image(&scene, &camera, w, h);
        let gpu_image = target.readback()?;
        let comparison = support::compare(&cpu_image, &gpu_image);
        println!("comparison {w}x{h}: {comparison:?}");
        if !comparison.passes() {
            return Err("CPU/GPU comparison failed".into());
        }
        let mut cpu = Vec::new();
        let mut gpu_samples = Vec::new();
        let mut readback = Vec::new();
        for _ in 0..50 {
            let start = Instant::now();
            support::draw_cpu(&mut framebuffer, &scene, &camera);
            std::hint::black_box(&framebuffer);
            cpu.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            target.render(&mut renderer, &camera)?;
            gpu.wait()?;
            gpu_samples.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            std::hint::black_box(target.readback()?);
            readback.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let c = stats(&mut cpu);
        let g = stats(&mut gpu_samples);
        let r = stats(&mut readback);
        let layout = render_gpu::ReadbackLayout::new(w, h)?;
        println!(
            "{w}x{h},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.4},{}",
            c.0,
            c.1,
            c.2,
            c.3,
            g.0,
            g.1,
            g.2,
            g.3,
            c.0 / g.0,
            r.0,
            u64::from(w) * u64::from(h) * 8 + layout.buffer_size * 2
        );
    }
    Ok(())
}

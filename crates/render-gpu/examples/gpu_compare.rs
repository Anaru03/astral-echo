mod support;

use render_gpu::{GpuContext, GpuError, GpuOptions, GpuRenderer, OffscreenTarget};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu = match GpuContext::new(GpuOptions::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(e)) => {
            println!("SKIP GPU: {e}");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    println!("ADAPTER {:?}", gpu.adapter_info());
    let scene = render_core::demo_scene();
    let mut renderer = GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene)?;
    for (width, height) in [(320, 180), (640, 360), (1280, 720), (321, 181)] {
        let camera = render_core::demo_camera(width as f32 / height as f32).unwrap();
        let cpu = support::cpu_image(&scene, &camera, width, height);
        let target = OffscreenTarget::new(gpu.clone(), width, height)?;
        target.render(&mut renderer, &camera)?;
        let image = target.readback()?;
        cpu.write_png(format!("target/f5a-output/cpu-{width}x{height}.png"))?;
        image.write_png(format!("target/f5a-output/gpu-{width}x{height}.png"))?;
        let comparison = support::compare(&cpu, &image);
        println!("{width}x{height}: {comparison:?}");
        if !comparison.passes() {
            return Err("CPU/GPU tolerance exceeded".into());
        }
    }
    Ok(())
}

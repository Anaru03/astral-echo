use render_gpu::{GpuContext, GpuError, GpuOptions, GpuRenderer, OffscreenTarget};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu = match GpuContext::new(GpuOptions::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            println!("SKIP GPU: {reason}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    println!("ADAPTER {:?}", gpu.adapter_info());
    let mut renderer = GpuRenderer::new(
        gpu.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
        &render_core::demo_scene(),
    )?;
    let target = OffscreenTarget::new(gpu, 640, 360)?;
    target.render(
        &mut renderer,
        &render_core::demo_camera(16.0 / 9.0).map_err(|e| format!("camera: {e:?}"))?,
    )?;
    let image = target.readback()?;
    image.write_png("target/f5a-output/gpu-f3.png")?;
    println!(
        "GPU PNG 640x360, depth pixels={}, resources={:?}",
        image.depth.len(),
        renderer.resources()
    );
    Ok(())
}

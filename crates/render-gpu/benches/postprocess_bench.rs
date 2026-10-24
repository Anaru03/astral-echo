use render_core::{shadow_fixture, Mat4, Vec3};
use render_gpu::{
    GpuContext, GpuError, GpuRenderer, OffscreenTarget, PostProcessConfig, PostProcessPreset,
};
use std::sync::Arc;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            println!("SKIP GPU: {reason}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    println!(
        "adapter={:?}; resolution,preset,host_ms,postprocess_allocations,postprocess_bytes,draws",
        gpu.adapter_info()
    );
    for (width, height) in [(1280, 720), (1920, 1080)] {
        for preset in [
            PostProcessPreset::Low,
            PostProcessPreset::Medium,
            PostProcessPreset::High,
        ] {
            let scene = shadow_fixture();
            let vp = Mat4::perspective_rh(1.0, width as f32 / height as f32, 0.1, 100.0)
                * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
            let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene)?;
            let mut target = OffscreenTarget::new(gpu.clone(), width, height)?;
            target.set_postprocess(PostProcessConfig::preset(preset))?;
            for _ in 0..4 {
                target.render_prepared(&mut renderer, &scene, vp)?;
                gpu.wait()?;
            }
            let mut total = 0.0;
            for _ in 0..20 {
                let start = Instant::now();
                target.render_prepared(&mut renderer, &scene, vp)?;
                gpu.wait()?;
                total += start.elapsed().as_secs_f64() * 1000.0;
            }
            let resources = target.postprocess_resources();
            println!(
                "{}x{}, {:?},{:.3},{},{},{}",
                width,
                height,
                preset,
                total / 20.0,
                resources.map_or(0, |r| r.2),
                u64::from(width) * u64::from(height) * 4,
                renderer.resources().draw_count
            );
        }
    }
    Ok(())
}

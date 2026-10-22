fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let frames = match args.next().as_deref() {
        None => None,
        Some("--frames") => Some(args.next().ok_or("missing frame limit")?.parse()?),
        _ => return Err("usage: gpu_window [--frames N]".into()),
    };
    match render_gpu::run_window(
        render_core::demo_scene(),
        render_core::demo_camera(16.0 / 9.0).map_err(|e| format!("{e:?}"))?,
        frames,
    ) {
        Ok(report) => println!("{report:?}"),
        Err(render_gpu::GpuError::Unavailable(message)) => println!("SKIP GPU: {message}"),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

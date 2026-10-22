use render_gpu::{GpuContext, GpuError, GpuOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match GpuContext::new(GpuOptions::default()) {
        Ok(gpu) => println!("ADAPTER {:?}", gpu.adapter_info()),
        Err(GpuError::Unavailable(message)) => println!("SKIP GPU: {message}"),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

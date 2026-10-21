use std::fs;
use std::path::Path;

use crate::error::ImageError;
use crate::framebuffer::Framebuffer;

pub fn write_png(path: &Path, framebuffer: &Framebuffer) -> Result<(), ImageError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let file = fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, framebuffer.width(), framebuffer.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::default());
    let mut writer = encoder.write_header().map_err(ImageError::Encode)?;
    writer
        .write_image_data(framebuffer.color_bytes())
        .map_err(ImageError::Encode)?;
    writer.finish().map_err(ImageError::Encode)?;
    Ok(())
}

pub fn write_png_bytes(framebuffer: &Framebuffer) -> Result<Vec<u8>, ImageError> {
    let mut buffer = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buffer, framebuffer.width(), framebuffer.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::default());
        let mut writer = encoder.write_header().map_err(ImageError::Encode)?;
        writer
            .write_image_data(framebuffer.color_bytes())
            .map_err(ImageError::Encode)?;
        writer.finish().map_err(ImageError::Encode)?;
    }
    Ok(buffer)
}

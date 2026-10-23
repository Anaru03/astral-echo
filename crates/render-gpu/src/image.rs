use std::path::Path;

use crate::GpuError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadbackLayout {
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub buffer_size: u64,
}

impl ReadbackLayout {
    pub fn new(width: u32, height: u32) -> Result<Self, GpuError> {
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidInput("zero image dimension".into()));
        }
        let row = width
            .checked_mul(4)
            .and_then(|v| v.checked_add(255))
            .map(|v| v / 256 * 256)
            .ok_or_else(|| GpuError::InvalidInput("image row overflow".into()))?;
        let size = u64::from(row) * u64::from(height);
        if size > isize::MAX as u64 {
            return Err(GpuError::InvalidInput("image allocation overflow".into()));
        }
        Ok(Self {
            width,
            height,
            bytes_per_row: row,
            buffer_size: size,
        })
    }

    pub fn unpack(self, data: &[u8]) -> Result<Vec<u8>, GpuError> {
        if self != Self::new(self.width, self.height)? {
            return Err(GpuError::Readback("invalid readback layout".into()));
        }
        if data.len() as u64 != self.buffer_size {
            return Err(GpuError::Readback("invalid mapped length".into()));
        }
        let mut pixels = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for row in data.chunks_exact(self.bytes_per_row as usize) {
            pixels.extend_from_slice(&row[..self.width as usize * 4]);
        }
        Ok(pixels)
    }
}

#[derive(Clone, Debug)]
pub struct GpuImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub depth: Vec<f32>,
}

impl GpuImage {
    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        ReadbackLayout::new(self.width, self.height)?;
        if self.rgba.len() != self.width as usize * self.height as usize * 4 {
            return Err(GpuError::InvalidInput("invalid RGBA image length".into()).into());
        }
        if let Some(parent) = path.as_ref().parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(std::fs::File::create(path)?),
            self.width,
            self.height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.rgba)?;
        writer.finish()?;
        Ok(())
    }
}

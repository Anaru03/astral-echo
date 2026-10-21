use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PixelError {
    ZeroDimensions,
    DimensionOverflow,
    BufferLength { expected: usize, actual: usize },
    InvalidScaleFactor,
}

impl fmt::Display for PixelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimensions => formatter.write_str("pixel dimensions must be nonzero"),
            Self::DimensionOverflow => {
                formatter.write_str("pixel dimensions exceed addressable memory")
            }
            Self::BufferLength { expected, actual } => {
                write!(formatter, "buffer length {actual}, expected {expected}")
            }
            Self::InvalidScaleFactor => {
                formatter.write_str("DPI scale factor must be finite and positive")
            }
        }
    }
}

impl Error for PixelError {}

fn pixel_count(size: (u32, u32)) -> Result<usize, PixelError> {
    if size.0 == 0 || size.1 == 0 {
        return Err(PixelError::ZeroDimensions);
    }
    let count = u64::from(size.0) * u64::from(size.1);
    let bytes = count.checked_mul(4).ok_or(PixelError::DimensionOverflow)?;
    if bytes > isize::MAX as u64 {
        return Err(PixelError::DimensionOverflow);
    }
    usize::try_from(count).map_err(|_| PixelError::DimensionOverflow)
}

pub const fn rgba_to_xrgb(rgba: [u8; 4]) -> u32 {
    ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | rgba[2] as u32
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScaleLayout {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ScaleLayout {
    pub fn fit(source: (u32, u32), target: (u32, u32)) -> Result<Self, PixelError> {
        pixel_count(source)?;
        pixel_count(target)?;
        let (width, height) = if u64::from(target.0) * u64::from(source.1)
            <= u64::from(target.1) * u64::from(source.0)
        {
            (
                target.0,
                (u64::from(target.0) * u64::from(source.1) / u64::from(source.0)).max(1) as u32,
            )
        } else {
            (
                (u64::from(target.1) * u64::from(source.0) / u64::from(source.1)).max(1) as u32,
                target.1,
            )
        };
        Ok(Self {
            x: (target.0 - width) / 2,
            y: (target.1 - height) / 2,
            width,
            height,
        })
    }
}

pub fn copy_nearest_rgba(
    rgba: &[u8],
    source: (u32, u32),
    target: (u32, u32),
    output: &mut [u32],
) -> Result<ScaleLayout, PixelError> {
    let source_len = pixel_count(source)? * 4;
    let target_len = pixel_count(target)?;
    for (expected, actual) in [(source_len, rgba.len()), (target_len, output.len())] {
        if expected != actual {
            return Err(PixelError::BufferLength { expected, actual });
        }
    }
    let layout = ScaleLayout::fit(source, target)?;
    output.fill(0);
    for y in 0..layout.height {
        let sy = (u64::from(y) * u64::from(source.1) / u64::from(layout.height)) as usize;
        let target_row = (y + layout.y) as usize * target.0 as usize + layout.x as usize;
        for x in 0..layout.width {
            let sx = (u64::from(x) * u64::from(source.0) / u64::from(layout.width)) as usize;
            let source_pixel = (sy * source.0 as usize + sx) * 4;
            output[target_row + x as usize] = rgba_to_xrgb([
                rgba[source_pixel],
                rgba[source_pixel + 1],
                rgba[source_pixel + 2],
                rgba[source_pixel + 3],
            ]);
        }
    }
    Ok(layout)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeState {
    physical: (u32, u32),
    scale_factor: f64,
}

impl ResizeState {
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Result<Self, PixelError> {
        let mut state = Self {
            physical: (0, 0),
            scale_factor: 1.0,
        };
        state.resize(width, height)?;
        state.set_scale_factor(scale_factor)?;
        Ok(state)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<bool, PixelError> {
        if width != 0 && height != 0 {
            pixel_count((width, height))?;
        }
        let changed = self.physical != (width, height);
        self.physical = (width, height);
        Ok(changed)
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) -> Result<(), PixelError> {
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(PixelError::InvalidScaleFactor);
        }
        self.scale_factor = scale_factor;
        Ok(())
    }

    pub const fn physical_size(&self) -> (u32, u32) {
        self.physical
    }

    pub fn logical_size(&self) -> (f64, f64) {
        (
            self.physical.0 as f64 / self.scale_factor,
            self.physical.1 as f64 / self.scale_factor,
        )
    }

    pub fn surface_size(&self) -> Option<(NonZeroU32, NonZeroU32)> {
        Some((
            NonZeroU32::new(self.physical.0)?,
            NonZeroU32::new(self.physical.1)?,
        ))
    }
}

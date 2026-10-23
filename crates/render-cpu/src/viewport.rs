use crate::error::RasterError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Viewport {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Result<Self, RasterError> {
        if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
            return Err(RasterError::InvalidViewport);
        }
        if width <= 0.0 || height <= 0.0 {
            return Err(RasterError::InvalidViewport);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub fn from_size(width: u32, height: u32) -> Result<Self, RasterError> {
        Self::new(0.0, 0.0, width as f32, height as f32)
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn ndc_to_pixel(&self, ndc_x: f32, ndc_y: f32) -> (f32, f32) {
        let px = self.x + (ndc_x * 0.5 + 0.5) * self.width;
        let py = self.y + (0.5 - ndc_y * 0.5) * self.height;
        (px, py)
    }

    pub fn pixel_to_ndc(&self, px: f32, py: f32) -> (f32, f32) {
        let ndc_x = ((px - self.x) / self.width) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((py - self.y) / self.height) * 2.0;
        (ndc_x, ndc_y)
    }
}

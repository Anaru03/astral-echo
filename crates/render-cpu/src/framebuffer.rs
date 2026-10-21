use crate::error::RasterError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    color: Vec<u8>,
    depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Result<Self, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::InvalidFramebufferSize);
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > u64::from(u32::MAX) {
            return Err(RasterError::FramebufferDimensionOverflow);
        }
        let len = pixels
            .checked_mul(4)
            .ok_or(RasterError::FramebufferDimensionOverflow)?;
        if len > usize::MAX as u64 {
            return Err(RasterError::FramebufferDimensionOverflow);
        }
        Ok(Self {
            width,
            height,
            color: vec![0u8; len as usize],
            depth: vec![1.0f32; pixels as usize],
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn color_bytes(&self) -> &[u8] {
        &self.color
    }

    pub fn color_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.color
    }

    pub fn depth_slice_mut(&mut self) -> &mut [f32] {
        &mut self.depth
    }

    /// Devuelve slices mutables disjuntos de color y profundidad.
    pub fn color_depth_slices_mut(&mut self) -> (&mut [u8], &mut [f32]) {
        (&mut self.color, &mut self.depth)
    }

    pub fn depth_at(&self, x: u32, y: u32) -> Option<f32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.depth[self.index(x, y)])
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = self.index(x, y) * 4;
        Some(Color::new(
            self.color[i],
            self.color[i + 1],
            self.color[i + 2],
            self.color[i + 3],
        ))
    }

    pub fn clear_color(&mut self, color: Color) {
        let rgba = color.to_rgba8();
        for px in self.color.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
    }

    pub fn clear_depth(&mut self) {
        for d in self.depth.iter_mut() {
            *d = 1.0f32;
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.clear_color(color);
        self.clear_depth();
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let i = self.index(x, y) * 4;
        self.color[i] = color.r;
        self.color[i + 1] = color.g;
        self.color[i + 2] = color.b;
        self.color[i + 3] = color.a;
        true
    }

    pub fn set_depth(&mut self, x: u32, y: u32, depth: f32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let i = self.index(x, y);
        self.depth[i] = depth;
        true
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), RasterError> {
        let fresh = Framebuffer::new(width, height)?;
        self.width = fresh.width;
        self.height = fresh.height;
        self.color = fresh.color;
        self.depth = fresh.depth;
        Ok(())
    }

    fn index(&self, x: u32, y: u32) -> usize {
        (y as usize) * (self.width as usize) + (x as usize)
    }
}

//! Pantalla inicial nativa, presentada en la misma ventana del renderizador.
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

pub struct NativeSplash {
    pub window: Arc<Window>,
    pub context: Context<Arc<Window>>,
    pub surface: Surface<Arc<Window>, Arc<Window>>,
    pub selected: usize,
    pub controls: bool,
}

impl NativeSplash {
    pub fn present(&mut self, loading: bool) -> Result<(), String> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.surface
            .resize(
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            )
            .map_err(|e| e.to_string())?;
        let source = if loading {
            include_bytes!("../../../assets/menu/native/loading.png").as_slice()
        } else if self.controls {
            include_bytes!("../../../assets/menu/native/controls.png").as_slice()
        } else {
            include_bytes!("../../../assets/menu/native/menu.png").as_slice()
        };
        let image = image::load_from_memory(source)
            .map_err(|e| e.to_string())?
            .to_rgb8();
        let (iw, ih) = image.dimensions();
        let scale = (size.width as f32 / iw as f32).min(size.height as f32 / ih as f32);
        let dw = (iw as f32 * scale).round().max(1.0) as u32;
        let dh = (ih as f32 * scale).round().max(1.0) as u32;
        let scaled = image::imageops::resize(&image, dw, dh, image::imageops::FilterType::Triangle);
        let xoff = (size.width - dw) / 2;
        let yoff = (size.height - dh) / 2;
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        buffer.fill(0x080C19);
        for y in 0..dh {
            for x in 0..dw {
                let p = scaled.get_pixel(x, y).0;
                let index = ((y + yoff) * size.width + (x + xoff)) as usize;
                buffer[index] = (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]);
            }
        }
        // Marcador de seleccion discreto, sin alterar la imagen original.
        if !loading && !self.controls {
            let x = xoff + (251.0 * scale) as u32;
            let y = yoff + ((611 + self.selected * 47) as f32 * scale) as u32;
            let r = (5.0 * scale).max(2.0) as u32;
            for yy in y.saturating_sub(r)..(y + r).min(size.height) {
                for xx in x.saturating_sub(r)..(x + r).min(size.width) {
                    buffer[(yy * size.width + xx) as usize] = 0x5BDCE7;
                }
            }
        }
        buffer.present().map_err(|e| e.to_string())
    }
}

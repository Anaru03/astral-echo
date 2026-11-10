use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub enum TextureLoadError {
    Io(std::io::Error),
    Decode(png::DecodingError),
    Invalid(String),
}

impl fmt::Display for TextureLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "PNG I/O error: {error}"),
            Self::Decode(error) => write!(f, "PNG decoding error: {error}"),
            Self::Invalid(message) => write!(f, "invalid PNG: {message}"),
        }
    }
}

impl std::error::Error for TextureLoadError {}

impl From<std::io::Error> for TextureLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<png::DecodingError> for TextureLoadError {
    fn from(error: png::DecodingError) -> Self {
        Self::Decode(error)
    }
}

impl TextureImage {
    pub fn load_png(path: impl AsRef<Path>) -> Result<Self, TextureLoadError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut decoder = png::Decoder::new(reader);

        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);

        let mut reader = decoder.read_info()?;

        let mut buffer = vec![
            0;
            reader.output_buffer_size().ok_or_else(|| {
                TextureLoadError::Invalid("PNG output buffer too large".into())
            })?
        ];

        let info = reader.next_frame(&mut buffer)?;
        let bytes = &buffer[..info.buffer_size()];

        let pixel_count = (info.width as usize)
            .checked_mul(info.height as usize)
            .ok_or_else(|| TextureLoadError::Invalid("PNG dimensions overflow".into()))?;

        let mut pixels = Vec::with_capacity(
            pixel_count
                .checked_mul(4)
                .ok_or_else(|| TextureLoadError::Invalid("RGBA buffer size overflow".into()))?,
        );

        match info.color_type {
            png::ColorType::Rgb => {
                if bytes.len() != pixel_count * 3 {
                    return Err(TextureLoadError::Invalid("invalid RGB data length".into()));
                }

                for rgb in bytes.chunks_exact(3) {
                    pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }

            png::ColorType::Rgba => {
                if bytes.len() != pixel_count * 4 {
                    return Err(TextureLoadError::Invalid("invalid RGBA data length".into()));
                }

                pixels.extend_from_slice(bytes);
            }

            png::ColorType::Grayscale => {
                if bytes.len() != pixel_count {
                    return Err(TextureLoadError::Invalid(
                        "invalid grayscale data length".into(),
                    ));
                }

                for &gray in bytes {
                    pixels.extend_from_slice(&[gray, gray, gray, 255]);
                }
            }

            png::ColorType::GrayscaleAlpha => {
                if bytes.len() != pixel_count * 2 {
                    return Err(TextureLoadError::Invalid(
                        "invalid grayscale-alpha data length".into(),
                    ));
                }

                for gray_alpha in bytes.chunks_exact(2) {
                    pixels.extend_from_slice(&[
                        gray_alpha[0],
                        gray_alpha[0],
                        gray_alpha[0],
                        gray_alpha[1],
                    ]);
                }
            }

            _ => {
                return Err(TextureLoadError::Invalid(
                    "unsupported PNG color format".into(),
                ));
            }
        }

        Ok(Self {
            width: info.width,
            height: info.height,
            pixels,
        })
    }
}

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RasterError {
    InvalidFramebufferSize,
    FramebufferDimensionOverflow,
    InvalidViewport,
    NonFiniteVertex,
    InvalidClip,
    InvalidPngOutput,
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            RasterError::InvalidFramebufferSize => "invalid framebuffer size",
            RasterError::FramebufferDimensionOverflow => "framebuffer dimensions overflow",
            RasterError::InvalidViewport => "invalid viewport",
            RasterError::NonFiniteVertex => "non finite vertex data",
            RasterError::InvalidClip => "invalid clip space coordinates",
            RasterError::InvalidPngOutput => "invalid png output path",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for RasterError {}

#[derive(Debug)]
pub enum ImageError {
    Encode(png::EncodingError),
    Io(std::io::Error),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::Encode(e) => write!(f, "png encoding failed: {e}"),
            ImageError::Io(e) => write!(f, "png io failed: {e}"),
        }
    }
}

impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImageError::Encode(e) => Some(e),
            ImageError::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for ImageError {
    fn from(value: std::io::Error) -> Self {
        ImageError::Io(value)
    }
}

use std::error::Error;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};
use std::str::FromStr;

use render_cpu::RasterMode;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    #[default]
    Low,
    Medium,
    High,
}

impl Preset {
    pub const fn resolution(self) -> (u32, u32) {
        match self {
            Self::Low => (640, 360),
            Self::Medium => (960, 540),
            Self::High => (1280, 720),
        }
    }

    pub fn fps(self) -> NonZeroU32 {
        NonZeroU32::new(match self {
            Self::Low => 30,
            Self::Medium | Self::High => 60,
        })
        .unwrap()
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ConfigError {}

impl FromStr for Preset {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(ConfigError(format!(
                "unknown preset: {value}; expected low, medium or high"
            ))),
        }
    }
}

pub fn parse_raster_mode(value: &str) -> Result<RasterMode, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "sequential" => Ok(RasterMode::Secuencial),
        "parallel" => Ok(RasterMode::Paralelo),
        "auto" => Ok(RasterMode::Auto),
        _ => Err(ConfigError(format!(
            "unknown raster mode: {value}; expected sequential, parallel or auto"
        ))),
    }
}

pub const fn raster_mode_name(mode: RasterMode) -> &'static str {
    match mode {
        RasterMode::Secuencial => "Sequential",
        RasterMode::Paralelo => "Parallel",
        RasterMode::Auto => "Auto",
    }
}

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub preset: Preset,
    pub raster_mode: RasterMode,
    pub max_frames: Option<NonZeroU64>,
    pub raster_controls: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Astral Echo - CPU".into(),
            preset: Preset::Low,
            raster_mode: RasterMode::Auto,
            max_frames: None,
            raster_controls: true,
        }
    }
}
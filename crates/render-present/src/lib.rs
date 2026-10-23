pub mod config;
pub mod hot_swap;
pub mod hot_window;
pub mod pixels;
pub mod timing;
pub mod window;

pub use config::{parse_raster_mode, raster_mode_name, ConfigError, Preset, WindowConfig};
pub use hot_swap::{BackendKind, BackendSnapshot, HotSwapState};
pub use hot_window::{
    run_hot_swap_window, run_hot_swap_window_with_debug, GraphicsDebugConfig, HotSwapReport,
    SnapshotSource,
};
pub use pixels::{copy_nearest_rgba, rgba_to_xrgb, PixelError, ResizeState, ScaleLayout};
pub use timing::{FrameMetrics, FramePacer};
pub use window::{run_window, PresentError, RunReport};

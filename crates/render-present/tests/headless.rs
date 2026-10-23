use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use render_cpu::RasterMode;
use render_present::{
    copy_nearest_rgba, parse_raster_mode, rgba_to_xrgb, FrameMetrics, FramePacer, PixelError,
    Preset, ResizeState, ScaleLayout, WindowConfig,
};

const RGBA: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 255, 255, 255, 0,
];
const RED: u32 = 0xff0000;
const GREEN: u32 = 0x00ff00;
const BLUE: u32 = 0x0000ff;
const WHITE: u32 = 0xffffff;

#[test]
fn presets_match_resolution_and_frame_rate_contract() {
    for (preset, size, fps) in [
        (Preset::Low, (640, 360), 30),
        (Preset::Medium, (960, 540), 60),
        (Preset::High, (1280, 720), 60),
    ] {
        assert_eq!(preset.resolution(), size);
        assert_eq!(preset.fps().get(), fps);
        assert_eq!(preset.to_string().parse::<Preset>().unwrap(), preset);
    }
    let config = WindowConfig::default();
    assert_eq!(config.preset, Preset::Low);
    assert_eq!(config.raster_mode, RasterMode::Auto);
    assert!(config.max_frames.is_none());
}

#[test]
fn raster_selection_and_invalid_options_are_explicit() {
    assert_eq!(
        parse_raster_mode("Sequential").unwrap(),
        RasterMode::Secuencial
    );
    assert_eq!(parse_raster_mode("PARALLEL").unwrap(), RasterMode::Paralelo);
    assert_eq!(parse_raster_mode("auto").unwrap(), RasterMode::Auto);
    assert!(parse_raster_mode("gpu").is_err());
    assert!("ultra".parse::<Preset>().is_err());
}

#[test]
fn rgba_conversion_preserves_rgb_and_clears_unused_high_byte() {
    assert_eq!(rgba_to_xrgb([0x12, 0x34, 0x56, 0]), 0x00123456);
    assert_eq!(rgba_to_xrgb([0x12, 0x34, 0x56, 255]), 0x00123456);
    assert_eq!(rgba_to_xrgb([255, 255, 255, 255]), WHITE);
}

#[test]
fn native_size_conversion_preserves_orientation() {
    let mut output = [0; 4];
    copy_nearest_rgba(&RGBA, (2, 2), (2, 2), &mut output).unwrap();
    assert_eq!(output, [RED, GREEN, BLUE, WHITE]);
}

#[test]
fn integer_upscale_replicates_each_source_pixel() {
    let mut output = [0; 16];
    copy_nearest_rgba(&RGBA, (2, 2), (4, 4), &mut output).unwrap();
    assert_eq!(
        output,
        [
            RED, RED, GREEN, GREEN, RED, RED, GREEN, GREEN, BLUE, BLUE, WHITE, WHITE, BLUE, BLUE,
            WHITE, WHITE
        ]
    );
}

#[test]
fn fractional_upscale_uses_nearest_neighbors_without_filtering() {
    let mut output = [0; 9];
    copy_nearest_rgba(&RGBA, (2, 2), (3, 3), &mut output).unwrap();
    assert_eq!(
        output,
        [RED, RED, GREEN, RED, RED, GREEN, BLUE, BLUE, WHITE]
    );
}

#[test]
fn downscale_samples_expected_source_coordinates() {
    let rgba: Vec<u8> = (0..16).flat_map(|value| [value, 0, 0, 255]).collect();
    let mut output = [0; 4];
    copy_nearest_rgba(&rgba, (4, 4), (2, 2), &mut output).unwrap();
    assert_eq!(output, [0, 2 << 16, 8 << 16, 10 << 16]);
}

#[test]
fn aspect_ratio_adds_black_letterbox_bars() {
    let mut output = [WHITE; 16];
    let layout = copy_nearest_rgba(&RGBA[..8], (2, 1), (4, 4), &mut output).unwrap();
    assert_eq!(
        layout,
        ScaleLayout {
            x: 0,
            y: 1,
            width: 4,
            height: 2
        }
    );
    assert_eq!(
        output,
        [0, 0, 0, 0, RED, RED, GREEN, GREEN, RED, RED, GREEN, GREEN, 0, 0, 0, 0]
    );
}

#[test]
fn aspect_ratio_adds_black_pillarbox_bars() {
    let mut output = [WHITE; 16];
    let layout = copy_nearest_rgba(&RGBA[..8], (1, 2), (4, 4), &mut output).unwrap();
    assert_eq!(
        layout,
        ScaleLayout {
            x: 1,
            y: 0,
            width: 2,
            height: 4
        }
    );
    assert_eq!(
        output,
        [0, RED, RED, 0, 0, RED, RED, 0, 0, GREEN, GREEN, 0, 0, GREEN, GREEN, 0]
    );
}

#[test]
fn odd_sizes_and_single_pixel_targets_stay_within_surface() {
    assert_eq!(
        ScaleLayout::fit((640, 360), (801, 601)).unwrap(),
        ScaleLayout {
            x: 0,
            y: 75,
            width: 801,
            height: 450
        }
    );
    let mut output = [0];
    copy_nearest_rgba(&RGBA, (2, 2), (1, 1), &mut output).unwrap();
    assert_eq!(output, [RED]);
}

#[test]
fn invalid_sizes_and_buffer_lengths_do_not_modify_output() {
    let mut output = [WHITE; 4];
    assert!(copy_nearest_rgba(&RGBA[..12], (2, 2), (2, 2), &mut output).is_err());
    assert!(copy_nearest_rgba(&RGBA, (2, 2), (2, 3), &mut output).is_err());
    assert_eq!(output, [WHITE; 4]);
    assert_eq!(
        ScaleLayout::fit((0, 2), (2, 2)),
        Err(PixelError::ZeroDimensions)
    );
    assert_eq!(
        ScaleLayout::fit((2, 2), (0, 2)),
        Err(PixelError::ZeroDimensions)
    );
    assert_eq!(
        ScaleLayout::fit((2, 2), (u32::MAX, u32::MAX)),
        Err(PixelError::DimensionOverflow)
    );
}

#[test]
fn resize_minimize_and_restore_track_physical_pixels() {
    let mut state = ResizeState::new(800, 600, 1.0).unwrap();
    assert!(!state.resize(800, 600).unwrap());
    for size in [(0, 0), (0, 600), (800, 0)] {
        state.resize(size.0, size.1).unwrap();
        assert!(state.surface_size().is_none());
    }
    assert!(state.resize(801, 601).unwrap());
    let size = state.surface_size().unwrap();
    assert_eq!((size.0.get(), size.1.get()), (801, 601));
}

#[test]
fn dpi_change_does_not_apply_physical_scaling_twice() {
    let mut state = ResizeState::new(960, 540, 1.5).unwrap();
    assert_eq!(state.logical_size(), (640.0, 360.0));
    state.set_scale_factor(2.0).unwrap();
    state.resize(1280, 720).unwrap();
    assert_eq!(state.logical_size(), (640.0, 360.0));
    assert_eq!(state.physical_size(), (1280, 720));
    assert_eq!(
        ScaleLayout::fit(Preset::Low.resolution(), state.physical_size()).unwrap(),
        ScaleLayout {
            x: 0,
            y: 0,
            width: 1280,
            height: 720
        }
    );
}

#[test]
fn invalid_resize_and_dpi_preserve_previous_state() {
    let mut state = ResizeState::new(640, 360, 1.0).unwrap();
    let initial = state;
    assert!(state.resize(u32::MAX, u32::MAX).is_err());
    assert_eq!(state, initial);
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(state.set_scale_factor(scale).is_err());
        assert_eq!(state, initial);
    }
}

#[test]
fn frame_pacer_limits_early_redraws_without_sleeping_in_tests() {
    for (fps, nanos) in [(30, 33_333_333), (60, 16_666_666)] {
        let now = Instant::now();
        let mut pacer = FramePacer::new(NonZeroU32::new(fps).unwrap(), now);
        assert!(pacer.is_due(now));
        pacer.frame_started(now);
        assert!(!pacer.is_due(now));
        assert!(!pacer.is_due(now + Duration::from_nanos(nanos - 1)));
        assert_eq!(pacer.deadline(), now + Duration::from_nanos(nanos));
        assert!(pacer.is_due(pacer.deadline()));
    }
}

#[test]
fn delayed_frames_do_not_schedule_a_catch_up_burst() {
    let now = Instant::now();
    let mut pacer = FramePacer::new(Preset::Low.fps(), now);
    let delayed = now + Duration::from_secs(5);
    assert!(pacer.is_due(delayed));
    pacer.frame_started(delayed);
    assert_eq!(pacer.deadline(), delayed + Duration::from_nanos(33_333_333));
    assert!(!pacer.is_due(delayed));
}

#[test]
fn render_and_presentation_metrics_are_independent() {
    let mut metrics = FrameMetrics::default();
    assert_eq!(metrics.average_render(), Duration::ZERO);
    assert_eq!(metrics.average_presentation(), Duration::ZERO);
    metrics.record(Duration::from_millis(1), Duration::from_millis(9));
    metrics.record(Duration::from_millis(3), Duration::from_millis(11));
    assert_eq!(metrics.frames, 2);
    assert_eq!(metrics.average_render(), Duration::from_millis(2));
    assert_eq!(metrics.average_presentation(), Duration::from_millis(10));
    assert_eq!(metrics.last_render, Duration::from_millis(3));
    assert_eq!(metrics.last_presentation, Duration::from_millis(11));
}

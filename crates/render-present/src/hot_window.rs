use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use render_core::{InterpolationConfig, RenderSnapshot, SnapshotBuffer, SnapshotScene};
use render_cpu::{
    render_snapshot_scene_with_shader, render_snapshot_scene_with_shader_with_stats, write_png,
    Framebuffer, FragmentShader, RasterMode,
};
use render_gpu::{GpuOptions, SurfaceRenderer};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::{
    copy_nearest_rgba, BackendKind, BackendSnapshot, HotSwapState, Preset, ScaleLayout,
    WindowConfig,
};

pub type SnapshotSource = Box<dyn FnMut(u32, u32) -> Result<RenderSnapshot, String>>;

#[derive(Clone, Debug, Default)]
pub struct GraphicsDebugConfig {
    pub directory: Option<std::path::PathBuf>,
}

pub fn run_hot_swap_window(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
) -> Result<HotSwapReport, String> {
    run_hot_swap_window_with_input_and_scene(
        config,
        source,
        start_gpu,
        |_| {},
        |_| {},
        GraphicsDebugConfig::default(),
    )
}

pub fn run_hot_swap_window_with_debug(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    debug: GraphicsDebugConfig,
) -> Result<HotSwapReport, String> {
    run_hot_swap_window_with_input_and_scene(config, source, start_gpu, |_| {}, |_| {}, debug)
}

pub fn run_hot_swap_window_with_input_and_scene(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    input: impl FnMut(KeyCode) + 'static,
    configure_scene: impl FnMut(&mut SnapshotScene) + 'static,
    debug: GraphicsDebugConfig,
) -> Result<HotSwapReport, String> {
    let mut app = Host {
        config,
        source,
        input: Box::new(input),
        configure_scene: Box::new(configure_scene),
        start_gpu,
        state: None,
        failure: None,
        report: HotSwapReport {
            transitions: 0,
            last_transition_ms: None,
            gpu_error: None,
        },
        debug,
        debug_frame: 0,
    };
    EventLoop::new()
        .map_err(|e| e.to_string())?
        .run_app(&mut app)
        .map_err(|e| e.to_string())?;
    app.failure.map_or_else(|| Ok(app.report), Err)
}

pub struct HotSwapReport {
    pub transitions: u64,
    pub last_transition_ms: Option<f64>,
    pub gpu_error: Option<String>,
}

struct Host {
    config: WindowConfig,
    source: SnapshotSource,
    input: Box<dyn FnMut(KeyCode)>,
    configure_scene: Box<dyn FnMut(&mut SnapshotScene)>,
    start_gpu: bool,
    state: Option<HostState>,
    failure: Option<String>,
    report: HotSwapReport,
    debug: GraphicsDebugConfig,
    debug_frame: u64,
}

struct HostState {
    window: Arc<Window>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    _context: Context<Arc<Window>>,
    framebuffer: Framebuffer,
    snapshot: Rc<SnapshotScene>,
    backend: HotSwapState<()>,
    buffer: SnapshotBuffer,
    started: Instant,
    gpu: Option<SurfaceRenderer>,
    occluded: bool,
    preset: Preset,
    cpu_surface_size: Option<(u32, u32)>,
    cpu_redraw: CpuRedrawGate,
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let size = self.config.preset.resolution();
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&self.config.title)
                .with_inner_size(LogicalSize::new(size.0, size.1)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.failure = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let context = match Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.failure = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let mut surface = match Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.failure = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let framebuffer = match Framebuffer::new(size.0, size.1) {
            Ok(f) => f,
            Err(e) => {
                self.failure = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let snapshot = match (self.source)(size.0, size.1) {
            Ok(s) => s,
            Err(e) => {
                self.failure = Some(e);
                event_loop.exit();
                return;
            }
        };
        if let Err(error) = surface.resize(
            NonZeroU32::new(size.0).unwrap(),
            NonZeroU32::new(size.1).unwrap(),
        ) {
            self.failure = Some(format!("initial CPU surface resize failed: {error}"));
            event_loop.exit();
            return;
        }
        let mut buffer = SnapshotBuffer::new(InterpolationConfig::default());
        if !buffer.push(snapshot, 0.0) {
            self.failure = Some("initial snapshot could not be buffered".into());
            event_loop.exit();
            return;
        }
        let Some(snapshot) = buffer.sample(0.0) else {
            self.failure = Some("initial snapshot unavailable".into());
            event_loop.exit();
            return;
        };
        let mut scene = SnapshotScene::default();
        (self.configure_scene)(&mut scene);
        if let Err(error) = scene.update(&snapshot) {
            self.failure = Some(error.to_string());
            event_loop.exit();
            return;
        }
        let snapshot = Rc::new(scene);
        let backend = HotSwapState::new(
            BackendKind::Cpu,
            BackendSnapshot {
                value: (),
                width: size.0,
                height: size.1,
            },
        );
        self.state = Some(HostState {
            window,
            surface: Some(surface),
            _context: context,
            framebuffer,
            snapshot,
            backend,
            gpu: None,
            occluded: false,
            preset: self.config.preset,
            cpu_surface_size: Some(size),
            buffer,
            started: Instant::now(),
            cpu_redraw: CpuRedrawGate::default(),
        });
        if self.start_gpu {
            self.toggle();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window_id) = self.state.as_ref().map(|state| state.window.id()) else {
            return;
        };
        if window_id != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Occluded(v) => {
                if let Some(state) = self.state.as_mut() {
                    state.occluded = v;
                }
            }
            WindowEvent::Resized(size) => {
                if let Err(error) = self.resize(size.width, size.height) {
                    self.failure = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyG => self.toggle(),
                        KeyCode::F9 | KeyCode::KeyM => self.capture_debug("marked"),
                        KeyCode::F10 => self.capture_images(),
                        KeyCode::KeyS if self.config.raster_controls => {
                            self.config.raster_mode = RasterMode::Secuencial
                        }
                        KeyCode::KeyP if self.config.raster_controls => {
                            self.config.raster_mode = RasterMode::Paralelo
                        }
                        KeyCode::KeyA if self.config.raster_controls => {
                            self.config.raster_mode = RasterMode::Auto
                        }
                        KeyCode::Digit1 => self.set_preset(Preset::Low),
                        KeyCode::Digit2 => self.set_preset(Preset::Medium),
                        KeyCode::Digit3 => self.set_preset(Preset::High),
                        _ => {}
                    }
                    (self.input)(code);
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            if backend_ready_for_redraw(
                state.backend.backend(),
                state.surface.is_some(),
                state.gpu.as_ref().is_some_and(SurfaceRenderer::ready_for_present),
                state.window.inner_size(),
                !state.cpu_redraw.is_blocked(),
            ) {
                state.window.request_redraw();
            }
        }
    }
}

impl Host {
    fn set_preset(&mut self, preset: Preset) {
        if let Some(state) = &mut self.state {
            let (width, height) = preset.resolution();
            if let Err(error) = state.framebuffer.resize(width, height) {
                self.report_gpu_error(format!("CPU framebuffer resize failed: {error}"));
                return;
            }
            state.preset = preset;
            if let Some(gpu) = &mut state.gpu {
                if preset == Preset::Low {
                    if let Err(error) = gpu.set_postprocess(render_gpu::PostProcessConfig::default()) {
                        self.report_gpu_error(error.to_string());
                    }
                } else {
                    if let Err(error) = gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High) {
                        self.report_gpu_error(error.to_string());
                    }
                }
            }
        }
        self.config.preset = preset;
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        let mut gpu_error = None;
        if let Some(state) = self.state.as_mut() {
            if state.backend.backend() == BackendKind::Cpu {
                if width == 0 || height == 0 {
                    state.surface = None;
                    state.cpu_surface_size = None;
                    state.cpu_redraw.reset();
                } else if let Some(surface) = &mut state.surface {
                    if !resize_is_needed(state.cpu_surface_size, (width, height)) {
                        state.cpu_redraw.reset();
                        return Ok(());
                    }
                    let result = NonZeroU32::new(width.max(1))
                        .zip(NonZeroU32::new(height.max(1)))
                        .ok_or_else(|| "invalid CPU surface dimensions".to_string())
                        .and_then(|(width, height)| {
                            surface
                                .resize(width, height)
                                .map_err(|error| error.to_string())
                        });
                    if let Err(error) = result {
                        state.surface = None;
                        state.cpu_surface_size = None;
                        state.cpu_redraw.block();
                        println!("CPU resize error={error}");
                    } else {
                        state.cpu_surface_size = Some((width, height));
                        state.cpu_redraw.reset();
                    }
                } else {
                    state.surface = Self::new_cpu_surface_for_size(state, width, height);
                    if state.surface.is_none() {
                        state.cpu_surface_size = None;
                        state.cpu_redraw.block();
                        println!("CPU surface recreation failed");
                    } else {
                        state.cpu_surface_size = Some((width, height));
                        state.cpu_redraw.reset();
                    }
                }
            }
            if let Some(gpu) = &mut state.gpu {
                let (_, _, _, gpu_width, gpu_height) = gpu.diagnostic_state();
                if (width, height) != (gpu_width, gpu_height) || !gpu.ready_for_present() {
                    if let Err(error) = gpu.resize(width, height) {
                        gpu_error = Some(error.to_string());
                    }
                }
            }
        }
        if let Some(error) = gpu_error {
            self.report_gpu_error(error);
            self.toggle();
            let fallback_ready = self.state.as_ref().is_some_and(|state| {
                cpu_fallback_is_confirmable(state.backend.backend(), state.surface.is_some())
            });
            if !fallback_ready {
                return Err("GPU resize failed and CPU fallback is unavailable".into());
            }
        }
        Ok(())
    }

    fn toggle(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let target = if state.backend.backend() == BackendKind::Cpu {
            BackendKind::Gpu
        } else {
            BackendKind::Cpu
        };
        let previous_gpu = state.gpu.take();
        let released_surface = if target == BackendKind::Gpu {
            state.surface.take()
        } else {
            None
        };
        let released_cpu_surface_size = if target == BackendKind::Gpu {
            state.cpu_surface_size.take()
        } else {
            None
        };
        let gpu = if target == BackendKind::Gpu {
            let mut gpu = match SurfaceRenderer::new(
                state.window.clone(),
                &state.snapshot.owned_draws(),
                GpuOptions::default(),
            ) {
                Ok(gpu) => gpu,
                Err(error) => {
                    state.surface = released_surface;
                    state.cpu_surface_size = released_cpu_surface_size;
                    state.cpu_redraw.reset();
                    self.report_gpu_error(error.to_string());
                    return;
                }
            };
            if state.config_preset() != Preset::Low {
                let _ = gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High);
            }
            Some(gpu)
        } else {
            let Some(surface) = Self::new_cpu_surface(state) else {
                state.gpu = previous_gpu;
                self.report_gpu_error("CPU presentation surface unavailable".into());
                return;
            };
            state.surface = Some(surface);
            let size = state.window.inner_size();
            state.cpu_surface_size = Some((size.width, size.height));
            state.cpu_redraw.reset();
            let internal = (state.framebuffer.width(), state.framebuffer.height());
            if let Ok(snapshot) = (self.source)(internal.0, internal.1) {
                if update_shared_scene(&mut state.snapshot, &snapshot).is_ok() {
                    state.backend.update_snapshot(BackendSnapshot {
                        value: (),
                        width: internal.0,
                        height: internal.1,
                    });
                }
            }
            None
        };
        let result = state.backend.try_switch(target, |_| Ok(()));
        if result.is_ok() {
            state.gpu = gpu;
            state.window.request_redraw();
        } else if target == BackendKind::Gpu {
            state.surface = released_surface;
            state.cpu_surface_size = released_cpu_surface_size;
            state.gpu = previous_gpu;
        }
        let transition_error = result.err();
        if let Some(error) = &transition_error {
            println!("hot-swap error={error}");
        } else {
            self.report.transitions = state.backend.transitions();
            self.report.last_transition_ms = state
                .backend
                .last_transition()
                .map(|d| d.as_secs_f64() * 1000.0);
            println!(
                "hot-swap backend={:?} nodes={} buffer={} transition_ms={:.3}",
                target,
                state.snapshot.snapshot().nodes.len(),
                state.buffer.len(),
                self.report.last_transition_ms.unwrap_or_default()
            );
        }
        let _ = state;
        if let Some(error) = transition_error {
            self.report_gpu_error(error);
        }
        self.capture_debug("transition");
    }
    fn report_gpu_error(&mut self, error: String) {
        self.report.gpu_error = Some(error);
    }

    fn capture_debug(&mut self, reason: &str) {
        let Some(directory) = self.debug.directory.as_ref() else {
            return;
        };
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let _ = std::fs::create_dir_all(directory);
        let path = directory.join(format!("frame-{:04}-{}.log", self.debug_frame, reason));
        self.debug_frame += 1;
        let size = state.window.inner_size();
        let prepared = state.snapshot.prepare(render_core::CullingConfig {
            width: size.width,
            height: size.height,
            ..Default::default()
        });
        let internal = (state.framebuffer.width(), state.framebuffer.height());
        let layout = ScaleLayout::fit(internal, (size.width, size.height))
            .map(|layout| {
                format!(
                    "{}x{}+{},{}",
                    layout.width, layout.height, layout.x, layout.y
                )
            })
            .unwrap_or_else(|_| "invalid".into());
        let vp = state.snapshot.view_projection();
        let gpu_state = state.gpu.as_ref().map(|gpu| {
            let (format, hdr, postprocess, width, height) = gpu.diagnostic_state();
            format!("gpu_format={format:?} hdr={hdr} postprocess={postprocess} gpu_size={width}x{height}")
        }).unwrap_or_else(|| "gpu_format=none hdr=false postprocess=false gpu_size=0x0".into());
        let text = format!("reason={reason}\nbackend={:?}\nphysical={}x{}\ninternal={}x{}\nscale_layout={layout}\nview_projection={vp:?}\ntick={}\nnodes={} visible={} discarded={} triangles={} draws={} buffer={}\n", state.backend.backend(), size.width, size.height, internal.0, internal.1, state.snapshot.snapshot().tick.0, state.snapshot.snapshot().nodes.len(), prepared.visible, prepared.discarded, prepared.triangles, prepared.draws().count(), state.buffer.len());
        let text = format!("preset={:?}\n{gpu_state}\n{text}", state.preset);
        if std::fs::write(&path, text).is_ok() {
            println!("graphics-debug capture={}", path.display());
        }
    }

    fn capture_images(&mut self) {
        let Some(directory) = self.debug.directory.as_ref() else {
            return;
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.backend.backend() != BackendKind::Cpu {
            return;
        }
        let size = state.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if !backend_ready_for_redraw(
            state.backend.backend(),
            state.surface.is_some(),
            state.gpu.as_ref().is_some_and(SurfaceRenderer::ready_for_present),
            size,
            !state.cpu_redraw.is_blocked(),
        ) {
            return;
        }
        let _ = std::fs::create_dir_all(directory);
        let internal_path =
            directory.join(format!("frame-{:04}-cpu-internal.png", self.debug_frame));
        let scaled_path = directory.join(format!("frame-{:04}-cpu-scaled.png", self.debug_frame));
        self.debug_frame += 1;
        let _ = render_snapshot_scene_with_shader(
            &mut state.framebuffer,
            &state.snapshot,
            self.config.raster_mode,
            FragmentShader::SolarBands,
        );
        let _ = write_png(&internal_path, &state.framebuffer);
        let mut pixels = vec![0u32; (size.width as usize) * (size.height as usize)];
        if copy_nearest_rgba(
            state.framebuffer.color_bytes(),
            (state.framebuffer.width(), state.framebuffer.height()),
            (size.width, size.height),
            &mut pixels,
        )
        .is_ok()
        {
            if let Ok(mut scaled) = Framebuffer::new(size.width, size.height) {
                for (dst, pixel) in scaled.color_bytes_mut().chunks_exact_mut(4).zip(pixels) {
                    dst[0] = (pixel >> 16) as u8;
                    dst[1] = (pixel >> 8) as u8;
                    dst[2] = pixel as u8;
                    dst[3] = 255;
                }
                let _ = write_png(&scaled_path, &scaled);
            }
        }
        println!(
            "graphics-debug images internal={} scaled={}",
            internal_path.display(),
            scaled_path.display()
        );
    }

    fn new_cpu_surface(state: &HostState) -> Option<Surface<Arc<Window>, Arc<Window>>> {
        let size = state.window.inner_size();
        Self::new_cpu_surface_for_size(state, size.width, size.height)
    }

    fn new_cpu_surface_for_size(
        state: &HostState,
        width: u32,
        height: u32,
    ) -> Option<Surface<Arc<Window>, Arc<Window>>> {
        if width == 0 || height == 0 {
            return None;
        }
        let mut surface = Surface::new(&state._context, state.window.clone()).ok()?;
        surface
            .resize(NonZeroU32::new(width)?, NonZeroU32::new(height)?)
            .ok()?;
        Some(surface)
    }

    fn draw(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.occluded {
            return;
        }
        let size = state.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if state.backend.backend() == BackendKind::Cpu && state.cpu_redraw.is_blocked() {
            return;
        }
        let internal = (state.framebuffer.width(), state.framebuffer.height());
        if let Ok(snapshot) = (self.source)(internal.0, internal.1) {
            let timestamp = state.started.elapsed().as_secs_f32();
            let _ = state.buffer.push(snapshot, timestamp);
            if let Some(interpolated) = state.buffer.sample(timestamp) {
                if update_shared_scene(&mut state.snapshot, &interpolated).is_err() {
                    return;
                }
                if state.snapshot.overlay().game_over {
                    state.window.set_title("Astral Echo | GAME OVER");
                }
                state.backend.update_snapshot(BackendSnapshot {
                    value: (),
                    width: size.width,
                    height: size.height,
                });
            }
        }
        if state.backend.backend() == BackendKind::Gpu {
            let gpu_error = match state.gpu.as_mut() {
                Some(gpu) => match gpu.present_snapshot_with_stats(&state.snapshot) {
                    Ok((_, stats)) => {
                        println!(
                            "hot-swap frame backend={:?} tick={} nodes={} considered={} visible={} discarded={} triangles={}",
                            state.backend.backend(),
                            state.snapshot.snapshot().tick.0,
                            state.snapshot.snapshot().nodes.len(),
                            stats.considered,
                            stats.visible,
                            stats.discarded,
                            stats.triangles
                        );
                        None
                    }
                    Err(error) => Some(error.to_string()),
                },
                None => Some("GPU backend active without GPU resources".into()),
            };
            if let Some(error) = gpu_error {
                let _ = state;
                self.report_gpu_error(error);
                self.toggle();
            }
            return;
        }
        if let Ok((stats, _)) = render_snapshot_scene_with_shader_with_stats(
            &mut state.framebuffer,
            &state.snapshot,
            self.config.raster_mode,
            FragmentShader::SolarBands,
        ) {
            println!(
                "hot-swap frame backend={:?} tick={} nodes={} considered={} visible={} discarded={} triangles={}",
                state.backend.backend(),
                state.snapshot.snapshot().tick.0,
                state.snapshot.snapshot().nodes.len(),
                stats.considered,
                stats.visible,
                stats.discarded,
                stats.triangles
            );
        }
        let presentation_error = if let Some(surface) = &mut state.surface {
            match surface.buffer_mut() {
                Ok(mut buffer) => {
                    if buffer.width().get() != size.width || buffer.height().get() != size.height {
                        if self.debug.directory.is_some() {
                            println!(
                                "graphics-debug softbuffer-size requested={}x{} actual={}x{} len={} expected={}",
                                size.width,
                                size.height,
                                buffer.width(),
                                buffer.height(),
                                buffer.len(),
                                u64::from(size.width) * u64::from(size.height)
                            );
                        }
                        Some(format!(
                            "CPU presentation buffer dimensions mismatch: requested={}x{} actual={}x{}",
                            size.width,
                            size.height,
                            buffer.width(),
                            buffer.height()
                        ))
                    } else {
                        if let Err(error) = copy_nearest_rgba(
                            state.framebuffer.color_bytes(),
                            (state.framebuffer.width(), state.framebuffer.height()),
                            (size.width, size.height),
                            &mut buffer,
                        ) {
                            Some(format!("CPU framebuffer copy failed: {error}"))
                        } else {
                            buffer
                                .present()
                                .err()
                                .map(|error| format!("CPU presentation failed: {error}"))
                        }
                    }
                }
                Err(error) => Some(format!("CPU buffer acquisition failed: {error}")),
            }
        } else {
            Some("CPU presentation surface unavailable".into())
        };
        if let Some(error) = presentation_error {
            state.cpu_redraw.block();
            self.report_gpu_error(error);
        }
    }
}

fn update_shared_scene(
    scene: &mut Rc<SnapshotScene>,
    snapshot: &RenderSnapshot,
) -> Result<render_core::SceneUpdate, String> {
    Rc::get_mut(scene)
        .ok_or_else(|| "snapshot scene is shared during update".to_string())?
        .update(snapshot)
        .map_err(|error| error.to_string())
}

fn cpu_fallback_is_confirmable(backend: BackendKind, surface_ready: bool) -> bool {
    backend == BackendKind::Cpu && surface_ready
}

fn backend_ready_for_redraw(
    backend: BackendKind,
    cpu_surface_ready: bool,
    gpu_ready: bool,
    window_size: winit::dpi::PhysicalSize<u32>,
    cpu_redraw_allowed: bool,
) -> bool {
    if window_size.width == 0 || window_size.height == 0 {
        return false;
    }
    match backend {
        BackendKind::Cpu => cpu_surface_ready && cpu_redraw_allowed,
        BackendKind::Gpu => gpu_ready,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CpuRedrawGate {
    blocked: bool,
}

impl CpuRedrawGate {
    fn block(&mut self) {
        self.blocked = true;
    }

    fn reset(&mut self) {
        self.blocked = false;
    }

    fn is_blocked(self) -> bool {
        self.blocked
    }
}

fn resize_is_needed(current: Option<(u32, u32)>, requested: (u32, u32)) -> bool {
    current != Some(requested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_reuse_the_same_scene_instance() {
        let mut scene = Rc::new(SnapshotScene::default());
        let pointer = Rc::as_ptr(&scene);

        update_shared_scene(&mut scene, &RenderSnapshot::minimal_demo()).unwrap();
        update_shared_scene(&mut scene, &RenderSnapshot::minimal_demo()).unwrap();

        assert_eq!(Rc::as_ptr(&scene), pointer);
        assert_eq!(Rc::strong_count(&scene), 1);
    }

    #[test]
    fn shared_scene_is_not_mutated_without_unique_ownership() {
        let mut scene = Rc::new(SnapshotScene::default());
        let shared = scene.clone();

        assert!(update_shared_scene(&mut scene, &RenderSnapshot::minimal_demo()).is_err());
        assert_eq!(Rc::strong_count(&shared), 2);
    }

    #[test]
    fn cpu_fallback_requires_cpu_backend_and_ready_surface() {
        assert!(cpu_fallback_is_confirmable(BackendKind::Cpu, true));
        assert!(!cpu_fallback_is_confirmable(BackendKind::Cpu, false));
        assert!(!cpu_fallback_is_confirmable(BackendKind::Gpu, true));
    }

    #[test]
    fn redraw_is_disabled_until_the_active_backend_has_resources() {
        assert!(backend_ready_for_redraw(BackendKind::Cpu, true, false, (960, 540).into(), true));
        assert!(!backend_ready_for_redraw(BackendKind::Cpu, false, false, (960, 540).into(), true));
        assert!(backend_ready_for_redraw(BackendKind::Gpu, false, true, (960, 540).into(), false));
        assert!(!backend_ready_for_redraw(BackendKind::Gpu, false, false, (960, 540).into(), false));
        assert!(!backend_ready_for_redraw(BackendKind::Cpu, true, false, (960, 540).into(), false));
        assert!(!backend_ready_for_redraw(BackendKind::Cpu, true, false, (0, 540).into(), true));
        assert!(!backend_ready_for_redraw(BackendKind::Gpu, false, true, (960, 0).into(), true));
    }

    #[test]
    fn cpu_redraw_gate_recovers_after_a_validating_event() {
        let mut gate = CpuRedrawGate::default();
        assert!(!gate.is_blocked());

        gate.block();
        assert!(gate.is_blocked());
        assert!(!backend_ready_for_redraw(BackendKind::Cpu, true, false, (960, 540).into(), !gate.is_blocked()));

        gate.reset();
        assert!(!gate.is_blocked());
        assert!(backend_ready_for_redraw(BackendKind::Cpu, true, false, (960, 540).into(), !gate.is_blocked()));
    }

    #[test]
    fn duplicate_resize_is_skipped_only_for_confirmed_dimensions() {
        assert!(!resize_is_needed(Some((960, 540)), (960, 540)));
        assert!(resize_is_needed(Some((960, 540)), (1280, 720)));
        assert!(resize_is_needed(None, (960, 540)));
    }
}

impl HostState {
    fn config_preset(&self) -> Preset {
        self.preset
    }
}
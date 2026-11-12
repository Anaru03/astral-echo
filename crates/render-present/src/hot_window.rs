use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use render_core::{
    hud_contextual_button_hit, hud_controls_button_hit, hud_controls_panel_hit,
    InterpolationConfig, LodLevel, MeshHandle, RenderSnapshot, SnapshotBuffer, SnapshotScene,
    VisualAssets,
};
use render_cpu::{
    render_snapshot_scene_with_shader, render_snapshot_scene_with_stats_and_scratch, write_png,
    CpuPipelineScratch, FragmentShader, Framebuffer, RasterMode,
};
use render_gpu::{GpuError, GpuOptions, SurfaceRenderer};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::{
    copy_nearest_rgba, BackendKind, BackendSnapshot, DeferredMeshRegistry, HotSwapState, Preset,
    ScaleLayout, VisualAssetHost, WindowConfig,
};

pub type SnapshotSource = Box<dyn FnMut(u32, u32) -> Result<RenderSnapshot, String>>;

#[derive(Clone, Debug, Default)]
pub struct GraphicsDebugConfig {
    pub directory: Option<std::path::PathBuf>,
    pub dual_capture: bool,
}

fn parse_auto_capture_after_frames(value: Option<std::ffi::OsString>) -> Option<u64> {
    value
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
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
        |_, _| {},
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
    run_hot_swap_window_with_input_and_scene(config, source, start_gpu, |_, _| {}, |_| {}, debug)
}

pub fn run_hot_swap_window_with_input_and_scene(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    input: impl FnMut(KeyCode, ElementState) + 'static,
    configure_scene: impl FnMut(&mut SnapshotScene) + 'static,
    debug: GraphicsDebugConfig,
) -> Result<HotSwapReport, String> {
    run_hot_swap_window_with_input_and_scene_and_focus(
        config,
        source,
        start_gpu,
        input,
        |_| {},
        configure_scene,
        debug,
    )
}

pub fn run_hot_swap_window_with_input_and_scene_and_focus(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    input: impl FnMut(KeyCode, ElementState) + 'static,
    focus: impl FnMut(bool) + 'static,
    configure_scene: impl FnMut(&mut SnapshotScene) + 'static,
    debug: GraphicsDebugConfig,
) -> Result<HotSwapReport, String> {
    run_hot_swap_window_with_input_and_scene_and_focus_and_assets(
        config,
        source,
        start_gpu,
        input,
        focus,
        configure_scene,
        debug,
        VisualAssetHost::default(),
    )
}

pub fn run_hot_swap_window_with_input_and_scene_and_focus_and_assets(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    input: impl FnMut(KeyCode, ElementState) + 'static,
    focus: impl FnMut(bool) + 'static,
    configure_scene: impl FnMut(&mut SnapshotScene) + 'static,
    debug: GraphicsDebugConfig,
    visual_assets: VisualAssetHost,
) -> Result<HotSwapReport, String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let wake_proxy = event_loop.create_proxy();
    let mut app = Host {
        config,
        source,
        input: Box::new(input),
        focus: Box::new(focus),
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
        frames: 0,
        auto_capture_after_frames: parse_auto_capture_after_frames(std::env::var_os(
            "ASTRAL_ECHO_DUAL_CAPTURE_AFTER_FRAMES",
        )),
        auto_capture_requested: false,
        wake_proxy,
        visual_assets,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.failure.map_or_else(|| Ok(app.report), Err)
}

pub struct HotSwapReport {
    pub transitions: u64,
    pub last_transition_ms: Option<f64>,
    pub gpu_error: Option<String>,
}

impl HotSwapReport {
    fn record_transition(&mut self, transitions: u64, duration_ms: Option<f64>) {
        self.transitions = transitions;
        self.last_transition_ms = duration_ms;
    }
}

struct Host {
    config: WindowConfig,
    source: SnapshotSource,
    input: Box<dyn FnMut(KeyCode, ElementState)>,
    focus: Box<dyn FnMut(bool)>,
    configure_scene: Box<dyn FnMut(&mut SnapshotScene)>,
    start_gpu: bool,
    state: Option<HostState>,
    failure: Option<String>,
    report: HotSwapReport,
    debug: GraphicsDebugConfig,
    debug_frame: u64,
    frames: u64,
    auto_capture_after_frames: Option<u64>,
    auto_capture_requested: bool,
    wake_proxy: EventLoopProxy<()>,
    visual_assets: VisualAssetHost,
}

struct HostState {
    window: Arc<Window>,
    window_title: String,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    _context: Context<Arc<Window>>,
    framebuffer: Framebuffer,
    cpu_scratch: CpuPipelineScratch,
    snapshot: Rc<SnapshotScene>,
    backend: HotSwapState<()>,
    buffer: SnapshotBuffer,
    started: Instant,
    gpu: Option<SurfaceRenderer>,
    occluded: bool,
    preset: Preset,
    cpu_surface_size: Option<(u32, u32)>,
    cpu_redraw: CpuRedrawGate,
    pending_gpu: Option<PendingGpu>,
    pending_gpu_started: Option<Instant>,
    cursor_position: (f64, f64),
    deferred_meshes: DeferredMeshRegistry,
    visual_assets: VisualAssetHost,
}

struct PendingGpu {
    result: Receiver<Result<SurfaceRenderer, GpuError>>,
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
            window_title: String::new(),
            surface: Some(surface),
            _context: context,
            framebuffer,
            cpu_scratch: CpuPipelineScratch::default(),
            snapshot,
            backend,
            gpu: None,
            occluded: false,
            preset: self.config.preset,
            cpu_surface_size: Some(size),
            buffer,
            started: Instant::now(),
            cpu_redraw: CpuRedrawGate::default(),
            pending_gpu: None,
            pending_gpu_started: None,
            cursor_position: (0.0, 0.0),
            deferred_meshes: DeferredMeshRegistry::default(),
            visual_assets: std::mem::replace(&mut self.visual_assets, VisualAssetHost::default()),
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
            WindowEvent::Focused(focused) => {
                (self.focus)(focused);
                if focused {
                    return;
                }
                for code in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD] {
                    (self.input)(code, ElementState::Released);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if !event.repeat => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        match code {
                            KeyCode::KeyG if self.config.raster_controls => self.toggle(),
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
                    }
                    (self.input)(code, event.state);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = self.state.as_mut() {
                    state.cursor_position = (position.x, position.y);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let input_action = self.state.as_ref().and_then(|state| {
                    let (x, y) = state.cursor_position;
                    let hud = &state.snapshot.overlay().hud;
                    let x = x.max(0.0) as u32;
                    let y = y.max(0.0) as u32;
                    if hud.controls_panel.open {
                        hud_controls_panel_hit(
                            hud,
                            state.framebuffer.width(),
                            state.framebuffer.height(),
                            x,
                            y,
                        )
                        .then_some(KeyCode::F1)
                    } else if hud_controls_button_hit(
                        hud,
                        state.framebuffer.width(),
                        state.framebuffer.height(),
                        x,
                        y,
                    ) {
                        Some(KeyCode::F1)
                    } else if hud_contextual_button_hit(
                        hud,
                        state.framebuffer.width(),
                        state.framebuffer.height(),
                        x,
                        y,
                    ) {
                        Some(KeyCode::KeyI)
                    } else {
                        None
                    }
                });
                if let Some(key) = input_action {
                    (self.input)(key, ElementState::Pressed);
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_pending_gpu(_event_loop);
        if self
            .config
            .max_frames
            .is_some_and(|limit| self.frames >= limit.get())
        {
            _event_loop.exit();
            return;
        }
        if let Some(state) = &self.state {
            if backend_ready_for_redraw(
                state.backend.backend(),
                state.surface.is_some(),
                state
                    .gpu
                    .as_ref()
                    .is_some_and(SurfaceRenderer::ready_for_present),
                state.window.inner_size(),
                !state.cpu_redraw.is_blocked(),
            ) {
                state.window.request_redraw();
            }
            if state.pending_gpu.is_some() {
                if self.frames % 60 == 0 {
                    println!(
                        "hot-swap about-to-wait pending frames={} backend={:?}",
                        self.frames,
                        state.backend.backend()
                    );
                }
                _event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        println!(
            "hot-swap user-event pending={}",
            self.state
                .as_ref()
                .is_some_and(|state| state.pending_gpu.is_some())
        );
        self.poll_pending_gpu(event_loop);
    }
}

impl Host {
    fn poll_pending_gpu(&mut self, event_loop: &ActiveEventLoop) {
        let Some(pending) = self
            .state
            .as_mut()
            .and_then(|state| state.pending_gpu.take())
        else {
            return;
        };
        match pending.result.try_recv() {
            Err(TryRecvError::Empty) => {
                if let Some(state) = self.state.as_mut() {
                    state.pending_gpu = Some(pending);
                }
                println!("hot-swap gpu-worker pending");
                event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
            Ok(Ok(mut gpu)) => {
                println!("hot-swap gpu-worker ready");
                let preparation_ms = self
                    .state
                    .as_mut()
                    .and_then(|state| state.pending_gpu_started.take())
                    .map(|started| started.elapsed().as_secs_f64() * 1000.0);
                let Some(state) = self.state.as_mut() else {
                    return;
                };
                let size = state.window.inner_size();
                if size.width == 0 || size.height == 0 {
                    state.gpu = Some(gpu);
                    state
                        .backend
                        .cancel_switch("GPU presentation dimensions are zero");
                    self.report_gpu_error("GPU presentation dimensions are zero".into());
                    return;
                }
                let (_, _, _, gpu_width, gpu_height) = gpu.diagnostic_state();
                if (size.width, size.height) != (gpu_width, gpu_height) || !gpu.ready_for_present()
                {
                    if let Err(error) = gpu.resize_without_wait(size.width, size.height) {
                        state.backend.cancel_switch(error.to_string());
                        self.report_gpu_error(error.to_string());
                        return;
                    }
                }
                state.surface = None;
                state.cpu_surface_size = None;
                state.gpu = Some(gpu);
                let confirmed = state.backend.confirm_switch(BackendKind::Gpu).is_ok();
                let transitions = state.backend.transitions();
                if confirmed {
                    println!(
                        "hot-swap gpu-ready preparation_ms={:.3}",
                        preparation_ms.unwrap_or_default()
                    );
                    state.window.request_redraw();
                    let _ = state;
                    self.report.record_transition(transitions, preparation_ms);
                }
            }
            Ok(Err(error)) => {
                println!("hot-swap gpu-worker error={error}");
                let preparation_ms = self
                    .state
                    .as_mut()
                    .and_then(|state| state.pending_gpu_started.take())
                    .map(|started| started.elapsed().as_secs_f64() * 1000.0);
                if let Some(state) = self.state.as_mut() {
                    state.backend.cancel_switch(error.to_string());
                }
                println!(
                    "hot-swap gpu-failed preparation_ms={:.3} error={error}",
                    preparation_ms.unwrap_or_default()
                );
                self.report_gpu_error(error.to_string());
            }
            Err(TryRecvError::Disconnected) => {
                let error = GpuError::Backend("GPU preparation worker disconnected".into());
                println!("hot-swap gpu-worker error={error}");
                let preparation_ms = self
                    .state
                    .as_mut()
                    .and_then(|state| state.pending_gpu_started.take())
                    .map(|started| started.elapsed().as_secs_f64() * 1000.0);
                if let Some(state) = self.state.as_mut() {
                    state.backend.cancel_switch(error.to_string());
                }
                println!(
                    "hot-swap gpu-failed preparation_ms={:.3} error={error}",
                    preparation_ms.unwrap_or_default()
                );
                self.report_gpu_error(error.to_string());
            }
        }
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.pending_gpu.is_none())
        {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }

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
                    if let Err(error) =
                        gpu.set_postprocess(render_gpu::PostProcessConfig::default())
                    {
                        self.report_gpu_error(error.to_string());
                    }
                } else {
                    if let Err(error) =
                        gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High)
                    {
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
            if state.backend.backend() == BackendKind::Gpu {
                if let Some(gpu) = &mut state.gpu {
                    let (_, _, _, gpu_width, gpu_height) = gpu.diagnostic_state();
                    if (width, height) != (gpu_width, gpu_height) || !gpu.ready_for_present() {
                        if let Err(error) = gpu.resize(width, height) {
                            gpu_error = Some(error.to_string());
                        }
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
        println!(
            "hot-swap key-g enter backend={:?} pending={}",
            state.backend.backend(),
            state.pending_gpu.is_some()
        );
        if state.pending_gpu.is_some() {
            println!("hot-swap key-g exit pending-request-ignored");
            return;
        }
        let target = if state.backend.backend() == BackendKind::Cpu {
            BackendKind::Gpu
        } else {
            BackendKind::Cpu
        };
        if !state.backend.request_switch(target) {
            return;
        }
        let transition_started = Instant::now();
        let previous_gpu = state.gpu.take();
        let gpu = if target == BackendKind::Gpu {
            let mut gpu = match previous_gpu {
                Some(gpu) => gpu,
                None => {
                    state.pending_gpu_started = Some(Instant::now());
                    println!("hot-swap gpu-requested");
                    let window = state.window.clone();
                    // The new renderer starts empty. Shared meshes are
                    // prepared from SnapshotScene after the backend is live,
                    // avoiding a full RasterMesh clone during reconstruction.
                    let scene = Vec::new();
                    let high_preset = state.config_preset() != Preset::Low;
                    let window_surface = match SurfaceRenderer::create_window_surface(
                        window.clone(),
                        GpuOptions::default(),
                    ) {
                        Ok(surface) => surface,
                        Err(error) => {
                            state.backend.cancel_switch(error.to_string());
                            state.pending_gpu_started = None;
                            self.report_gpu_error(error.to_string());
                            println!("hot-swap gpu-surface error={error}");
                            return;
                        }
                    };
                    let (sender, receiver) = mpsc::sync_channel(1);
                    let wake_proxy = self.wake_proxy.clone();
                    let worker = std::thread::Builder::new()
                        .name("astral-echo-gpu-init".into())
                        .spawn(move || {
                            println!("hot-swap gpu-worker start");
                            let result = render_gpu::block_on(async move {
                                let mut gpu = SurfaceRenderer::new_async_with_surface(
                                    window,
                                    window_surface,
                                    &scene,
                                    GpuOptions::default(),
                                )
                                .await?;
                                if high_preset {
                                    gpu.set_postprocess_preset_async(
                                        render_gpu::PostProcessPreset::High,
                                    )
                                    .await?;
                                }
                                Ok(gpu)
                            });
                            println!("hot-swap gpu-worker finished");
                            let _ = sender.send(result);
                            let _ = wake_proxy.send_event(());
                        });
                    if worker.is_err() {
                        state
                            .backend
                            .cancel_switch("could not start GPU preparation worker");
                        state.pending_gpu_started = None;
                        self.report_gpu_error("could not start GPU preparation worker".into());
                        println!("hot-swap gpu-worker spawn-error");
                        return;
                    }
                    state.pending_gpu = Some(PendingGpu { result: receiver });
                    println!("hot-swap key-g exit pending=true");
                    return;
                }
            };
            let size = state.window.inner_size();
            if size.width == 0 || size.height == 0 {
                state.gpu = Some(gpu);
                state
                    .backend
                    .cancel_switch("GPU presentation dimensions are zero");
                self.report_gpu_error("GPU presentation dimensions are zero".into());
                return;
            }
            let (_, _, _, gpu_width, gpu_height) = gpu.diagnostic_state();
            if (size.width, size.height) != (gpu_width, gpu_height) || !gpu.ready_for_present() {
                if let Err(error) = gpu.resize_without_wait(size.width, size.height) {
                    state.gpu = Some(gpu);
                    state.backend.cancel_switch(error.to_string());
                    self.report_gpu_error(error.to_string());
                    return;
                }
            }
            state.surface = None;
            state.cpu_surface_size = None;
            Some(gpu)
        } else {
            let Some(surface) = Self::new_cpu_surface(state) else {
                state.gpu = previous_gpu;
                state
                    .backend
                    .cancel_switch("CPU presentation surface unavailable");
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
            // A successful CPU switch deliberately drops the old GPU
            // renderer. A failed device must never be reused later.
            None
        };
        let result = state.backend.confirm_switch(target);
        if result.is_ok() {
            state.gpu = gpu;
            state.window.request_redraw();
        }
        let transition_error = result.err();
        if let Some(error) = &transition_error {
            println!("hot-swap error={error}");
        } else {
            let duration_ms = Some(transition_started.elapsed().as_secs_f64() * 1000.0);
            self.report
                .record_transition(state.backend.transitions(), duration_ms);
            println!(
                "hot-swap backend={:?} nodes={} buffer={} transition_ms={:.3}",
                target,
                state.snapshot.snapshot().nodes.len(),
                state.buffer.len(),
                self.report.last_transition_ms.unwrap_or_default()
            );
        }
        let pending = state.pending_gpu.is_some();
        println!(
            "hot-swap key-g exit backend={:?} pending={}",
            target, pending
        );
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
        let _ = std::fs::create_dir_all(&directory);
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
        let Some(directory) = self.debug.directory.clone() else {
            return;
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.backend.backend() != BackendKind::Cpu {
            let frame = self.debug_frame;
            self.debug_frame += 1;
            match state.gpu.as_mut() {
                Some(gpu) if self.debug.dual_capture => {
                    let (scene_only, composite) =
                        Self::unique_dual_capture_paths(&directory, frame);
                    match gpu.request_dual_capture(scene_only.clone(), composite.clone()) {
                        Ok(()) => println!(
                            "graphics-debug gpu-dual-capture-requested scene_only={} composite={}",
                            scene_only.display(),
                            composite.display()
                        ),
                        Err(error) => println!("graphics-debug gpu-capture-error={error}"),
                    }
                }
                Some(gpu) => {
                    let path = Self::unique_capture_path(&directory, frame, "gpu");
                    match gpu.request_capture(path.clone()) {
                        Ok(()) => {
                            println!("graphics-debug gpu-capture-requested={}", path.display())
                        }
                        Err(error) => println!("graphics-debug gpu-capture-error={error}"),
                    }
                }
                None => println!("graphics-debug gpu-capture-error=GPU backend unavailable"),
            }
            return;
        }
        let size = state.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if !backend_ready_for_redraw(
            state.backend.backend(),
            state.surface.is_some(),
            state
                .gpu
                .as_ref()
                .is_some_and(SurfaceRenderer::ready_for_present),
            size,
            !state.cpu_redraw.is_blocked(),
        ) {
            return;
        }
        let _ = std::fs::create_dir_all(&directory);
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

    fn unique_capture_path(directory: &std::path::Path, frame: u64, backend: &str) -> PathBuf {
        let mut index = 0_u32;
        loop {
            let suffix = if index == 0 {
                String::new()
            } else {
                format!("-{index}")
            };
            let path = directory.join(format!("frame-{frame:04}-{backend}{suffix}.png"));
            if !path.exists() {
                return path;
            }
            index = index.saturating_add(1);
        }
    }

    fn unique_dual_capture_paths(directory: &std::path::Path, frame: u64) -> (PathBuf, PathBuf) {
        let mut index = 0_u32;
        loop {
            let suffix = if index == 0 {
                String::new()
            } else {
                format!("-{index}")
            };
            let scene_only = directory.join(format!(
                "building_planet_scene_only_frame-{frame:04}{suffix}.png"
            ));
            let composite = directory.join(format!(
                "building_planet_composite_frame-{frame:04}{suffix}.png"
            ));
            if !scene_only.exists() && !composite.exists() {
                return (scene_only, composite);
            }
            index = index.saturating_add(1);
        }
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
        self.frames = self.frames.saturating_add(1);
        if let Some(scene) = Rc::get_mut(&mut state.snapshot) {
            let _ = state
                .visual_assets
                .poll(&mut state.deferred_meshes, Some(scene));
        }
        state
            .visual_assets
            .set_gpu_backend_active(state.backend.backend() == BackendKind::Gpu);
        let frame_id = self.frames;
        let trace_frame = frame_id <= 3 || frame_id % 60 == 0 || state.pending_gpu.is_some();
        let frame_started = Instant::now();
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
                let title = hud_title(
                    &self.config.title,
                    state.backend.backend(),
                    state.snapshot.snapshot(),
                );
                if state.window_title != title {
                    state.window.set_title(&title);
                    state.window_title = title;
                }
                state.backend.update_snapshot(BackendSnapshot {
                    value: (),
                    width: size.width,
                    height: size.height,
                });
            }
        }
        if state.backend.backend() == BackendKind::Gpu {
            let auto_capture_due = self
                .auto_capture_after_frames
                .is_some_and(|threshold| frame_id >= threshold)
                && !self.auto_capture_requested;
            let gpu_error = match state.gpu.as_mut() {
                Some(gpu) => {
                    if let Ok(nareth) = state
                        .snapshot
                        .shared_mesh(MeshHandle::NARETH_PLANET, VisualAssets::PLANET)
                    {
                        let _ = gpu.ensure_shared_mesh(
                            MeshHandle::NARETH_PLANET,
                            LodLevel::High,
                            nareth,
                        );
                    }
                    match gpu.present_snapshot_with_stats(&state.snapshot) {
                        Ok((_, stats)) => {
                            let gpu_events = gpu.poll_gpu_asset_events();
                            state
                                .visual_assets
                                .publish_gpu_events(gpu_events.iter().cloned());
                            for event in gpu_events {
                                println!(
                                "hot-swap gpu-asset mesh={} lod={:?} generation={} state={:?} error={:?}",
                                event.mesh.0,
                                event.lod,
                                event.generation,
                                event.state,
                                event.error
                            );
                            }
                            if trace_frame {
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
                                println!(
                                "hot-swap frame-presented backend=GPU frame={} elapsed_ms={:.3}",
                                frame_id,
                                frame_started.elapsed().as_secs_f64() * 1000.0
                            );
                            }
                            if auto_capture_due {
                                if let Some(directory) = self.debug.directory.clone() {
                                    if self.debug.dual_capture {
                                        let capture_frame = self.debug_frame;
                                        let (scene_only, composite) =
                                            Self::unique_dual_capture_paths(
                                                &directory,
                                                capture_frame,
                                            );
                                        match gpu.request_dual_capture(
                                            scene_only.clone(),
                                            composite.clone(),
                                        ) {
                                            Ok(()) => {
                                                self.auto_capture_requested = true;
                                                self.debug_frame =
                                                    self.debug_frame.saturating_add(1);
                                                println!(
                                                "graphics-debug gpu-auto-dual-capture-requested frame={} scene_only={} composite={}",
                                                frame_id,
                                                scene_only.display(),
                                                composite.display()
                                            );
                                            }
                                            Err(error) => println!(
                                                "graphics-debug gpu-auto-capture-error={error}"
                                            ),
                                        }
                                    }
                                }
                            }
                            None
                        }
                        Err(error) => Some(error.to_string()),
                    }
                }
                None => Some("GPU backend active without GPU resources".into()),
            };
            if let Some(error) = gpu_error {
                // A failed GPU renderer is not reusable. Drop it before the
                // controlled CPU transition; the registered CPU mesh remains
                // owned by SnapshotScene/VisualAssets.
                if let Some(state) = self.state.as_mut() {
                    state.gpu = None;
                }
                self.report_gpu_error(error);
                self.toggle();
            }
            return;
        }
        if let Ok((stats, _)) = render_snapshot_scene_with_stats_and_scratch(
            &mut state.framebuffer,
            &state.snapshot,
            self.config.raster_mode,
            &mut state.cpu_scratch,
        ) {
            if trace_frame {
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
        } else if trace_frame {
            println!(
                "hot-swap frame-presented backend=CPU frame={} elapsed_ms={:.3}",
                frame_id,
                frame_started.elapsed().as_secs_f64() * 1000.0
            );
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

fn hud_title(base: &str, backend: BackendKind, snapshot: &RenderSnapshot) -> String {
    if snapshot.overlay.game_over {
        return format!("{base} | GAME OVER");
    }
    let backend = match backend {
        BackendKind::Cpu => "CPU",
        BackendKind::Gpu => "GPU",
    };
    let flight = match snapshot.overlay.hud.flight {
        render_core::HudFlightState::Free => "libre",
        render_core::HudFlightState::Atmospheric => "atmosfera",
        render_core::HudFlightState::Landing => "aterrizaje",
        render_core::HudFlightState::Landed => "aterrizado",
        render_core::HudFlightState::Impacted => "impactado",
    };
    let autopilot = match snapshot.overlay.hud.autopilot {
        render_core::HudAutopilotState::Inactive => "inactivo",
        render_core::HudAutopilotState::Planned => "planificado",
        render_core::HudAutopilotState::InTransit => "en_transito",
        render_core::HudAutopilotState::Completed => "completado",
        render_core::HudAutopilotState::Interrupted => "interrumpido",
        render_core::HudAutopilotState::Failed => "fallido",
    };
    let target = snapshot
        .overlay
        .hud
        .target
        .map_or_else(|| "ninguno".to_owned(), |value| value.to_string());
    let landing = if snapshot.overlay.hud.landing_available {
        " | F aterrizar"
    } else {
        ""
    };
    let mission = snapshot
        .overlay
        .hud
        .mission_name
        .as_deref()
        .unwrap_or("sin_mision");
    let objective = snapshot
        .overlay
        .hud
        .objective_name
        .as_deref()
        .unwrap_or("sin_objetivo");
    let progress = snapshot
        .overlay
        .hud
        .mission_progress
        .map_or_else(|| "-".to_owned(), |(done, total)| format!("{done}/{total}"));
    let mission_status = match snapshot.overlay.hud.mission_state {
        Some(render_core::HudMissionState::Locked) => "bloqueada",
        Some(render_core::HudMissionState::Available) => "disponible",
        Some(render_core::HudMissionState::Active) => "activa",
        Some(render_core::HudMissionState::Failed) => "fallida",
        Some(render_core::HudMissionState::Completed) => "completada",
        None => "sin_estado",
    };
    let feedback = snapshot
        .overlay
        .hud
        .feedback
        .as_deref()
        .unwrap_or("sin_feedback");
    let feedback = feedback.chars().take(180).collect::<String>();
    format!(
        "{base} | {backend} | tick={} | mision={mission} ({mission_status} {progress}) | mision_objetivo={objective} | objetivo={target} | vuelo={flight} | auto={autopilot} | enemigos={} | feedback={feedback} | M iniciar | I interactuar | R energia | Y oxigeno | O observar | P vuelo | Flechas orbitar | Espacio tiempo | F aterrizar/atacar{landing}",
        snapshot.tick.0,
        snapshot.overlay.hud.active_enemies,
    )
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
    fn automatic_capture_frame_configuration_requires_a_positive_integer() {
        assert_eq!(parse_auto_capture_after_frames(None), None);
        assert_eq!(parse_auto_capture_after_frames(Some("".into())), None);
        assert_eq!(
            parse_auto_capture_after_frames(Some("invalid".into())),
            None
        );
        assert_eq!(parse_auto_capture_after_frames(Some("0".into())), None);
        assert_eq!(parse_auto_capture_after_frames(Some("-1".into())), None);
        assert_eq!(parse_auto_capture_after_frames(Some("12".into())), Some(12));
    }

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
        assert!(backend_ready_for_redraw(
            BackendKind::Cpu,
            true,
            false,
            (960, 540).into(),
            true
        ));
        assert!(!backend_ready_for_redraw(
            BackendKind::Cpu,
            false,
            false,
            (960, 540).into(),
            true
        ));
        assert!(backend_ready_for_redraw(
            BackendKind::Gpu,
            false,
            true,
            (960, 540).into(),
            false
        ));
        assert!(!backend_ready_for_redraw(
            BackendKind::Gpu,
            false,
            false,
            (960, 540).into(),
            false
        ));
        assert!(!backend_ready_for_redraw(
            BackendKind::Cpu,
            true,
            false,
            (960, 540).into(),
            false
        ));
        assert!(!backend_ready_for_redraw(
            BackendKind::Cpu,
            true,
            false,
            (0, 540).into(),
            true
        ));
        assert!(!backend_ready_for_redraw(
            BackendKind::Gpu,
            false,
            true,
            (960, 0).into(),
            true
        ));
    }

    #[test]
    fn cpu_redraw_gate_recovers_after_a_validating_event() {
        let mut gate = CpuRedrawGate::default();
        assert!(!gate.is_blocked());

        gate.block();
        assert!(gate.is_blocked());
        assert!(!backend_ready_for_redraw(
            BackendKind::Cpu,
            true,
            false,
            (960, 540).into(),
            !gate.is_blocked()
        ));

        gate.reset();
        assert!(!gate.is_blocked());
        assert!(backend_ready_for_redraw(
            BackendKind::Cpu,
            true,
            false,
            (960, 540).into(),
            !gate.is_blocked()
        ));
    }

    #[test]
    fn duplicate_resize_is_skipped_only_for_confirmed_dimensions() {
        assert!(!resize_is_needed(Some((960, 540)), (960, 540)));
        assert!(resize_is_needed(Some((960, 540)), (1280, 720)));
        assert!(resize_is_needed(None, (960, 540)));
    }

    #[test]
    fn hud_title_reports_backend_tick_and_controls() {
        let snapshot = RenderSnapshot::minimal_demo();
        let title = hud_title("Astral Echo", BackendKind::Cpu, &snapshot);

        assert!(title.contains("CPU"));
        assert!(title.contains("tick=1"));
        assert!(title.contains("O observar"));
        assert!(title.contains("F aterrizar/atacar"));
    }

    #[test]
    fn hud_title_reports_shared_snapshot_context() {
        let mut snapshot = RenderSnapshot::minimal_demo();
        snapshot.overlay.hud.flight = render_core::HudFlightState::Atmospheric;
        snapshot.overlay.hud.autopilot = render_core::HudAutopilotState::InTransit;
        snapshot.overlay.hud.target = Some(42);
        snapshot.overlay.hud.active_enemies = 2;
        snapshot.overlay.hud.feedback = Some("OPERACION PARCIALMENTE COMPLETADA".into());
        let title = hud_title("Astral Echo", BackendKind::Gpu, &snapshot);

        assert!(title.contains("vuelo=atmosfera"));
        assert!(title.contains("objetivo=42"));
        assert!(title.contains("auto=en_transito"));
        assert!(title.contains("enemigos=2"));
        assert!(title.contains("feedback=OPERACION PARCIALMENTE COMPLETADA"));
    }

    #[test]
    fn hud_title_uses_current_feedback_and_does_not_keep_previous_text() {
        let mut snapshot = RenderSnapshot::minimal_demo();
        snapshot.overlay.hud.feedback = Some("OPERACION FALLIDA".into());
        let failed = hud_title("Astral Echo", BackendKind::Cpu, &snapshot);
        assert!(failed.contains("feedback=OPERACION FALLIDA"));

        snapshot.overlay.hud.feedback = Some("OPERACION COMPLETADA".into());
        let completed = hud_title("Astral Echo", BackendKind::Cpu, &snapshot);
        assert!(completed.contains("feedback=OPERACION COMPLETADA"));
        assert!(!completed.contains("OPERACION FALLIDA"));

        snapshot.overlay.hud.feedback = None;
        let empty = hud_title("Astral Echo", BackendKind::Cpu, &snapshot);
        assert!(empty.contains("feedback=sin_feedback"));
    }

    #[test]
    fn hud_title_reports_each_mission_state_without_changing_required_progress() {
        let cases = [
            (render_core::HudMissionState::Locked, "bloqueada", "0/5"),
            (render_core::HudMissionState::Available, "disponible", "0/5"),
            (render_core::HudMissionState::Active, "activa", "2/5"),
            (render_core::HudMissionState::Failed, "fallida", "2/5"),
            (render_core::HudMissionState::Completed, "completada", "5/5"),
        ];

        for (state, label, progress) in cases {
            let mut snapshot = RenderSnapshot::minimal_demo();
            snapshot.overlay.hud.mission_name = Some("El despertar".into());
            snapshot.overlay.hud.mission_progress = Some(match state {
                render_core::HudMissionState::Completed => (5, 5),
                render_core::HudMissionState::Active | render_core::HudMissionState::Failed => {
                    (2, 5)
                }
                render_core::HudMissionState::Locked | render_core::HudMissionState::Available => {
                    (0, 5)
                }
            });
            snapshot.overlay.hud.mission_state = Some(state);

            let title = hud_title("Astral Echo", BackendKind::Cpu, &snapshot);

            assert!(title.contains("mision=El despertar"));
            assert!(title.contains(&format!("{label} {progress}")));
        }
    }

    #[test]
    fn hud_title_prioritizes_game_over() {
        let mut snapshot = RenderSnapshot::minimal_demo();
        snapshot.overlay.game_over = true;

        assert_eq!(
            hud_title("Astral Echo", BackendKind::Gpu, &snapshot),
            "Astral Echo | GAME OVER"
        );
    }

    fn report() -> HotSwapReport {
        HotSwapReport {
            transitions: 0,
            last_transition_ms: None,
            gpu_error: None,
        }
    }

    fn backend_state() -> HotSwapState<()> {
        HotSwapState::new(
            BackendKind::Cpu,
            BackendSnapshot {
                value: (),
                width: 640,
                height: 360,
            },
        )
    }

    #[test]
    fn report_records_each_confirmed_transition_once() {
        let mut state = backend_state();
        let mut report = report();

        assert!(state.request_switch(BackendKind::Gpu));
        assert!(state.confirm_switch(BackendKind::Gpu).unwrap());
        report.record_transition(state.transitions(), Some(12.5));

        assert_eq!(report.transitions, 1);
        assert_eq!(report.last_transition_ms, Some(12.5));
    }

    #[test]
    fn rejected_duplicate_does_not_change_report() {
        let mut state = backend_state();
        let report = report();

        assert!(state.request_switch(BackendKind::Gpu));
        assert!(!state.request_switch(BackendKind::Gpu));

        assert_eq!(report.transitions, 0);
        assert_eq!(report.last_transition_ms, None);
    }

    #[test]
    fn cancelled_transition_does_not_change_report() {
        let mut state = backend_state();
        let report = report();

        assert!(state.request_switch(BackendKind::Gpu));
        state.cancel_switch("GPU unavailable");

        assert_eq!(report.transitions, 0);
        assert_eq!(report.last_transition_ms, None);
        assert_eq!(state.backend(), BackendKind::Cpu);
    }

    #[test]
    fn consecutive_cpu_gpu_transitions_are_counted_without_duplication() {
        let mut state = backend_state();
        let mut report = report();

        assert!(state.request_switch(BackendKind::Gpu));
        assert!(state.confirm_switch(BackendKind::Gpu).unwrap());
        report.record_transition(state.transitions(), Some(10.0));

        assert!(state.request_switch(BackendKind::Cpu));
        assert!(state.confirm_switch(BackendKind::Cpu).unwrap());
        report.record_transition(state.transitions(), Some(4.0));

        assert_eq!(report.transitions, 2);
        assert_eq!(report.last_transition_ms, Some(4.0));
        assert_eq!(state.backend(), BackendKind::Cpu);
    }
}

impl HostState {
    fn config_preset(&self) -> Preset {
        self.preset
    }
}

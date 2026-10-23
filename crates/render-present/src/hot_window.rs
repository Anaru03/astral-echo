use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use render_core::{InterpolationConfig, RenderSnapshot, SnapshotBuffer, SnapshotScene};
use render_cpu::{render_snapshot_scene, Framebuffer, RasterMode};
use render_gpu::{GpuOptions, SurfaceRenderer};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::{copy_nearest_rgba, BackendKind, BackendSnapshot, HotSwapState, Preset, WindowConfig};

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
    run_hot_swap_window_with_debug(config, source, start_gpu, GraphicsDebugConfig::default())
}

pub fn run_hot_swap_window_with_debug(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
    debug: GraphicsDebugConfig,
) -> Result<HotSwapReport, String> {
    let mut app = Host {
        config,
        source,
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
    backend: HotSwapState<Rc<SnapshotScene>>,
    buffer: SnapshotBuffer,
    started: Instant,
    gpu: Option<SurfaceRenderer>,
    occluded: bool,
    preset: Preset,
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
        let _ = surface.resize(
            NonZeroU32::new(size.0).unwrap(),
            NonZeroU32::new(size.1).unwrap(),
        );
        let mut buffer = SnapshotBuffer::new(InterpolationConfig::default());
        let _ = buffer.push(snapshot, 0.0);
        let snapshot = buffer
            .sample(0.0)
            .ok_or_else(|| "initial snapshot unavailable".to_string())
            .unwrap();
        let mut scene = SnapshotScene::default();
        scene
            .update(&snapshot)
            .map_err(|error| error.to_string())
            .unwrap();
        let snapshot = Rc::new(scene);
        let backend = HotSwapState::new(
            BackendKind::Cpu,
            BackendSnapshot {
                value: snapshot.clone(),
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
            buffer,
            started: Instant::now(),
        });
        if self.start_gpu {
            self.toggle();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Occluded(v) => state.occluded = v,
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut state.surface {
                    let _ = surface.resize(
                        NonZeroU32::new(size.width.max(1)).unwrap(),
                        NonZeroU32::new(size.height.max(1)).unwrap(),
                    );
                }
                if let Some(gpu) = &mut state.gpu {
                    let _ = gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyG => self.toggle(),
                        KeyCode::F9 | KeyCode::KeyM => self.capture_debug("marked"),
                        KeyCode::KeyS => self.config.raster_mode = RasterMode::Secuencial,
                        KeyCode::KeyP => self.config.raster_mode = RasterMode::Paralelo,
                        KeyCode::KeyA => self.config.raster_mode = RasterMode::Auto,
                        KeyCode::Digit1 => self.set_preset(Preset::Low),
                        KeyCode::Digit2 => self.set_preset(Preset::Medium),
                        KeyCode::Digit3 => self.set_preset(Preset::High),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

impl Host {
    fn set_preset(&mut self, preset: Preset) {
        self.config.preset = preset;
        if let Some(state) = &mut self.state {
            state.preset = preset;
            let (width, height) = preset.resolution();
            let _ = state.framebuffer.resize(width, height);
            let _ = state
                .window
                .request_inner_size(LogicalSize::new(width, height));
            if let Some(gpu) = &mut state.gpu {
                let _ = gpu.resize(width, height);
                if preset == Preset::Low {
                    let _ = gpu.set_postprocess(render_gpu::PostProcessConfig::default());
                } else {
                    let _ = gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High);
                }
            }
        }
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
        let gpu = if target == BackendKind::Gpu {
            let mut gpu = match SurfaceRenderer::new(
                state.window.clone(),
                &state.snapshot.owned_draws(),
                GpuOptions::default(),
            ) {
                Ok(gpu) => gpu,
                Err(error) => {
                    state.surface = released_surface;
                    self.report_gpu_error(error.to_string());
                    return;
                }
            };
            if state.config_preset() != Preset::Low {
                let _ = gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High);
            }
            Some(gpu)
        } else {
            state.surface = Self::new_cpu_surface(state);
            None
        };
        let result = state.backend.try_switch(target, |_| Ok(()));
        if result.is_ok() {
            state.gpu = gpu;
            state.window.request_redraw();
        } else if target == BackendKind::Gpu {
            state.surface = released_surface;
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
        let text = format!("reason={reason}\nbackend={:?}\nphysical={}x{}\ninternal={}x{}\ntick={}\nnodes={} visible={} discarded={} triangles={} draws={} buffer={}\n", state.backend.backend(), size.width, size.height, state.framebuffer.width(), state.framebuffer.height(), state.snapshot.snapshot().tick.0, state.snapshot.snapshot().nodes.len(), prepared.visible, prepared.discarded, prepared.triangles, prepared.draws().count(), state.buffer.len());
        if std::fs::write(&path, text).is_ok() {
            println!("graphics-debug capture={}", path.display());
        }
    }

    fn new_cpu_surface(state: &HostState) -> Option<Surface<Arc<Window>, Arc<Window>>> {
        let size = state.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let mut surface = Surface::new(&state._context, state.window.clone()).ok()?;
        surface
            .resize(NonZeroU32::new(size.width)?, NonZeroU32::new(size.height)?)
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
        if let Ok(snapshot) = (self.source)(size.width, size.height) {
            let timestamp = state.started.elapsed().as_secs_f32();
            let _ = state.buffer.push(snapshot, timestamp);
            if let Some(interpolated) = state.buffer.sample(timestamp) {
                let mut scene = SnapshotScene::default();
                if scene.update(&interpolated).is_err() {
                    return;
                }
                let scene = Rc::new(scene);
                state.snapshot = scene.clone();
                state.backend.update_snapshot(BackendSnapshot {
                    value: scene,
                    width: size.width,
                    height: size.height,
                });
            }
        }
        let prepared = state.snapshot.prepare(render_core::CullingConfig {
            width: size.width,
            height: size.height,
            ..Default::default()
        });
        println!(
            "hot-swap frame backend={:?} tick={} nodes={} considered={} visible={} discarded={} triangles={}",
            state.backend.backend(),
            state.snapshot.snapshot().tick.0,
            state.snapshot.snapshot().nodes.len(),
            prepared.considered,
            prepared.visible,
            prepared.discarded,
            prepared.triangles
        );
        if state.backend.backend() == BackendKind::Gpu {
            if let Some(gpu) = &mut state.gpu {
                let _ = gpu.present_snapshot(&state.snapshot);
            }
            return;
        }
        let _ = render_snapshot_scene(
            &mut state.framebuffer,
            &state.snapshot,
            self.config.raster_mode,
        );
        if let Some(surface) = &mut state.surface {
            let Ok(mut buffer) = surface.buffer_mut() else {
                return;
            };
            let _ = copy_nearest_rgba(
                state.framebuffer.color_bytes(),
                (state.framebuffer.width(), state.framebuffer.height()),
                (size.width, size.height),
                &mut buffer,
            );
            let _ = buffer.present();
        }
    }
}

impl HostState {
    fn config_preset(&self) -> Preset {
        self.preset
    }
}

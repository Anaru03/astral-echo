use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use render_core::SnapshotScene;
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

pub type SnapshotSource = Box<dyn FnMut(u32, u32) -> Result<SnapshotScene, String>>;

pub fn run_hot_swap_window(
    config: WindowConfig,
    source: SnapshotSource,
    start_gpu: bool,
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
}

struct HostState {
    window: Arc<Window>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    _context: Context<Arc<Window>>,
    framebuffer: Framebuffer,
    snapshot: Rc<SnapshotScene>,
    backend: HotSwapState<Rc<SnapshotScene>>,
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
        let snapshot = Rc::new(snapshot);
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
            surface,
            _context: context,
            framebuffer,
            snapshot,
            backend,
            gpu: None,
            occluded: false,
            preset: self.config.preset,
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
                let _ = state.surface.resize(
                    NonZeroU32::new(size.width.max(1)).unwrap(),
                    NonZeroU32::new(size.height.max(1)).unwrap(),
                );
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
                        KeyCode::KeyS => self.config.raster_mode = RasterMode::Secuencial,
                        KeyCode::KeyP => self.config.raster_mode = RasterMode::Paralelo,
                        KeyCode::KeyA => self.config.raster_mode = RasterMode::Auto,
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
    fn toggle(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let target = if state.backend.backend() == BackendKind::Cpu {
            BackendKind::Gpu
        } else {
            BackendKind::Cpu
        };
        let gpu = if target == BackendKind::Gpu {
            let mut gpu = match SurfaceRenderer::new(
                state.window.clone(),
                &state.snapshot.owned_draws(),
                GpuOptions::default(),
            ) {
                Ok(gpu) => gpu,
                Err(error) => {
                    self.report_gpu_error(error.to_string());
                    return;
                }
            };
            if state.config_preset() != Preset::Low {
                let _ = gpu.set_postprocess_preset(render_gpu::PostProcessPreset::High);
            }
            Some(gpu)
        } else {
            None
        };
        let result = state.backend.try_switch(target, |_| Ok(()));
        if result.is_ok() {
            state.gpu = gpu;
        }
        if let Err(error) = result {
            self.report_gpu_error(error);
        } else {
            self.report.transitions = state.backend.transitions();
            self.report.last_transition_ms = state
                .backend
                .last_transition()
                .map(|d| d.as_secs_f64() * 1000.0);
        }
    }
    fn report_gpu_error(&mut self, error: String) {
        self.report.gpu_error = Some(error);
    }
    fn draw(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.occluded {
            return;
        }
        let size = state.window.inner_size();
        if let Ok(scene) = (self.source)(size.width, size.height) {
            let scene = Rc::new(scene);
            state.snapshot = scene.clone();
            state.backend.update_snapshot(BackendSnapshot {
                value: scene,
                width: size.width,
                height: size.height,
            });
        }
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
        if let Ok(mut buffer) = state.surface.buffer_mut() {
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

use std::sync::Arc;
use std::time::{Duration, Instant};

use render_core::{Camera, Mat4, RasterMesh, RenderSnapshot, SnapshotScene, Transform};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::renderer::DepthTarget;
use crate::{GpuContext, GpuError, GpuOptions, GpuRenderer};

pub struct SurfaceRenderer {
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    gpu: Arc<GpuContext>,
    renderer: GpuRenderer,
    config: wgpu::SurfaceConfiguration,
    depth: Option<DepthTarget>,
}

impl SurfaceRenderer {
    pub fn new(
        window: Arc<Window>,
        scene: &[(RasterMesh, Transform)],
        options: GpuOptions,
    ) -> Result<Self, GpuError> {
        let instance = GpuContext::create_instance(options)?;
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| GpuError::Backend(e.to_string()))?;
        let gpu = Arc::new(GpuContext::from_instance(
            instance,
            Some(&surface),
            options,
        )?);
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| GpuError::Unavailable("adapter cannot present to window".into()))?;
        let formats = surface.get_capabilities(&gpu.adapter).formats;
        config.format = formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .or_else(|| {
                formats.iter().copied().find(|f| {
                    matches!(
                        f,
                        wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb
                    )
                })
            })
            .ok_or_else(|| GpuError::Unavailable("no supported surface color format".into()))?;
        config.present_mode = wgpu::PresentMode::Fifo;
        let renderer = GpuRenderer::new(gpu.clone(), config.format, scene)?;
        println!(
            "GPU window: {:?}, format={:?}",
            gpu.adapter_info(),
            config.format
        );
        let mut result = Self {
            surface,
            window,
            gpu,
            renderer,
            config,
            depth: None,
        };
        result.resize(size.width, size.height)?;
        Ok(result)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        if width == 0 || height == 0 {
            self.depth = None;
            return Ok(());
        }
        let depth = DepthTarget::new(&self.gpu, width, height)?;
        self.config.width = width;
        self.config.height = height;
        self.gpu.wait()?;
        self.surface.configure(&self.gpu.device, &self.config);
        self.depth = Some(depth);
        self.gpu.check()
    }

    pub fn present(&mut self, camera: &Camera) -> Result<bool, GpuError> {
        self.present_matrix(camera.view_projection_matrix())
    }

    pub fn present_snapshot(&mut self, scene: &SnapshotScene) -> Result<bool, GpuError> {
        let prepared = scene.prepare(render_core::CullingConfig {
            width: self.config.width,
            height: self.config.height,
            ..Default::default()
        });
        self.renderer.sync_prepared(&prepared)?;
        self.present_matrix(scene.view_projection())
    }

    fn present_matrix(&mut self, vp: Mat4) -> Result<bool, GpuError> {
        self.gpu.check()?;
        let Some(depth) = &self.depth else {
            return Ok(false);
        };
        let (texture, reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => (t, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => (t, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.gpu.create_surface(self.window.clone())?;
                if !self.gpu.adapter.is_surface_supported(&self.surface) {
                    return Err(GpuError::Unavailable(
                        "adapter no longer supports surface".into(),
                    ));
                }
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GpuError::Backend("surface validation failed".into()))
            }
        };
        let view = texture.texture.create_view(&Default::default());
        self.renderer
            .render_to_view_matrix(&view, &depth.view, vp)?;
        self.window.pre_present_notify();
        texture.present();
        if reconfigure {
            self.surface.configure(&self.gpu.device, &self.config);
        }
        self.gpu.check()?;
        Ok(true)
    }
}

#[derive(Default, Debug)]
pub struct WindowReport {
    pub frames: u64,
    pub elapsed: Duration,
}

pub fn run_window(
    scene: Vec<(RasterMesh, Transform)>,
    camera: Camera,
    max_frames: Option<u64>,
) -> Result<WindowReport, GpuError> {
    run_app(scene, camera, max_frames, None)
}

type SnapshotSource = Box<dyn FnMut(u32, u32) -> Result<RenderSnapshot, GpuError>>;

pub fn run_snapshot_window(
    source: impl FnMut(u32, u32) -> Result<RenderSnapshot, GpuError> + 'static,
    max_frames: Option<u64>,
) -> Result<WindowReport, GpuError> {
    let mut source: SnapshotSource = Box::new(source);
    let mut scene = SnapshotScene::default();
    scene
        .update(&source(960, 540)?)
        .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
    let draws = scene.owned_draws();
    run_app(draws, Camera::demo(), max_frames, Some((scene, source)))
}

fn run_app(
    scene: Vec<(RasterMesh, Transform)>,
    camera: Camera,
    max_frames: Option<u64>,
    snapshot: Option<(SnapshotScene, SnapshotSource)>,
) -> Result<WindowReport, GpuError> {
    if max_frames == Some(0) {
        return Err(GpuError::InvalidInput("zero frame limit".into()));
    }
    let event_loop = EventLoop::new().map_err(|e| GpuError::Backend(e.to_string()))?;
    let mut app = App {
        scene,
        camera,
        surface: None,
        error: None,
        report: WindowReport::default(),
        start: Instant::now(),
        next: Instant::now(),
        max_frames,
        occluded: false,
        snapshot,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| GpuError::Backend(e.to_string()))?;
    app.report.elapsed = app.start.elapsed();
    match app.error {
        Some(error) => Err(error),
        None => Ok(app.report),
    }
}

struct App {
    scene: Vec<(RasterMesh, Transform)>,
    camera: Camera,
    surface: Option<SurfaceRenderer>,
    error: Option<GpuError>,
    report: WindowReport,
    start: Instant,
    next: Instant,
    max_frames: Option<u64>,
    occluded: bool,
    snapshot: Option<(SnapshotScene, SnapshotSource)>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: GpuError) {
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let result = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Astral Echo F5A GPU")
                    .with_inner_size(winit::dpi::LogicalSize::new(960.0, 540.0)),
            )
            .map_err(|e| GpuError::Backend(e.to_string()))
            .and_then(|window| {
                SurfaceRenderer::new(Arc::new(window), &self.scene, GpuOptions::default())
            });
        match result {
            Ok(surface) => {
                self.surface = Some(surface);
                self.next = Instant::now();
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.surface = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        let result = match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                Ok(())
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                let size = surface.window.inner_size();
                surface.resize(size.width, size.height)
            }
            WindowEvent::Occluded(value) => {
                self.occluded = value;
                Ok(())
            }
            WindowEvent::RedrawRequested if !self.occluded => {
                let size = surface.window.inner_size();
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if self
                    .camera
                    .set_aspect(size.width as f32 / size.height as f32)
                    .is_err()
                {
                    return;
                }
                let frame = if let Some((scene, source)) = &mut self.snapshot {
                    source(size.width, size.height).and_then(|snapshot| {
                        scene
                            .update(&snapshot)
                            .map_err(|e| GpuError::InvalidInput(e.to_string()))?;
                        surface.present_snapshot(scene)
                    })
                } else {
                    surface.present(&self.camera)
                };
                match frame {
                    Ok(presented) => {
                        self.report.frames += u64::from(presented);
                        if self.max_frames.is_some_and(|n| self.report.frames >= n) {
                            event_loop.exit();
                        }
                        self.next = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(surface) = &self.surface {
            let size = surface.window.inner_size();
            if self.occluded || size.width == 0 || size.height == 0 {
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                if Instant::now() >= self.next {
                    surface.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
            }
        }
    }
}

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use render_core::{
    Camera, GpuAssetEvent, Mat4, PreparedSceneStats, RasterMesh, RenderSnapshot, SnapshotScene,
    Transform,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::postprocess::PostProcessPass;
use crate::renderer::DepthTarget;
use crate::{
    block_on, game_over_overlay_scene, hud_overlay_scene, GpuContext, GpuError, GpuImage,
    GpuOptions, GpuRenderer, PostProcessConfig, PostProcessPreset, ReadbackLayout,
};

pub struct SurfaceRenderer {
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    gpu: Arc<GpuContext>,
    renderer: GpuRenderer,
    config: wgpu::SurfaceConfiguration,
    depth: Option<DepthTarget>,
    scene: Vec<(RasterMesh, Transform)>,
    hdr_renderer: Option<GpuRenderer>,
    postprocess: Option<PostProcessPass>,
    hdr_texture: Option<wgpu::Texture>,
    hdr_view: Option<wgpu::TextureView>,
    capture_request: Option<CaptureRequest>,
}

enum CaptureRequest {
    Composite(PathBuf),
    Dual {
        scene_only: PathBuf,
        composite: PathBuf,
    },
}

pub struct WindowSurface {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
}

impl SurfaceRenderer {
    pub fn new(
        window: Arc<Window>,
        scene: &[(RasterMesh, Transform)],
        options: GpuOptions,
    ) -> Result<Self, GpuError> {
        block_on(Self::new_async_ref(window, scene, options))
    }

    pub fn new_async(
        window: Arc<Window>,
        scene: Vec<(RasterMesh, Transform)>,
        options: GpuOptions,
    ) -> Pin<Box<dyn Future<Output = Result<Self, GpuError>>>> {
        Box::pin(async move {
            let window_surface = Self::create_window_surface(window.clone(), options)?;
            Self::new_async_with_surface(window, window_surface, &scene, options).await
        })
    }

    pub fn create_window_surface(
        window: Arc<Window>,
        options: GpuOptions,
    ) -> Result<WindowSurface, GpuError> {
        let instance = GpuContext::create_instance(options)?;
        let surface = instance
            .create_surface(window)
            .map_err(|e| GpuError::Backend(e.to_string()))?;
        Ok(WindowSurface { instance, surface })
    }

    async fn new_async_ref(
        window: Arc<Window>,
        scene: &[(RasterMesh, Transform)],
        options: GpuOptions,
    ) -> Result<Self, GpuError> {
        let window_surface = Self::create_window_surface(window.clone(), options)?;
        Self::new_async_with_surface(window, window_surface, scene, options).await
    }

    pub async fn new_async_with_surface(
        window: Arc<Window>,
        window_surface: WindowSurface,
        scene: &[(RasterMesh, Transform)],
        options: GpuOptions,
    ) -> Result<Self, GpuError> {
        let started = Instant::now();
        println!("hot-swap gpu-surface-start");
        let WindowSurface { instance, surface } = window_surface;
        let gpu =
            Arc::new(GpuContext::from_instance_async(instance, Some(&surface), options).await?);
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| GpuError::Unavailable("adapter cannot present to window".into()))?;
        println!(
            "hot-swap gpu-surface-configured elapsed_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let formats = capabilities.formats;
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
        if capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let renderer = GpuRenderer::new_async(gpu.clone(), config.format, scene).await?;
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
            scene: scene.to_vec(),
            hdr_renderer: None,
            postprocess: None,
            hdr_texture: None,
            hdr_view: None,
            capture_request: None,
        };
        result.resize_without_wait(size.width, size.height)?;
        println!(
            "hot-swap gpu-surface-ready elapsed_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(result)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        self.resize_with_wait(width, height, true)
    }

    pub fn resize_without_wait(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        self.resize_with_wait(width, height, false)
    }

    fn resize_with_wait(&mut self, width: u32, height: u32, wait: bool) -> Result<(), GpuError> {
        if width == 0 || height == 0 {
            self.depth = None;
            return Ok(());
        }
        let depth = DepthTarget::new(&self.gpu, width, height)?;
        if self.postprocess.is_some() {
            let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("persistent window HDR"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.hdr_texture = Some(texture);
            self.hdr_view = Some(view);
            if let Some(pass) = &mut self.postprocess {
                pass.resize(&self.gpu, width, height);
            }
        }
        self.config.width = width;
        self.config.height = height;
        if wait {
            self.gpu.wait()?;
        }
        self.surface.configure(&self.gpu.device, &self.config);
        self.depth = Some(depth);
        self.gpu.check()
    }

    pub fn set_postprocess_preset(&mut self, preset: PostProcessPreset) -> Result<(), GpuError> {
        self.set_postprocess(PostProcessConfig::preset(preset))
    }

    pub async fn set_postprocess_preset_async(
        &mut self,
        preset: PostProcessPreset,
    ) -> Result<(), GpuError> {
        self.set_postprocess_async(PostProcessConfig::preset(preset))
            .await
    }

    async fn set_postprocess_async(&mut self, config: PostProcessConfig) -> Result<(), GpuError> {
        config.validate()?;
        if config.enabled {
            if self.hdr_renderer.is_none() {
                self.hdr_renderer = Some(
                    GpuRenderer::new_async(
                        self.gpu.clone(),
                        wgpu::TextureFormat::Rgba16Float,
                        &self.scene,
                    )
                    .await?,
                );
            }
            if self.postprocess.is_none() {
                self.postprocess = Some(PostProcessPass::new(
                    &self.gpu,
                    self.config.format,
                    wgpu::TextureFormat::Rgba16Float,
                    self.config.width,
                    self.config.height,
                )?);
            }
            self.postprocess
                .as_mut()
                .expect("postprocess")
                .set_config(config)?;
            if self.hdr_texture.is_none() && self.config.width > 0 && self.config.height > 0 {
                self.resize_without_wait(self.config.width, self.config.height)?;
            }
        } else if let Some(pass) = &mut self.postprocess {
            pass.set_config(config)?;
        }
        Ok(())
    }

    pub fn set_postprocess(&mut self, config: PostProcessConfig) -> Result<(), GpuError> {
        config.validate()?;
        if config.enabled {
            if self.hdr_renderer.is_none() {
                self.hdr_renderer = Some(GpuRenderer::new(
                    self.gpu.clone(),
                    wgpu::TextureFormat::Rgba16Float,
                    &self.scene,
                )?);
            }
            if self.postprocess.is_none() {
                self.postprocess = Some(PostProcessPass::new(
                    &self.gpu,
                    self.config.format,
                    wgpu::TextureFormat::Rgba16Float,
                    self.config.width,
                    self.config.height,
                )?);
            }
            self.postprocess
                .as_mut()
                .expect("postprocess")
                .set_config(config)?;
            if self.hdr_texture.is_none() && self.config.width > 0 && self.config.height > 0 {
                self.resize(self.config.width, self.config.height)?;
            }
        } else if let Some(pass) = &mut self.postprocess {
            pass.set_config(config)?;
        }
        Ok(())
    }

    pub fn postprocess_config(&self) -> Option<PostProcessConfig> {
        self.postprocess.as_ref().map(|pass| pass.config())
    }

    pub fn diagnostic_state(&self) -> (wgpu::TextureFormat, bool, bool, u32, u32) {
        (
            self.config.format,
            self.hdr_view.is_some(),
            self.postprocess
                .as_ref()
                .is_some_and(|pass| pass.is_enabled()),
            self.config.width,
            self.config.height,
        )
    }

    pub fn ready_for_present(&self) -> bool {
        self.config.width > 0 && self.config.height > 0 && self.depth.is_some()
    }

    pub fn poll_gpu_asset_events(&mut self) -> Vec<GpuAssetEvent> {
        let mut events = self.renderer.poll_gpu_asset_events();
        if let Some(renderer) = &mut self.hdr_renderer {
            events.extend(renderer.poll_gpu_asset_events());
        }
        events
    }

    pub fn ensure_shared_mesh(
        &mut self,
        mesh: render_core::MeshHandle,
        lod: render_core::LodLevel,
        source: Arc<render_core::RasterMesh>,
    ) -> Result<(), GpuError> {
        self.renderer.ensure_shared_mesh(mesh, lod, source)
    }

    pub fn request_capture(&mut self, path: PathBuf) -> Result<(), GpuError> {
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(GpuError::Unavailable(
                "GPU surface does not support COPY_SRC readback".into(),
            ));
        }
        if self.capture_request.is_some() {
            return Err(GpuError::InvalidInput(
                "a GPU capture is already pending".into(),
            ));
        }
        self.capture_request = Some(CaptureRequest::Composite(path));
        Ok(())
    }

    /// Requests one readback pair from the same composed frame: the scene
    /// after postprocessing and the final image after the HUD.
    pub fn request_dual_capture(
        &mut self,
        scene_only: PathBuf,
        composite: PathBuf,
    ) -> Result<(), GpuError> {
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(GpuError::Unavailable(
                "GPU surface does not support COPY_SRC readback".into(),
            ));
        }
        if self.capture_request.is_some() {
            return Err(GpuError::InvalidInput(
                "a GPU capture is already pending".into(),
            ));
        }
        self.capture_request = Some(CaptureRequest::Dual {
            scene_only,
            composite,
        });
        Ok(())
    }

    pub fn present(&mut self, camera: &Camera) -> Result<bool, GpuError> {
        self.present_matrix(camera.view_projection_matrix(), Default::default())
    }

    pub fn present_snapshot(&mut self, scene: &SnapshotScene) -> Result<bool, GpuError> {
        self.present_snapshot_with_stats(scene)
            .map(|(presented, _)| presented)
    }

    pub fn present_snapshot_with_stats(
        &mut self,
        scene: &SnapshotScene,
    ) -> Result<(bool, PreparedSceneStats), GpuError> {
        if self
            .postprocess
            .as_ref()
            .is_some_and(|pass| pass.is_enabled())
            && self.hdr_view.is_none()
            && self.config.width > 0
            && self.config.height > 0
        {
            self.resize(self.config.width, self.config.height)?;
        }
        let prepared = scene.prepare(render_core::CullingConfig::for_viewport(
            self.config.width,
            self.config.height,
        ));
        let prepared_stats = prepared.stats();
        self.renderer.sync_prepared(&prepared)?;
        if let Some(renderer) = &mut self.hdr_renderer {
            renderer.sync_prepared(&prepared)?;
        }
        let overlay = scene.overlay();
        Ok((
            self.present_matrix(scene.view_projection(), overlay)?,
            prepared_stats,
        ))
    }

    fn present_matrix(
        &mut self,
        vp: Mat4,
        overlay: render_core::RenderOverlay,
    ) -> Result<bool, GpuError> {
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
        let capture = self.capture_request.take();
        let (scene_capture, composite_capture) = match capture {
            Some(CaptureRequest::Composite(path)) => (None, Some(path)),
            Some(CaptureRequest::Dual {
                scene_only,
                composite,
            }) => (Some(scene_only), Some(composite)),
            None => (None, None),
        };
        if overlay.game_over {
            let overlay_scene = game_over_overlay_scene(self.config.width, self.config.height);
            let mut overlay_renderer =
                GpuRenderer::new(self.gpu.clone(), self.config.format, &overlay_scene)?;
            overlay_renderer.render_to_view_matrix_with_clear(
                &view,
                &depth.view,
                Mat4::IDENTITY,
                [0, 0, 0, 255],
            )?;
            let hud_scene = hud_overlay_scene(&overlay.hud, self.config.width, self.config.height);
            if !hud_scene.is_empty() {
                let mut hud_renderer =
                    GpuRenderer::new(self.gpu.clone(), self.config.format, &hud_scene)?;
                let hud_depth = DepthTarget::new(&self.gpu, self.config.width, self.config.height)?;
                hud_renderer.render_to_view_matrix_over(&view, &hud_depth.view, Mat4::IDENTITY)?;
            }
            if let Some(path) = scene_capture {
                self.capture_surface_texture(&texture.texture, &path)?;
            }
            if let Some(path) = composite_capture {
                self.capture_surface_texture(&texture.texture, &path)?;
            }
            self.window.pre_present_notify();
            texture.present();
            if reconfigure {
                self.surface.configure(&self.gpu.device, &self.config);
            }
            return Ok(true);
        }
        if let (Some(hdr), Some(pass), Some(hdr_view)) = (
            &mut self.hdr_renderer,
            self.postprocess.as_mut().filter(|p| p.is_enabled()),
            self.hdr_view.as_ref(),
        ) {
            pass.set_source_view(&self.gpu, hdr_view);
            hdr.render_to_view_matrix(hdr_view, &depth.view, vp)?;
            let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
            pass.encode(&self.gpu, &mut encoder, &view);
            self.gpu.queue.submit([encoder.finish()]);
        } else {
            self.renderer.render_to_view_matrix_with_clear(
                &view,
                &depth.view,
                vp,
                overlay_clear_color(&overlay),
            )?;
        }
        if let Some(path) = scene_capture {
            self.capture_surface_texture(&texture.texture, &path)?;
        }
        let hud_scene = hud_overlay_scene(&overlay.hud, self.config.width, self.config.height);
        if !hud_scene.is_empty() {
            let mut hud_renderer =
                GpuRenderer::new(self.gpu.clone(), self.config.format, &hud_scene)?;
            let hud_depth = DepthTarget::new(&self.gpu, self.config.width, self.config.height)?;
            hud_renderer.render_to_view_matrix_over(&view, &hud_depth.view, Mat4::IDENTITY)?;
        }
        if let Some(path) = composite_capture {
            self.capture_surface_texture(&texture.texture, &path)?;
        }
        self.window.pre_present_notify();
        texture.present();
        if reconfigure {
            self.surface.configure(&self.gpu.device, &self.config);
        }
        self.gpu.check()?;
        Ok(true)
    }

    fn capture_surface_texture(
        &self,
        texture: &wgpu::Texture,
        path: &PathBuf,
    ) -> Result<(), GpuError> {
        if path.exists() {
            return Err(GpuError::Readback(format!(
                "capture output already exists: {}",
                path.display()
            )));
        }
        let layout = ReadbackLayout::new(self.config.width, self.config.height)?;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("one-shot surface readback"),
            size: layout.buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(layout.bytes_per_row),
                    rows_per_image: Some(self.config.height),
                },
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.wait()?;
        let (sender, receiver) = mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.gpu.wait()?;
        receiver
            .recv_timeout(Duration::from_secs(20))
            .map_err(|error| GpuError::Readback(error.to_string()))?
            .map_err(|error| GpuError::Readback(error.to_string()))?;
        let mapped = buffer.slice(..).get_mapped_range();
        let mut rgba = layout.unpack(&mapped)?;
        drop(mapped);
        buffer.unmap();
        if matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        GpuImage {
            width: self.config.width,
            height: self.config.height,
            rgba,
            depth: Vec::new(),
        }
        .write_png(path)
        .map_err(|error| GpuError::Readback(error.to_string()))
    }
}

fn overlay_clear_color(overlay: &render_core::RenderOverlay) -> [u8; 4] {
    let scale = 1.0 - overlay.fade_to_black.clamp(0.0, 1.0);
    [
        (8.0 * scale) as u8,
        (10.0 * scale) as u8,
        (18.0 * scale) as u8,
        255,
    ]
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

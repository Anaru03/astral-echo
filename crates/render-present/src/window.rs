use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use render_cpu::{Framebuffer, RasterError, RasterMode};
use softbuffer::{Context, SoftBufferError, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::error::{EventLoopError, OsError};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::{
    copy_nearest_rgba, raster_mode_name, FrameMetrics, FramePacer, PixelError, Preset, ResizeState,
    WindowConfig,
};

#[derive(Debug)]
pub enum PresentError {
    EventLoop(EventLoopError),
    Window(OsError),
    Surface(SoftBufferError),
    Raster(RasterError),
    Pixels(PixelError),
}

impl fmt::Display for PresentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(formatter, "event loop: {error}"),
            Self::Window(error) => write!(formatter, "window: {error}"),
            Self::Surface(error) => write!(formatter, "presentation surface: {error}"),
            Self::Raster(error) => write!(formatter, "CPU render: {error}"),
            Self::Pixels(error) => write!(formatter, "presentation pixels: {error}"),
        }
    }
}

impl Error for PresentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::EventLoop(error) => error,
            Self::Window(error) => error,
            Self::Surface(error) => error,
            Self::Raster(error) => error,
            Self::Pixels(error) => error,
        })
    }
}

pub struct RunReport {
    pub framebuffer: Framebuffer,
    pub metrics: FrameMetrics,
    pub elapsed: Duration,
}

struct WindowState {
    surface: Surface<Arc<Window>, Arc<Window>>,
    _context: Context<Arc<Window>>,
    window: Arc<Window>,
    size: ResizeState,
    occluded: bool,
}

impl WindowState {
    fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Result<Self, PresentError> {
        let size = config.preset.resolution();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&config.title)
                        .with_inner_size(LogicalSize::new(size.0, size.1)),
                )
                .map_err(PresentError::Window)?,
        );
        let context = Context::new(window.clone()).map_err(PresentError::Surface)?;
        let mut surface = Surface::new(&context, window.clone()).map_err(PresentError::Surface)?;
        let physical = window.inner_size();
        let size = ResizeState::new(physical.width, physical.height, window.scale_factor())
            .map_err(PresentError::Pixels)?;
        if let Some((width, height)) = size.surface_size() {
            surface
                .resize(width, height)
                .map_err(PresentError::Surface)?;
        }
        Ok(Self {
            surface,
            _context: context,
            window,
            size,
            occluded: false,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), PresentError> {
        if self
            .size
            .resize(size.width, size.height)
            .map_err(PresentError::Pixels)?
        {
            if let Some((width, height)) = self.size.surface_size() {
                self.surface
                    .resize(width, height)
                    .map_err(PresentError::Surface)?;
            }
        }
        Ok(())
    }

    fn can_draw(&self) -> bool {
        !self.occluded && self.size.surface_size().is_some()
    }
}

struct Application<F> {
    config: WindowConfig,
    framebuffer: Framebuffer,
    render: F,
    state: Option<WindowState>,
    pacer: FramePacer,
    metrics: FrameMetrics,
    failure: Option<PresentError>,
    redraw_pending: bool,
    last_title: Instant,
}

impl<F> Application<F>
where
    F: FnMut(&mut Framebuffer, RasterMode) -> Result<(), RasterError>,
{
    fn close(&mut self, event_loop: &ActiveEventLoop) {
        self.state = None;
        event_loop.exit();
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: PresentError) {
        self.failure = Some(error);
        self.close(event_loop);
    }

    fn title(&self) {
        if let Some(state) = &self.state {
            state.window.set_title(&format!(
                "{} | {} {}x{} | {} | render {:.2} ms | present {:.2} ms | limit {} FPS",
                self.config.title,
                self.config.preset,
                self.framebuffer.width(),
                self.framebuffer.height(),
                raster_mode_name(self.config.raster_mode),
                self.metrics.last_render.as_secs_f64() * 1000.0,
                self.metrics.last_presentation.as_secs_f64() * 1000.0,
                self.config.preset.fps(),
            ));
        }
    }

    fn key(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) -> Result<(), PresentError> {
        let preset = match code {
            KeyCode::Digit1 => Some(Preset::Low),
            KeyCode::Digit2 => Some(Preset::Medium),
            KeyCode::Digit3 => Some(Preset::High),
            _ => None,
        };
        if let Some(preset) = preset {
            let size = preset.resolution();
            self.framebuffer
                .resize(size.0, size.1)
                .map_err(PresentError::Raster)?;
            self.config.preset = preset;
            self.pacer = FramePacer::new(preset.fps(), Instant::now());
            if let Some(state) = &mut self.state {
                if let Some(size) = state
                    .window
                    .request_inner_size(LogicalSize::new(size.0, size.1))
                {
                    state.resize(size)?;
                }
            }
        }
        match code {
            KeyCode::Escape => self.close(event_loop),
            KeyCode::KeyS => self.config.raster_mode = RasterMode::Secuencial,
            KeyCode::KeyP => self.config.raster_mode = RasterMode::Paralelo,
            KeyCode::KeyA => self.config.raster_mode = RasterMode::Auto,
            _ => {}
        }
        self.title();
        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PresentError> {
        self.redraw_pending = false;
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        state.resize(state.window.inner_size())?;
        let now = Instant::now();
        if !state.can_draw() || !self.pacer.is_due(now) {
            return Ok(());
        }
        self.pacer.frame_started(now);
        let render_start = Instant::now();
        (self.render)(&mut self.framebuffer, self.config.raster_mode)
            .map_err(PresentError::Raster)?;
        let render_time = render_start.elapsed();
        let present_start = Instant::now();
        let mut buffer = state.surface.buffer_mut().map_err(PresentError::Surface)?;
        copy_nearest_rgba(
            self.framebuffer.color_bytes(),
            (self.framebuffer.width(), self.framebuffer.height()),
            state.size.physical_size(),
            &mut buffer,
        )
        .map_err(PresentError::Pixels)?;
        buffer.present().map_err(PresentError::Surface)?;
        self.metrics.record(render_time, present_start.elapsed());
        if self.metrics.frames == 1 || self.last_title.elapsed() >= Duration::from_secs(1) {
            self.title();
            self.last_title = Instant::now();
        }
        if self
            .config
            .max_frames
            .is_some_and(|limit| self.metrics.frames >= limit.get())
        {
            self.close(event_loop);
        }
        Ok(())
    }
}

impl<F> ApplicationHandler for Application<F>
where
    F: FnMut(&mut Framebuffer, RasterMode) -> Result<(), RasterError>,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match WindowState::new(event_loop, &self.config) {
                Ok(state) => {
                    self.state = Some(state);
                    self.redraw_pending = false;
                    self.pacer = FramePacer::new(self.config.preset.fps(), Instant::now());
                    self.title();
                }
                Err(error) => self.fail(event_loop, error),
            }
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.state = None;
        self.redraw_pending = false;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };
        if state.window.id() != id {
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.close(event_loop);
                Ok(())
            }
            WindowEvent::Resized(size) => {
                self.redraw_pending = false;
                state.resize(size)
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => state
                .size
                .set_scale_factor(scale_factor)
                .map_err(PresentError::Pixels)
                .and_then(|()| state.resize(state.window.inner_size())),
            WindowEvent::Occluded(occluded) => {
                state.occluded = occluded;
                self.redraw_pending = false;
                Ok(())
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event_loop)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &self.state else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        if !state.can_draw() {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else if self.pacer.is_due(Instant::now()) {
            if !self.redraw_pending {
                state.window.request_redraw();
                self.redraw_pending = true;
            }
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.pacer.deadline()));
        }
    }
}

pub fn run_window<F>(config: WindowConfig, render: F) -> Result<RunReport, PresentError>
where
    F: FnMut(&mut Framebuffer, RasterMode) -> Result<(), RasterError>,
{
    let size = config.preset.resolution();
    let framebuffer = Framebuffer::new(size.0, size.1).map_err(PresentError::Raster)?;
    let started = Instant::now();
    let mut application = Application {
        pacer: FramePacer::new(config.preset.fps(), started),
        config,
        framebuffer,
        render,
        state: None,
        metrics: FrameMetrics::default(),
        failure: None,
        redraw_pending: false,
        last_title: started,
    };
    EventLoop::new()
        .map_err(PresentError::EventLoop)?
        .run_app(&mut application)
        .map_err(PresentError::EventLoop)?;
    if let Some(error) = application.failure {
        return Err(error);
    }
    Ok(RunReport {
        framebuffer: application.framebuffer,
        metrics: application.metrics,
        elapsed: started.elapsed(),
    })
}

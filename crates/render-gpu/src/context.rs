use std::error::Error;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuError {
    Unavailable(String),
    InvalidInput(String),
    Backend(String),
    Readback(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(s) => write!(f, "GPU unavailable: {s}"),
            Self::InvalidInput(s) => write!(f, "invalid GPU input: {s}"),
            Self::Backend(s) => write!(f, "GPU backend: {s}"),
            Self::Readback(s) => write!(f, "GPU readback: {s}"),
        }
    }
}

impl Error for GpuError {}

struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GpuOptions {
    pub backends: wgpu::Backends,
    pub force_fallback_adapter: bool,
}

impl Default for GpuOptions {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::PRIMARY,
            force_fallback_adapter: false,
        }
    }
}

pub struct GpuContext {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    failure: Arc<Mutex<Option<String>>>,
}

impl GpuContext {
    pub fn new(options: GpuOptions) -> Result<Self, GpuError> {
        let instance = Self::create_instance(options)?;
        Self::from_instance(instance, None, options)
    }

    pub(crate) fn create_instance(options: GpuOptions) -> Result<wgpu::Instance, GpuError> {
        if options.backends.is_empty() {
            return Err(GpuError::Unavailable("no backends selected".into()));
        }
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = options.backends;
        Ok(wgpu::Instance::new(descriptor))
    }

    pub(crate) fn from_instance(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        options: GpuOptions,
    ) -> Result<Self, GpuError> {
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: options.force_fallback_adapter,
            compatible_surface: surface,
        }))
        .map_err(|e| GpuError::Unavailable(e.to_string()))?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Astral Echo GPU"),
            ..Default::default()
        }))
        .map_err(|e| GpuError::Backend(e.to_string()))?;
        let failure = Arc::new(Mutex::new(None));
        let errors = failure.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            *errors.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.to_string());
        }));
        let lost = failure.clone();
        device.set_device_lost_callback(move |reason, message| {
            *lost.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(format!("device lost {reason:?}: {message}"));
        });
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            failure,
        })
    }

    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    pub fn create_surface(
        &self,
        window: Arc<winit::window::Window>,
    ) -> Result<wgpu::Surface<'static>, GpuError> {
        self.instance
            .create_surface(window)
            .map_err(|e| GpuError::Backend(e.to_string()))
    }

    pub fn check(&self) -> Result<(), GpuError> {
        match &*self.failure.lock().unwrap_or_else(|e| e.into_inner()) {
            Some(error) => Err(GpuError::Backend(error.clone())),
            None => Ok(()),
        }
    }

    pub fn wait(&self) -> Result<(), GpuError> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(20)),
            })
            .map_err(|e| GpuError::Backend(e.to_string()))?;
        self.check()
    }
}

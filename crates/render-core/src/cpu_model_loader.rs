use std::any::Any;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

use crate::{load_obj, ObjLoadError, RasterMesh};

type Loader = dyn Fn(&Path) -> Result<RasterMesh, ObjLoadError> + Send + Sync + 'static;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuModelLoadState {
    Unloaded,
    LoadingCpu,
    CpuReady,
    Failed(String),
}

struct Entry {
    generation: u64,
    state: CpuModelLoadState,
    mesh: Option<Arc<RasterMesh>>,
}

struct Completion {
    path: PathBuf,
    generation: u64,
    result: Result<Arc<RasterMesh>, String>,
}

/// CPU-only asynchronous OBJ loader.
///
/// The manager is owned and polled by one thread. Workers only execute the
/// loader and send immutable results back through the channel; they never
/// access `VisualAssets` or the manager's entries directly.
pub struct CpuModelLoadManager {
    entries: BTreeMap<PathBuf, Entry>,
    completion_tx: Sender<Completion>,
    completion_rx: Receiver<Completion>,
    loader: Arc<Loader>,
}

impl Default for CpuModelLoadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuModelLoadManager {
    pub fn new() -> Self {
        Self::with_loader(|path| {
            if path
                .file_name()
                .is_some_and(|name| name == "Seraphim_planet.obj")
            {
                crate::load_obj_decimated(path, 32)
            } else {
                load_obj(path)
            }
        })
    }

    pub fn with_loader<F>(loader: F) -> Self
    where
        F: Fn(&Path) -> Result<RasterMesh, ObjLoadError> + Send + Sync + 'static,
    {
        let (completion_tx, completion_rx) = mpsc::channel();
        Self {
            entries: BTreeMap::new(),
            completion_tx,
            completion_rx,
            loader: Arc::new(loader),
        }
    }

    /// Returns the current state without waiting for a worker.
    pub fn state(&mut self, path: impl Into<PathBuf>) -> CpuModelLoadState {
        self.poll();
        self.entries
            .get(&path.into())
            .map_or(CpuModelLoadState::Unloaded, |entry| entry.state.clone())
    }

    /// Requests a resource. Repeated requests reuse an existing load or mesh.
    pub fn request(&mut self, path: impl Into<PathBuf>) -> CpuModelLoadState {
        self.poll();
        let path = path.into();
        if let Some(entry) = self.entries.get(&path) {
            return entry.state.clone();
        }
        self.start(path.clone(), 0);
        CpuModelLoadState::LoadingCpu
    }

    /// Explicitly retries a failed resource. Normal requests never retry.
    pub fn retry(&mut self, path: impl Into<PathBuf>) -> bool {
        self.poll();
        let path = path.into();
        let generation = {
            let Some(entry) = self.entries.get_mut(&path) else {
                return false;
            };
            if !matches!(entry.state, CpuModelLoadState::Failed(_)) {
                return false;
            }
            entry.generation = entry.generation.wrapping_add(1);
            entry.state = CpuModelLoadState::LoadingCpu;
            entry.mesh = None;
            entry.generation
        };
        if let Err(error) = self.spawn(path.clone(), generation) {
            if let Some(entry) = self.entries.get_mut(&path) {
                entry.state = CpuModelLoadState::Failed(error);
            }
        }
        true
    }

    /// Returns a shared immutable mesh when the resource reached CpuReady.
    pub fn mesh(&mut self, path: impl Into<PathBuf>) -> Option<Arc<RasterMesh>> {
        self.poll();
        self.entries
            .get(&path.into())
            .and_then(|entry| entry.mesh.clone())
    }

    /// Applies completed worker results without blocking the caller.
    pub fn poll(&mut self) {
        loop {
            let completion = match self.completion_rx.try_recv() {
                Ok(completion) => completion,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            let Some(entry) = self.entries.get_mut(&completion.path) else {
                continue;
            };
            if entry.generation != completion.generation
                || !matches!(entry.state, CpuModelLoadState::LoadingCpu)
            {
                continue;
            }
            match completion.result {
                Ok(mesh) => {
                    entry.mesh = Some(mesh);
                    entry.state = CpuModelLoadState::CpuReady;
                }
                Err(error) => {
                    entry.mesh = None;
                    entry.state = CpuModelLoadState::Failed(error);
                }
            }
        }
    }

    fn start(&mut self, path: PathBuf, generation: u64) {
        self.entries.insert(
            path.clone(),
            Entry {
                generation,
                state: CpuModelLoadState::LoadingCpu,
                mesh: None,
            },
        );
        if let Err(error) = self.spawn(path.clone(), generation) {
            if let Some(entry) = self.entries.get_mut(&path) {
                entry.state = CpuModelLoadState::Failed(error);
            }
        }
    }

    fn spawn(&self, path: PathBuf, generation: u64) -> Result<(), String> {
        let loader = Arc::clone(&self.loader);
        let completion_tx = self.completion_tx.clone();
        thread::Builder::new()
            .name("astral-echo-cpu-model-loader".into())
            .spawn(move || {
                let result = match catch_unwind(AssertUnwindSafe(|| loader(&path))) {
                    Ok(result) => result.map(Arc::new).map_err(|error| error.to_string()),
                    Err(payload) => Err(format!("loader panic: {}", panic_message(payload))),
                };
                let _ = completion_tx.send(Completion {
                    path,
                    generation,
                    result,
                });
            })
            .map(|_| ())
            .map_err(|error| format!("CPU model loader worker could not be spawned: {error}"))
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "panic payload did not contain a string".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver, SyncSender};
    use std::sync::Mutex;
    use std::time::Duration;

    static COUNTER: std::sync::OnceLock<AtomicUsize> = std::sync::OnceLock::new();

    fn temp_obj(name: &str, source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "astral_echo_cpu_loader_{}_{}_{}.obj",
            name,
            std::process::id(),
            COUNTER
                .get_or_init(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, source).expect("write controlled OBJ");
        path
    }

    fn triangle() -> &'static str {
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n"
    }

    fn wait_until_ready(manager: &mut CpuModelLoadManager, path: &Path) {
        for _ in 0..10_000 {
            manager.poll();
            if matches!(manager.state(path), CpuModelLoadState::CpuReady) {
                return;
            }
            std::thread::yield_now();
        }
        panic!("worker did not complete");
    }

    #[test]
    fn request_transitions_from_unloaded_to_loading_and_cpu_ready() {
        let path = temp_obj("ready", triangle());
        let mut manager = CpuModelLoadManager::new();
        assert_eq!(manager.state(&path), CpuModelLoadState::Unloaded);
        assert_eq!(manager.request(&path), CpuModelLoadState::LoadingCpu);
        wait_until_ready(&mut manager, &path);
        assert_eq!(manager.state(&path), CpuModelLoadState::CpuReady);
        assert_eq!(manager.mesh(&path).expect("mesh").indices.len(), 3);
        fs::remove_file(path).expect("remove controlled OBJ");
    }

    #[test]
    fn invalid_obj_reaches_failed_without_automatic_retry() {
        let path = temp_obj("invalid", "v 0 0 0\nf 1 2 3\n");
        let mut manager = CpuModelLoadManager::new();
        manager.request(&path);
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                fs::remove_file(path).expect("remove controlled OBJ");
                return;
            }
            std::thread::yield_now();
        }
        panic!("invalid OBJ did not fail");
    }

    #[test]
    fn repeated_requests_share_one_worker_and_one_arc() {
        let path = temp_obj("dedupe", triangle());
        let executions = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx): (SyncSender<()>, Receiver<()>) = mpsc::sync_channel(0);
        let release_rx = Arc::new(Mutex::new(release_rx));
        let executions_for_loader = Arc::clone(&executions);
        let release_for_loader = Arc::clone(&release_rx);
        let mut manager = CpuModelLoadManager::with_loader(move |obj_path| {
            executions_for_loader.fetch_add(1, Ordering::SeqCst);
            started_tx.send(()).expect("worker started");
            release_for_loader
                .lock()
                .expect("release lock")
                .recv()
                .expect("release worker");
            load_obj(obj_path)
        });
        assert_eq!(manager.request(&path), CpuModelLoadState::LoadingCpu);
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("worker did not start");
        assert_eq!(manager.request(&path), CpuModelLoadState::LoadingCpu);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        release_tx.send(()).expect("release worker");
        wait_until_ready(&mut manager, &path);
        let first = manager.mesh(&path).expect("mesh");
        let second = manager.mesh(&path).expect("same mesh");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        fs::remove_file(path).expect("remove controlled OBJ");
    }

    #[test]
    fn one_failure_does_not_affect_another_resource() {
        let bad = temp_obj("bad_isolated", "v 0 0 0\nf 1 2 3\n");
        let good = temp_obj("good_isolated", triangle());
        let mut manager = CpuModelLoadManager::new();
        manager.request(&bad);
        manager.request(&good);
        for _ in 0..10_000 {
            manager.poll();
            if matches!(manager.state(&bad), CpuModelLoadState::Failed(_))
                && matches!(manager.state(&good), CpuModelLoadState::CpuReady)
            {
                assert!(manager.mesh(&good).is_some());
                fs::remove_file(bad).expect("remove bad OBJ");
                fs::remove_file(good).expect("remove good OBJ");
                return;
            }
            std::thread::yield_now();
        }
        panic!("independent loads did not complete");
    }

    #[test]
    fn failed_request_requires_explicit_retry() {
        let path = temp_obj("retry", "v 0 0 0\nf 1 2 3\n");
        let executions = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&executions);
        let mut manager = CpuModelLoadManager::with_loader(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Err(ObjLoadError::Empty)
        });
        manager.request(&path);
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(
            manager.request(&path),
            CpuModelLoadState::Failed("OBJ contains no renderable geometry".into())
        );
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert!(manager.retry(&path));
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(executions.load(Ordering::SeqCst), 2);
        fs::remove_file(path).expect("remove controlled OBJ");
    }

    #[test]
    fn stale_completion_cannot_overwrite_a_retried_generation() {
        let path = temp_obj("generation", triangle());
        let executions = Arc::new(AtomicUsize::new(0));
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let executions_for_loader = Arc::clone(&executions);
        let release_for_loader = Arc::clone(&release_rx);
        let mut manager = CpuModelLoadManager::with_loader(move |_| {
            let execution = executions_for_loader.fetch_add(1, Ordering::SeqCst);
            if execution == 1 {
                release_for_loader
                    .lock()
                    .expect("release lock")
                    .recv()
                    .expect("release retried worker");
            }
            Err(ObjLoadError::Empty)
        });
        manager.request(&path);
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        let old_generation = manager.entries.get(&path).expect("entry").generation;
        assert!(manager.retry(&path));
        manager
            .completion_tx
            .send(Completion {
                path: path.clone(),
                generation: old_generation,
                result: Ok(Arc::new(
                    crate::load_obj_reader(&mut std::io::Cursor::new(triangle().as_bytes()))
                        .expect("controlled mesh"),
                )),
            })
            .expect("send stale completion");
        manager.poll();
        assert_eq!(manager.state(&path), CpuModelLoadState::LoadingCpu);
        release_tx.send(()).expect("release retried worker");
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(executions.load(Ordering::SeqCst), 2);
        fs::remove_file(path).expect("remove controlled OBJ");
    }

    #[test]
    fn loader_panic_reaches_failed_with_distinct_error() {
        let path = temp_obj("panic", triangle());
        let mut manager =
            CpuModelLoadManager::with_loader(|_| -> Result<RasterMesh, ObjLoadError> {
                panic!("controlled loader panic");
            });
        manager.request(&path);
        for _ in 0..10_000 {
            if let CpuModelLoadState::Failed(error) = manager.state(&path) {
                assert!(error.contains("loader panic"));
                assert!(error.contains("controlled loader panic"));
                assert_ne!(error, ObjLoadError::Empty.to_string());
                fs::remove_file(path).expect("remove controlled OBJ");
                return;
            }
            std::thread::yield_now();
        }
        panic!("loader panic did not reach Failed");
    }

    #[test]
    fn valid_worker_completes_after_another_worker_panics() {
        let panic_path = temp_obj("panic_isolated", triangle());
        let valid_path = temp_obj("valid_after_panic", triangle());
        let mut manager = CpuModelLoadManager::with_loader(move |path| {
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("panic_isolated"))
            {
                panic!("isolated panic");
            }
            load_obj(path)
        });
        manager.request(&panic_path);
        manager.request(&valid_path);
        for _ in 0..10_000 {
            manager.poll();
            if matches!(manager.state(&panic_path), CpuModelLoadState::Failed(_))
                && matches!(manager.state(&valid_path), CpuModelLoadState::CpuReady)
            {
                assert!(manager.mesh(&valid_path).is_some());
                fs::remove_file(panic_path).expect("remove panic OBJ");
                fs::remove_file(valid_path).expect("remove valid OBJ");
                return;
            }
            std::thread::yield_now();
        }
        panic!("panic isolation did not complete");
    }

    #[test]
    fn stale_panic_result_cannot_overwrite_new_generation() {
        let path = temp_obj("panic_generation", triangle());
        let mut manager = CpuModelLoadManager::with_loader(|_| Err(ObjLoadError::Empty));
        manager.request(&path);
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        let old_generation = manager.entries.get(&path).expect("entry").generation;
        assert!(manager.retry(&path));
        manager
            .completion_tx
            .send(Completion {
                path: path.clone(),
                generation: old_generation,
                result: Err("loader panic: stale generation".into()),
            })
            .expect("send stale panic");
        manager.poll();
        assert_eq!(manager.state(&path), CpuModelLoadState::LoadingCpu);
        for _ in 0..10_000 {
            if matches!(manager.state(&path), CpuModelLoadState::Failed(_)) {
                break;
            }
            std::thread::yield_now();
        }
        fs::remove_file(path).expect("remove controlled OBJ");
    }
}

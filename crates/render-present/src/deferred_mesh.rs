use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use render_core::{
    CpuModelLoadManager, CpuModelLoadState, GpuAssetEvent, GpuAssetState, MaterialHandle,
    MeshHandle, SnapshotScene, VisualAssetEvent, VisualAssetRequest, VisualAssetState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeferredMeshStatus {
    LoadingCpu,
    Registered,
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeferredMeshRequestError {
    AlreadyRequested(PathBuf, MeshHandle, MaterialHandle),
    ConflictingHandles(PathBuf, MeshHandle, MaterialHandle),
}

#[derive(Debug)]
pub struct VisualAssetClient {
    request_tx: Sender<VisualAssetRequest>,
    event_rx: Receiver<VisualAssetEvent>,
    gpu_event_rx: Receiver<GpuAssetEvent>,
    closed: bool,
    gpu_closed: bool,
}

impl VisualAssetClient {
    pub fn request(&self, request: VisualAssetRequest) -> Result<(), VisualAssetRequest> {
        self.request_tx.send(request).map_err(|error| error.0)
    }

    pub fn poll_events(&mut self) -> Vec<VisualAssetEvent> {
        let mut events = Vec::new();
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    break;
                }
            }
        }
        events
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn poll_gpu_events(&mut self) -> Vec<GpuAssetEvent> {
        let mut events = Vec::new();
        loop {
            match self.gpu_event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.gpu_closed = true;
                    break;
                }
            }
        }
        events
    }

    pub fn gpu_channel_closed(&self) -> bool {
        self.gpu_closed
    }
}

pub struct VisualAssetHost {
    request_rx: Receiver<VisualAssetRequest>,
    event_tx: Sender<VisualAssetEvent>,
    gpu_event_tx: Sender<GpuAssetEvent>,
    associations: BTreeMap<String, VisualAssetRequest>,
    gpu_states: BTreeMap<(MeshHandle, render_core::LodLevel), GpuAssetEvent>,
    gpu_backend_active: bool,
}

pub fn visual_asset_channel() -> (VisualAssetClient, VisualAssetHost) {
    let (request_tx, request_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (gpu_event_tx, gpu_event_rx) = mpsc::channel();
    (
        VisualAssetClient {
            request_tx,
            event_rx,
            gpu_event_rx,
            closed: false,
            gpu_closed: false,
        },
        VisualAssetHost {
            request_rx,
            event_tx,
            gpu_event_tx,
            associations: BTreeMap::new(),
            gpu_states: BTreeMap::new(),
            gpu_backend_active: false,
        },
    )
}

impl Default for VisualAssetHost {
    fn default() -> Self {
        let (_client, host) = visual_asset_channel();
        host
    }
}

impl VisualAssetHost {
    pub fn publish_gpu_events(&mut self, events: impl IntoIterator<Item = GpuAssetEvent>) {
        for event in events {
            let key = (event.mesh, event.lod);
            self.gpu_states.insert(key, event.clone());
            let _ = self.gpu_event_tx.send(event);
        }
    }

    /// Publishes one Unavailable transition when presentation is no longer on
    /// the GPU backend. This prevents an old GpuReady event from remaining
    /// authoritative after a backend switch.
    pub fn set_gpu_backend_active(&mut self, active: bool) {
        if self.gpu_backend_active == active {
            return;
        }
        self.gpu_backend_active = active;
        if !active {
            let events = self
                .gpu_states
                .values()
                .filter(|event| !matches!(event.state, GpuAssetState::Unavailable))
                .map(|event| GpuAssetEvent {
                    mesh: event.mesh,
                    lod: event.lod,
                    renderer_epoch: event.renderer_epoch,
                    generation: event.generation.wrapping_add(1),
                    state: GpuAssetState::Unavailable,
                    error: None,
                })
                .collect::<Vec<_>>();
            self.publish_gpu_events(events);
        }
    }

    pub fn poll(
        &mut self,
        registry: &mut DeferredMeshRegistry,
        scene: Option<&mut SnapshotScene>,
    ) -> Vec<VisualAssetEvent> {
        let mut emitted = Vec::new();
        loop {
            let request = match self.request_rx.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            emitted.extend(self.accept_request(registry, request));
        }
        if let Some(scene) = scene {
            for (path, status) in registry.poll(scene) {
                if let Some(request) = self
                    .associations
                    .values()
                    .find(|request| request.path == path)
                {
                    let event = status_event(&request.resource_id, status);
                    let _ = self.event_tx.send(event.clone());
                    emitted.push(event);
                }
            }
        }
        emitted
    }

    fn accept_request(
        &mut self,
        registry: &mut DeferredMeshRegistry,
        request: VisualAssetRequest,
    ) -> Vec<VisualAssetEvent> {
        if request.resource_id.is_empty()
            || request.path.as_os_str().is_empty()
            || !request.mesh.is_valid()
            || !request.material.is_valid()
        {
            return vec![self.emit_failure(&request, "invalid visual asset request")];
        }
        if let Some(previous) = self.associations.get(&request.resource_id) {
            if previous != &request {
                return vec![self.emit_failure(&request, "visual asset identity already assigned")];
            }
            return self.current_status(registry, previous);
        }
        if self
            .associations
            .values()
            .any(|previous| previous.path == request.path)
        {
            return vec![self.emit_failure(&request, "visual asset path already assigned")];
        }
        if self.associations.values().any(|previous| {
            previous.mesh == request.mesh
                && previous.material == request.material
                && previous.path != request.path
        }) {
            return vec![self.emit_failure(
                &request,
                "visual asset handles already assigned to another path",
            )];
        }
        match registry.request(request.path.clone(), request.mesh, request.material) {
            Ok(_) => {
                self.associations
                    .insert(request.resource_id.clone(), request.clone());
                vec![self.emit(&request, VisualAssetState::Loading, None)]
            }
            Err(DeferredMeshRequestError::AlreadyRequested(_, _, _)) => {
                self.associations
                    .insert(request.resource_id.clone(), request.clone());
                self.current_status(registry, &request)
            }
            Err(error) => vec![self.emit_failure(&request, &format!("{error:?}"))],
        }
    }

    fn current_status(
        &self,
        registry: &mut DeferredMeshRegistry,
        request: &VisualAssetRequest,
    ) -> Vec<VisualAssetEvent> {
        match registry.status(&request.path) {
            Some(DeferredMeshStatus::Registered) => {
                vec![self.emit(request, VisualAssetState::Ready, None)]
            }
            Some(DeferredMeshStatus::Failed(error)) => {
                vec![self.emit(request, VisualAssetState::Failed, Some(error))]
            }
            _ => vec![self.emit(request, VisualAssetState::Loading, None)],
        }
    }

    fn emit_failure(&self, request: &VisualAssetRequest, error: &str) -> VisualAssetEvent {
        self.emit(request, VisualAssetState::Failed, Some(error.to_owned()))
    }

    fn emit(
        &self,
        request: &VisualAssetRequest,
        state: VisualAssetState,
        error: Option<String>,
    ) -> VisualAssetEvent {
        let event = VisualAssetEvent {
            resource_id: request.resource_id.clone(),
            state,
            error,
        };
        let _ = self.event_tx.send(event.clone());
        event
    }
}

fn status_event(resource_id: &str, status: DeferredMeshStatus) -> VisualAssetEvent {
    match status {
        DeferredMeshStatus::Registered => VisualAssetEvent {
            resource_id: resource_id.to_owned(),
            state: VisualAssetState::Ready,
            error: None,
        },
        DeferredMeshStatus::Failed(error) => VisualAssetEvent {
            resource_id: resource_id.to_owned(),
            state: VisualAssetState::Failed,
            error: Some(error),
        },
        DeferredMeshStatus::LoadingCpu => VisualAssetEvent {
            resource_id: resource_id.to_owned(),
            state: VisualAssetState::Loading,
            error: None,
        },
    }
}

struct Request {
    mesh: MeshHandle,
    material: MaterialHandle,
    status: DeferredMeshStatus,
}

/// Presentation-thread integration boundary for CPU-loaded meshes.
///
/// `CpuModelLoadManager` owns the workers and immutable results. This type is
/// polled by the thread that owns `SnapshotScene`; workers never receive a
/// scene or visual-assets reference.
pub struct DeferredMeshRegistry {
    manager: CpuModelLoadManager,
    requests: BTreeMap<PathBuf, Request>,
}

impl Default for DeferredMeshRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeferredMeshRegistry {
    pub fn new() -> Self {
        Self {
            manager: CpuModelLoadManager::new(),
            requests: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    fn with_manager(manager: CpuModelLoadManager) -> Self {
        Self {
            manager,
            requests: BTreeMap::new(),
        }
    }

    /// Starts or reuses a CPU request. This operation never waits for parsing.
    pub fn request(
        &mut self,
        path: impl Into<PathBuf>,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Result<DeferredMeshStatus, DeferredMeshRequestError> {
        let path = path.into();
        if let Some(request) = self.requests.get(&path) {
            if request.mesh == mesh && request.material == material {
                return Err(DeferredMeshRequestError::AlreadyRequested(
                    path, mesh, material,
                ));
            }
            return Err(DeferredMeshRequestError::ConflictingHandles(
                path, mesh, material,
            ));
        }
        if self.requests.iter().any(|(existing_path, request)| {
            request.mesh == mesh && request.material == material && *existing_path != path
        }) {
            return Err(DeferredMeshRequestError::ConflictingHandles(
                path, mesh, material,
            ));
        }
        let status = match self.manager.request(path.clone()) {
            CpuModelLoadState::LoadingCpu => DeferredMeshStatus::LoadingCpu,
            CpuModelLoadState::CpuReady => DeferredMeshStatus::LoadingCpu,
            CpuModelLoadState::Failed(error) => DeferredMeshStatus::Failed(error),
            CpuModelLoadState::Unloaded => {
                DeferredMeshStatus::Failed("CPU model request did not enter a loading state".into())
            }
        };
        if mesh == MeshHandle::NARETH_PLANET {
            eprintln!(
                "nareth-diag state=FALLBACK_LOADING_CPU destination=9603 mesh={} material={} path={} registry_status={:?}",
                mesh.0, material.0, path.display(), status
            );
        }
        self.requests.insert(
            path,
            Request {
                mesh,
                material,
                status: status.clone(),
            },
        );
        Ok(status)
    }

    pub fn status(&mut self, path: impl AsRef<Path>) -> Option<DeferredMeshStatus> {
        self.poll_manager();
        self.requests
            .get(path.as_ref())
            .map(|request| request.status.clone())
    }

    /// Collects completed results and registers them on the scene owner.
    /// This method is non-blocking and must run on the scene-owning thread.
    pub fn poll(&mut self, scene: &mut SnapshotScene) -> Vec<(PathBuf, DeferredMeshStatus)> {
        self.poll_manager();
        let paths = self.requests.keys().cloned().collect::<Vec<_>>();
        let mut completed = Vec::new();
        for path in paths {
            let Some(request) = self.requests.get_mut(&path) else {
                continue;
            };
            if !matches!(request.status, DeferredMeshStatus::LoadingCpu) {
                continue;
            }
            match self.manager.state(&path) {
                CpuModelLoadState::LoadingCpu | CpuModelLoadState::Unloaded => {}
                CpuModelLoadState::Failed(error) => {
                    if request.mesh == MeshHandle::NARETH_PLANET {
                        eprintln!(
                            "nareth-diag state=FALLBACK_CPU_FAILED destination=9603 path={} error={}",
                            path.display(), error
                        );
                    }
                    request.status = DeferredMeshStatus::Failed(error);
                    completed.push((path, request.status.clone()));
                }
                CpuModelLoadState::CpuReady => {
                    let Some(mesh) = self.manager.mesh(&path) else {
                        request.status = DeferredMeshStatus::Failed(
                            "CPU loader reported ready without a mesh".into(),
                        );
                        completed.push((path, request.status.clone()));
                        continue;
                    };
                    let diagnostic = if request.mesh == MeshHandle::NARETH_PLANET {
                        Some((
                            std::sync::Arc::as_ptr(&mesh),
                            mesh.vertices.len(),
                            mesh.indices.len(),
                        ))
                    } else {
                        None
                    };
                    let result = scene.register_shared_mesh(request.mesh, request.material, mesh);
                    request.status = match result {
                        Ok(()) => {
                            if let Some((arc_ptr, vertices, indices)) = diagnostic {
                                eprintln!(
                                    "nareth-diag state=FALLBACK_CPU_REGISTERED_GPU_PENDING destination=9603 path={} arc_ptr={:p} vertices={} indices={} triangles={}",
                                    path.display(), arc_ptr, vertices, indices, indices / 3
                                );
                            }
                            DeferredMeshStatus::Registered
                        }
                        Err(error) => {
                            if request.mesh == MeshHandle::NARETH_PLANET {
                                eprintln!(
                                    "nareth-diag state=FALLBACK_CPU_FAILED destination=9603 path={} error=registration:{}",
                                    path.display(), error
                                );
                            }
                            DeferredMeshStatus::Failed(error.to_string())
                        }
                    };
                    completed.push((path, request.status.clone()));
                }
            }
        }
        completed
    }

    fn poll_manager(&mut self) {
        self.manager.poll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_core::{
        load_obj, CpuModelLoadManager, CullingConfig, RenderSnapshot, SnapshotNode, Transform,
        VisualError,
    };
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn fixture_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "astral_echo_deferred_mesh_{name}_{}_{}.obj",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn write_triangle(path: &Path) {
        fs::write(path, "v -1 -1 0\nv 1 -1 0\nv 0 1 0\nf 1 2 3\n").expect("write fixture OBJ");
    }

    #[test]
    fn gpu_events_are_forwarded_once_and_cpu_backend_invalidates_ready() {
        let (mut client, mut host) = visual_asset_channel();
        let ready = GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: render_core::LodLevel::High,
            renderer_epoch: 1,
            generation: 1,
            state: GpuAssetState::GpuReady,
            error: None,
        };
        host.publish_gpu_events([ready.clone()]);
        assert_eq!(client.poll_gpu_events(), vec![ready]);
        assert!(client.poll_gpu_events().is_empty());

        host.set_gpu_backend_active(true);
        host.set_gpu_backend_active(false);
        let events = client.poll_gpu_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, GpuAssetState::Unavailable);
        assert!(client.poll_gpu_events().is_empty());
    }

    fn snapshot(mesh: MeshHandle, material: MaterialHandle) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::minimal_demo();
        snapshot.nodes.clear();
        snapshot.push_node(SnapshotNode::destination(
            77,
            Transform::default(),
            mesh,
            material,
            true,
        ));
        snapshot
    }

    fn wait_registered(
        registry: &mut DeferredMeshRegistry,
        scene: &mut SnapshotScene,
        path: &Path,
    ) {
        for _ in 0..10_000 {
            registry.poll(scene);
            if registry.status(path) == Some(DeferredMeshStatus::Registered) {
                return;
            }
            std::thread::yield_now();
        }
        panic!("deferred mesh did not register");
    }

    #[test]
    fn request_is_nonblocking_and_mesh_is_absent_until_registration() {
        let path = fixture_path("nonblocking");
        write_triangle(&path);
        let mesh = MeshHandle::new(30_000);
        let material = MaterialHandle::new(30_000);
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        assert_eq!(
            registry.request(&path, mesh, material),
            Ok(DeferredMeshStatus::LoadingCpu)
        );
        assert_eq!(
            scene.update(&snapshot(mesh, material)),
            Err(VisualError::UnknownAsset(mesh, material))
        );
        wait_registered(&mut registry, &mut scene, &path);
        assert!(scene.update(&snapshot(mesh, material)).is_ok());
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn completed_load_is_registered_and_prepared_without_cloning_the_arc() {
        let path = fixture_path("prepared");
        write_triangle(&path);
        let mesh = MeshHandle::new(30_001);
        let material = MaterialHandle::new(30_001);
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        registry.request(&path, mesh, material).unwrap();
        wait_registered(&mut registry, &mut scene, &path);
        scene.update(&snapshot(mesh, material)).unwrap();
        let loaded = registry.manager.mesh(&path).expect("loaded mesh");
        let registered = scene.shared_mesh(mesh, material).expect("registered mesh");
        assert!(std::sync::Arc::ptr_eq(&loaded, &registered));
        let prepared = scene.prepare(CullingConfig::default());
        assert_eq!(prepared.stats().draws, 1);
        assert_eq!(prepared.stats().triangles, 1);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn prepared_scene_remains_valid_when_a_later_mesh_is_registered() {
        let path = fixture_path("prepared_before_registration");
        write_triangle(&path);
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        scene.update(&RenderSnapshot::minimal_demo()).unwrap();
        let prepared = scene.prepare(CullingConfig::default());
        let previous_stats = prepared.stats();
        registry
            .request(&path, MeshHandle::new(30_005), MaterialHandle::new(30_005))
            .unwrap();
        wait_registered(&mut registry, &mut scene, &path);
        assert_eq!(prepared.stats(), previous_stats);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn duplicate_request_and_registration_are_controlled() {
        let path = fixture_path("duplicate");
        write_triangle(&path);
        let executions = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&executions);
        let manager = CpuModelLoadManager::with_loader(move |obj_path| {
            count.fetch_add(1, Ordering::SeqCst);
            load_obj(obj_path)
        });
        let mut registry = DeferredMeshRegistry::with_manager(manager);
        let mut scene = SnapshotScene::default();
        let mesh = MeshHandle::new(30_002);
        let material = MaterialHandle::new(30_002);
        registry.request(&path, mesh, material).unwrap();
        assert_eq!(
            registry.request(&path, mesh, material),
            Err(DeferredMeshRequestError::AlreadyRequested(
                path.clone(),
                mesh,
                material
            ))
        );
        wait_registered(&mut registry, &mut scene, &path);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn failed_load_is_reported_without_affecting_existing_scene_assets() {
        let path = fixture_path("invalid");
        fs::write(&path, "v 0 0 0\nf 1 2 3\n").expect("write invalid OBJ");
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        let mesh = MeshHandle::new(30_003);
        let material = MaterialHandle::new(30_003);
        registry.request(&path, mesh, material).unwrap();
        for _ in 0..10_000 {
            registry.poll(&mut scene);
            if matches!(registry.status(&path), Some(DeferredMeshStatus::Failed(_))) {
                break;
            }
            std::thread::yield_now();
        }
        assert!(matches!(
            registry.status(&path),
            Some(DeferredMeshStatus::Failed(_))
        ));
        assert!(scene
            .mesh(&SnapshotNode::destination(
                1,
                Transform::default(),
                MeshHandle::CUBE,
                render_core::VisualAssets::DESTINATION,
                true,
            ))
            .is_ok());
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn manager_can_be_polled_without_waiting_for_a_worker() {
        let path = fixture_path("poll");
        write_triangle(&path);
        let manager = CpuModelLoadManager::with_loader(move |obj_path| {
            std::thread::sleep(Duration::from_millis(20));
            load_obj(obj_path)
        });
        let mut registry = DeferredMeshRegistry::with_manager(manager);
        let mut scene = SnapshotScene::default();
        let mesh = MeshHandle::new(30_004);
        let material = MaterialHandle::new(30_004);
        registry.request(&path, mesh, material).unwrap();
        let started = std::time::Instant::now();
        registry.poll(&mut scene);
        assert!(started.elapsed() < Duration::from_millis(10));
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn typed_channel_transports_requests_events_and_handles_without_blocking() {
        let (mut client, mut host) = visual_asset_channel();
        let path = fixture_path("channel");
        write_triangle(&path);
        let request = VisualAssetRequest {
            resource_id: "fixture.channel".into(),
            path: path.clone(),
            mesh: MeshHandle::new(30_010),
            material: MaterialHandle::new(30_010),
        };
        client
            .request(request.clone())
            .expect("request should send");
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        let loading = host.poll(&mut registry, Some(&mut scene));
        assert_eq!(loading.len(), 1);
        assert_eq!(loading[0].resource_id, request.resource_id);
        assert_eq!(loading[0].state, VisualAssetState::Loading);
        assert_eq!(client.poll_events(), loading);

        for _ in 0..10_000 {
            host.poll(&mut registry, Some(&mut scene));
            let events = client.poll_events();
            if events
                .iter()
                .any(|event| event.state == VisualAssetState::Ready)
            {
                assert!(scene.shared_mesh(request.mesh, request.material).is_ok());
                fs::remove_file(path).expect("remove fixture");
                return;
            }
            std::thread::yield_now();
        }
        panic!("typed channel did not deliver Ready");
    }

    #[test]
    fn typed_channel_reports_failures_and_deduplicates_identity() {
        let (mut client, mut host) = visual_asset_channel();
        let path = fixture_path("channel_invalid");
        fs::write(&path, "v 0 0 0\nf 1 2 3\n").expect("write invalid fixture");
        let request = VisualAssetRequest {
            resource_id: "fixture.invalid".into(),
            path: path.clone(),
            mesh: MeshHandle::new(30_011),
            material: MaterialHandle::new(30_011),
        };
        client.request(request.clone()).unwrap();
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        host.poll(&mut registry, Some(&mut scene));
        assert_eq!(client.poll_events().len(), 1);
        client.request(request).unwrap();
        let duplicate = host.poll(&mut registry, Some(&mut scene));
        assert_eq!(duplicate.len(), 1);
        assert_eq!(duplicate[0].state, VisualAssetState::Loading);
        let mut failed = false;
        for _ in 0..10_000 {
            host.poll(&mut registry, Some(&mut scene));
            failed |= client
                .poll_events()
                .iter()
                .any(|event| event.state == VisualAssetState::Failed);
            if failed {
                break;
            }
            std::thread::yield_now();
        }
        assert!(failed);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn typed_channel_rejects_a_different_path_for_an_existing_identity() {
        let (mut client, mut host) = visual_asset_channel();
        let first_path = fixture_path("identity_first");
        let second_path = fixture_path("identity_second");
        write_triangle(&first_path);
        write_triangle(&second_path);
        let first = VisualAssetRequest {
            resource_id: "planet.nareth".into(),
            path: first_path.clone(),
            mesh: MeshHandle::NARETH_PLANET,
            material: render_core::VisualAssets::PLANET,
        };
        client.request(first.clone()).unwrap();
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        host.poll(&mut registry, Some(&mut scene));
        let _ = client.poll_events();

        client
            .request(VisualAssetRequest {
                resource_id: first.resource_id.clone(),
                path: second_path.clone(),
                mesh: first.mesh,
                material: first.material,
            })
            .unwrap();
        let events = host.poll(&mut registry, Some(&mut scene));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, VisualAssetState::Failed);
        assert!(events[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("identity already assigned")));

        fs::remove_file(first_path).expect("remove first fixture");
        fs::remove_file(second_path).expect("remove second fixture");
    }

    #[test]
    fn typed_channel_rejects_another_resource_from_claiming_nareth_handles() {
        let (mut client, mut host) = visual_asset_channel();
        let nareth_path = fixture_path("nareth_handles");
        let test_path = fixture_path("test_handles");
        write_triangle(&nareth_path);
        write_triangle(&test_path);
        let nareth = VisualAssetRequest {
            resource_id: "planet.nareth".into(),
            path: nareth_path.clone(),
            mesh: MeshHandle::NARETH_PLANET,
            material: render_core::VisualAssets::PLANET,
        };
        client.request(nareth.clone()).unwrap();
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = SnapshotScene::default();
        host.poll(&mut registry, Some(&mut scene));
        let _ = client.poll_events();

        client
            .request(VisualAssetRequest {
                resource_id: "fixture.earth".into(),
                path: test_path.clone(),
                mesh: nareth.mesh,
                material: nareth.material,
            })
            .unwrap();
        let events = host.poll(&mut registry, Some(&mut scene));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, VisualAssetState::Failed);
        assert!(events[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("handles already assigned")));

        fs::remove_file(nareth_path).expect("remove Nareth fixture");
        fs::remove_file(test_path).expect("remove test fixture");
    }

    #[test]
    fn typed_channel_closes_without_panicking_or_blocking() {
        let (mut client, host) = visual_asset_channel();
        drop(host);
        assert!(client
            .request(VisualAssetRequest {
                resource_id: "closed".into(),
                path: PathBuf::from("fixture.obj"),
                mesh: MeshHandle::new(30_012),
                material: MaterialHandle::new(30_012),
            })
            .is_err());
        assert!(client.poll_events().is_empty());
        assert!(client.is_closed());
    }
}
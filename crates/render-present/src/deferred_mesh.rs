use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use render_core::{
    CpuModelLoadManager, CpuModelLoadState, MaterialHandle, MeshHandle, SnapshotScene,
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
                    path,
                    mesh,
                    material,
                ));
            }
            return Err(DeferredMeshRequestError::ConflictingHandles(
                path,
                mesh,
                material,
            ));
        }
        let status = match self.manager.request(path.clone()) {
            CpuModelLoadState::LoadingCpu => DeferredMeshStatus::LoadingCpu,
            CpuModelLoadState::CpuReady => DeferredMeshStatus::LoadingCpu,
            CpuModelLoadState::Failed(error) => DeferredMeshStatus::Failed(error),
            CpuModelLoadState::Unloaded => DeferredMeshStatus::Failed(
                "CPU model request did not enter a loading state".into(),
            ),
        };
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
        self.requests.get(path.as_ref()).map(|request| request.status.clone())
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
                    let result = scene.register_shared_mesh(request.mesh, request.material, mesh);
                    request.status = match result {
                        Ok(()) => DeferredMeshStatus::Registered,
                        Err(error) => DeferredMeshStatus::Failed(error.to_string()),
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
        load_obj, CullingConfig, CpuModelLoadManager, RenderSnapshot, SnapshotNode, Transform,
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
        fs::write(path, "v -1 -1 0\nv 1 -1 0\nv 0 1 0\nf 1 2 3\n")
            .expect("write fixture OBJ");
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
        assert_eq!(scene.update(&snapshot(mesh, material)), Err(VisualError::UnknownAsset(mesh, material)));
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
                path.clone(), mesh, material
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
        assert!(matches!(registry.status(&path), Some(DeferredMeshStatus::Failed(_))));
        assert!(scene.mesh(&SnapshotNode::destination(
            1,
            Transform::default(),
            MeshHandle::CUBE,
            render_core::VisualAssets::DESTINATION,
            true,
        )).is_ok());
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
}
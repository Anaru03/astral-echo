use std::path::PathBuf;

use crate::LodLevel;
use crate::{MaterialHandle, MeshHandle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualAssetRequest {
    pub resource_id: String,
    pub path: PathBuf,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualAssetState {
    Loading,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualAssetEvent {
    pub resource_id: String,
    pub state: VisualAssetState,
    pub error: Option<String>,
}

/// Availability of a visual resource in the GPU-owned cache.
///
/// This state is deliberately independent from [`VisualAssetState`], whose
/// `Ready` variant means that the CPU mesh has been registered in the scene.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuAssetState {
    Unavailable,
    UploadingGpu,
    GpuReady,
    GpuFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuAssetEvent {
    pub mesh: MeshHandle,
    pub lod: LodLevel,
    pub renderer_epoch: u64,
    pub generation: u64,
    pub state: GpuAssetState,
    pub error: Option<String>,
}

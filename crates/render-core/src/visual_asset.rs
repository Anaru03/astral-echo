use std::path::PathBuf;

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
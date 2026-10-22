use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    Camera, Mat4, MaterialHandle, MeshHandle, RasterMesh, RasterVertex, RenderSnapshot,
    SnapshotNode, Transform, Vec3, Vec4,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualError {
    UnknownAsset(MeshHandle, MaterialHandle),
    DuplicateIdentity,
    NonFinite,
}

impl fmt::Display for VisualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid visual snapshot: {self:?}")
    }
}

impl std::error::Error for VisualError {}

pub struct VisualAssets {
    meshes: BTreeMap<(MeshHandle, MaterialHandle), RasterMesh>,
}

impl VisualAssets {
    pub const SHIP: MaterialHandle = MaterialHandle::new(0);
    pub const ENEMY: MaterialHandle = MaterialHandle::new(1);
    pub const PLAYER: MaterialHandle = MaterialHandle::new(2);
    pub const DESTINATION: MaterialHandle = MaterialHandle::new(3);
    pub const COLLECTIBLE: MaterialHandle = MaterialHandle::new(4);

    pub fn mesh(
        &self,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Result<&RasterMesh, VisualError> {
        self.meshes
            .get(&(mesh, material))
            .ok_or(VisualError::UnknownAsset(mesh, material))
    }
}

impl Default for VisualAssets {
    fn default() -> Self {
        let cuboid = |scale, color| {
            let mut mesh = RasterMesh::cube(Vec3::ZERO, 1.0, color);
            for vertex in &mut mesh.vertices {
                vertex.position *= scale;
            }
            mesh
        };
        let diamond = |size: f32, color| {
            RasterMesh::new(
                [
                    Vec3::X,
                    Vec3::NEG_X,
                    Vec3::Y,
                    Vec3::NEG_Y,
                    Vec3::Z,
                    Vec3::NEG_Z,
                ]
                .into_iter()
                .map(|p| RasterVertex::new(p * size, color))
                .collect(),
                vec![
                    2, 0, 4, 2, 4, 1, 2, 1, 5, 2, 5, 0, 3, 4, 0, 3, 1, 4, 3, 5, 1, 3, 0, 5,
                ],
            )
        };
        Self {
            meshes: BTreeMap::from([
                (
                    (MeshHandle::SHIP_PLACEHOLDER, Self::SHIP),
                    cuboid(Vec3::new(4.0, 1.2, 7.0), Vec4::new(0.03, 0.5, 1.0, 1.0)),
                ),
                (
                    (MeshHandle::CUBE, Self::PLAYER),
                    cuboid(Vec3::new(1.2, 2.4, 1.2), Vec4::new(0.9, 0.9, 1.0, 1.0)),
                ),
                (
                    (MeshHandle::ENEMY_PLACEHOLDER, Self::ENEMY),
                    diamond(1.8, Vec4::new(0.9, 0.03, 0.1, 1.0)),
                ),
                (
                    (MeshHandle::DESTINATION_PLACEHOLDER, Self::DESTINATION),
                    diamond(3.0, Vec4::new(0.07, 0.85, 0.35, 1.0)),
                ),
                (
                    (MeshHandle::COLLECTIBLE_PLACEHOLDER, Self::COLLECTIBLE),
                    diamond(0.7, Vec4::new(1.0, 0.7, 0.05, 1.0)),
                ),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SceneUpdate {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
}

pub struct SnapshotScene {
    assets: VisualAssets,
    snapshot: RenderSnapshot,
}

impl Default for SnapshotScene {
    fn default() -> Self {
        let camera = Camera::demo();
        Self {
            assets: VisualAssets::default(),
            snapshot: RenderSnapshot::new(
                engine::Tick(0),
                camera.position(),
                camera.view_matrix(),
                camera.projection_matrix(),
            ),
        }
    }
}

impl SnapshotScene {
    pub fn update(&mut self, snapshot: &RenderSnapshot) -> Result<SceneUpdate, VisualError> {
        if !snapshot.view.is_finite()
            || !snapshot.projection.is_finite()
            || !snapshot.view_projection().is_finite()
            || !snapshot.camera_position.is_finite()
        {
            return Err(VisualError::NonFinite);
        }
        let mut seen = BTreeSet::new();
        for node in &snapshot.nodes {
            if !seen.insert(node.entity) {
                return Err(VisualError::DuplicateIdentity);
            }
            if !node.transform.matrix().is_finite() {
                return Err(VisualError::NonFinite);
            }
            self.assets.mesh(node.mesh, node.material)?;
        }
        let old: BTreeMap<_, _> = self.snapshot.nodes.iter().map(|n| (n.entity, n)).collect();
        let update = SceneUpdate {
            added: snapshot
                .nodes
                .iter()
                .filter(|n| !old.contains_key(&n.entity))
                .count(),
            removed: old.keys().filter(|id| !seen.contains(id)).count(),
            changed: snapshot
                .nodes
                .iter()
                .filter(|n| old.get(&n.entity).is_some_and(|previous| **previous != **n))
                .count(),
        };
        self.snapshot.clone_from(snapshot);
        self.snapshot.nodes.sort_by_key(|n| n.entity);
        Ok(update)
    }

    pub fn snapshot(&self) -> &RenderSnapshot {
        &self.snapshot
    }

    pub fn view_projection(&self) -> Mat4 {
        self.snapshot.view_projection()
    }

    pub fn visible_nodes(&self) -> impl Iterator<Item = &SnapshotNode> {
        self.snapshot.nodes.iter().filter(|n| n.visible)
    }

    pub fn mesh(&self, node: &SnapshotNode) -> Result<&RasterMesh, VisualError> {
        self.assets.mesh(node.mesh, node.material)
    }

    pub fn owned_draws(&self) -> Vec<(RasterMesh, Transform)> {
        self.draws()
            .map(|(node, mesh)| (mesh.clone(), node.transform))
            .collect()
    }

    pub fn draws(&self) -> impl Iterator<Item = (&SnapshotNode, &RasterMesh)> {
        self.visible_nodes()
            .map(|node| (node, &self.assets.meshes[&(node.mesh, node.material)]))
    }
}

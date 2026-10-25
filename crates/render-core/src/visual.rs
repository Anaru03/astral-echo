use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::{
    Aabb, Camera, Frustum, Mat4, MaterialHandle, MeshHandle, RasterMesh, RasterVertex,
    RenderSnapshot, SnapshotId, SnapshotNode, Transform, Vec3, Vec4,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LodLevel {
    Low,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibilityClass {
    Outside,
    Partial,
    Inside,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CullingConfig {
    pub width: u32,
    pub height: u32,
    pub low_threshold_px: f32,
    pub high_threshold_px: f32,
}

impl Default for CullingConfig {
    fn default() -> Self {
        Self {
            width: 640,
            height: 360,
            low_threshold_px: 12.0,
            high_threshold_px: 18.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedDraw {
    pub node: SnapshotNode,
    pub mesh: Arc<RasterMesh>,
    pub lod: LodLevel,
    pub visibility: VisibilityClass,
    pub projected_size_px: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedScene {
    draws: Vec<PreparedDraw>,
    shadow_draws: Option<Vec<PreparedDraw>>,
    pub lighting: crate::LightingEnvironment,
    materials: BTreeMap<MaterialHandle, crate::MaterialInfo>,
    pub considered: usize,
    pub visible: usize,
    pub discarded: usize,
    pub triangles: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreparedSceneStats {
    pub considered: usize,
    pub visible: usize,
    pub discarded: usize,
    pub triangles: usize,
    pub draws: usize,
}

impl PreparedScene {
    pub fn from_draws(draws: Vec<PreparedDraw>) -> Self {
        Self {
            considered: draws.len(),
            visible: draws.len(),
            discarded: 0,
            triangles: draws.iter().map(|draw| draw.mesh.indices.len() / 3).sum(),
            draws,
            shadow_draws: None,
            lighting: crate::LightingEnvironment::default(),
            materials: BTreeMap::new(),
        }
    }

    pub fn material(&self, handle: MaterialHandle) -> crate::MaterialInfo {
        self.materials.get(&handle).copied().unwrap_or_default()
    }

    pub fn stats(&self) -> PreparedSceneStats {
        PreparedSceneStats {
            considered: self.considered,
            visible: self.visible,
            discarded: self.discarded,
            triangles: self.triangles,
            draws: self.draws.len(),
        }
    }

    pub fn set_material(&mut self, handle: MaterialHandle, material: crate::MaterialInfo) {
        self.materials.insert(handle, material.sanitized());
    }

    pub fn draws(&self) -> impl DoubleEndedIterator<Item = &PreparedDraw> {
        self.draws.iter()
    }
    pub fn shadow_draws(&self) -> impl Iterator<Item = &PreparedDraw> {
        self.shadow_draws.as_deref().unwrap_or(&self.draws).iter()
    }

    pub fn owned_draws(&self) -> Vec<(RasterMesh, Transform)> {
        self.draws
            .iter()
            .map(|draw| ((*draw.mesh).clone(), draw.node.transform))
            .collect()
    }
}

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
    meshes: BTreeMap<(MeshHandle, MaterialHandle), Arc<RasterMesh>>,
    low: BTreeMap<(MeshHandle, MaterialHandle), Arc<RasterMesh>>,
    bounds: BTreeMap<(MeshHandle, MaterialHandle), Aabb>,
}

impl VisualAssets {
    pub const SHIP: MaterialHandle = MaterialHandle::new(0);
    pub const ENEMY: MaterialHandle = MaterialHandle::new(1);
    pub const PLAYER: MaterialHandle = MaterialHandle::new(2);
    pub const DESTINATION: MaterialHandle = MaterialHandle::new(3);
    pub const COLLECTIBLE: MaterialHandle = MaterialHandle::new(4);
    pub const SUN: MaterialHandle = MaterialHandle::new(5);
    pub const PLANET: MaterialHandle = MaterialHandle::new(6);

    pub fn mesh(
        &self,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Result<&RasterMesh, VisualError> {
        self.meshes
            .get(&(mesh, material))
            .map(Arc::as_ref)
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
        let low: BTreeMap<_, _> = BTreeMap::from([
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
            (
                (MeshHandle::SUN_SPHERE, Self::SUN),
                RasterMesh::sphere(
                    Vec3::ZERO,
                    1.0,
                    Vec4::new(1.0, 0.55, 0.08, 1.0),
                ),
            ),
            (
                (MeshHandle::PLANET_SPHERE, Self::PLANET),
                RasterMesh::planet_surface(Vec3::ZERO, 1.0),
            ),
        ])
        .into_iter()
        .map(|(key, mesh)| (key, Arc::new(mesh)))
        .collect();
        let bounds = low
            .iter()
            .map(|(key, mesh)| (*key, mesh_bounds(mesh)))
            .collect();
        let meshes = low.clone();
        Self {
            meshes,
            low,
            bounds,
        }
    }
}

fn subdivide(mesh: &RasterMesh) -> RasterMesh {
    let mut result = RasterMesh::default();
    let normals = mesh.resolved_normals();
    for triangle in mesh.indices.chunks_exact(3) {
        let a = mesh.vertices[triangle[0] as usize].with_normal(normals[triangle[0] as usize]);
        let b = mesh.vertices[triangle[1] as usize].with_normal(normals[triangle[1] as usize]);
        let c = mesh.vertices[triangle[2] as usize].with_normal(normals[triangle[2] as usize]);
        let midpoint = |a: RasterVertex, b: RasterVertex| {
            RasterVertex::new((a.position + b.position) * 0.5, (a.color + b.color) * 0.5)
                .with_normal((a.normal + b.normal) * 0.5)
        };
        let base = result.vertices.len() as u32;
        result
            .vertices
            .extend([a, b, c, midpoint(a, b), midpoint(b, c), midpoint(c, a)]);
        result
            .indices
            .extend([0, 3, 5, 3, 1, 4, 5, 4, 2, 3, 4, 5].map(|i| base + i));
    }
    result
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
    lighting_mode: crate::LightingMode,
    shadow_config: crate::ShadowConfig,
    materials: BTreeMap<MaterialHandle, crate::MaterialInfo>,
    lod_history: RefCell<BTreeMap<SnapshotId, (MeshHandle, MaterialHandle, LodLevel)>>,
}

impl Default for SnapshotScene {
    fn default() -> Self {
        let camera = Camera::demo();
        Self {
            assets: VisualAssets::default(),
            lod_history: RefCell::default(),
            lighting_mode: crate::LightingMode::Unlit,
            shadow_config: crate::ShadowConfig::default(),
            materials: BTreeMap::new(),
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
    pub fn set_lighting_mode(&mut self, mode: crate::LightingMode) {
        self.lighting_mode = mode;
    }

    pub fn set_shadow_config(&mut self, config: crate::ShadowConfig) {
        self.shadow_config = config;
    }

    pub fn set_material(&mut self, handle: MaterialHandle, material: crate::MaterialInfo) {
        self.materials.insert(handle, material.sanitized());
    }

    pub fn procedural_lod_fixture() -> Self {
        let mut scene = Self::default();
        scene.assets.meshes = scene
            .assets
            .low
            .iter()
            .map(|(key, mesh)| (*key, Arc::new(subdivide(mesh))))
            .collect();
        scene
    }

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
        self.lod_history
            .borrow_mut()
            .retain(|id, _| seen.contains(id));
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
        self.visible_nodes().map(|node| {
            (
                node,
                self.assets.meshes[&(node.mesh, node.material)].as_ref(),
            )
        })
    }

    pub fn prepare(&self, config: CullingConfig) -> PreparedScene {
        let mut prepared = PreparedScene {
            considered: self.snapshot.nodes.len(),
            lighting: crate::LightingEnvironment::from_snapshot(&self.snapshot, self.lighting_mode),
            materials: self.materials.clone(),
            ..Default::default()
        };
        prepared.lighting.shadows = self.shadow_config;
        let shadow_frustum = prepared
            .lighting
            .shadow_view_projection()
            .ok()
            .flatten()
            .map(Frustum::from_view_projection);
        if shadow_frustum.is_some() {
            prepared.shadow_draws = Some(Vec::new());
        }
        let mut history = self.lod_history.borrow_mut();
        let vp = self.view_projection();
        let frustum = Frustum::from_view_projection(self.view_projection());
        for node in self.snapshot.nodes.iter().filter(|node| node.visible) {
            let high = &self.assets.meshes[&(node.mesh, node.material)];
            let local = self.assets.bounds[&(node.mesh, node.material)];
            let transformed = local.transformed(node.transform.matrix());
            let visibility = classify_bounds(transformed, frustum);
            if visibility == VisibilityClass::Outside {
                prepared.discarded += 1;
                if shadow_frustum.is_some_and(|frustum| frustum.intersects_aabb(transformed)) {
                    let lod = history
                        .get(&node.entity)
                        .filter(|(mesh, material, _)| {
                            *mesh == node.mesh && *material == node.material
                        })
                        .map_or(LodLevel::High, |(_, _, lod)| *lod);
                    let mesh = if lod == LodLevel::Low {
                        self.assets.low[&(node.mesh, node.material)].clone()
                    } else {
                        high.clone()
                    };
                    prepared
                        .shadow_draws
                        .as_mut()
                        .expect("shadow list")
                        .push(PreparedDraw {
                            node: *node,
                            mesh,
                            lod,
                            visibility,
                            projected_size_px: 0.0,
                        });
                }
                continue;
            }
            let projected = projected_size(transformed, vp, config.width, config.height);
            let previous = history
                .get(&node.entity)
                .filter(|(mesh, material, _)| *mesh == node.mesh && *material == node.material)
                .map(|(_, _, lod)| *lod);
            let lod = match previous {
                Some(LodLevel::Low) if projected < config.high_threshold_px => LodLevel::Low,
                _ if projected < config.low_threshold_px => LodLevel::Low,
                _ => LodLevel::High,
            };
            history.insert(node.entity, (node.mesh, node.material, lod));
            let mesh = if lod == LodLevel::Low {
                self.assets.low[&(node.mesh, node.material)].clone()
            } else {
                high.clone()
            };
            prepared.triangles += mesh.indices.len() / 3;
            prepared.visible += 1;
            let draw = PreparedDraw {
                node: *node,
                mesh,
                lod,
                visibility,
                projected_size_px: projected,
            };
            if shadow_frustum.is_some_and(|frustum| frustum.intersects_aabb(transformed)) {
                prepared
                    .shadow_draws
                    .as_mut()
                    .expect("shadow list")
                    .push(draw.clone());
            }
            prepared.draws.push(draw);
        }
        prepared.discarded += self
            .snapshot
            .nodes
            .iter()
            .filter(|node| !node.visible)
            .count();
        prepared
    }
}

fn mesh_bounds(mesh: &RasterMesh) -> Aabb {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in &mesh.vertices {
        min = min.min(vertex.position);
        max = max.max(vertex.position);
    }
    Aabb { min, max }
}

fn classify_bounds(aabb: Aabb, frustum: Frustum) -> VisibilityClass {
    if !frustum.intersects_aabb(aabb) {
        return VisibilityClass::Outside;
    }
    if aabb
        .corners()
        .into_iter()
        .all(|corner| frustum.contains_point(corner))
    {
        VisibilityClass::Inside
    } else {
        VisibilityClass::Partial
    }
}

fn projected_size(bounds: Aabb, vp: Mat4, width: u32, height: u32) -> f32 {
    let mut min = crate::Vec2::splat(f32::INFINITY);
    let mut max = crate::Vec2::splat(f32::NEG_INFINITY);
    for corner in bounds.corners() {
        let clip = vp * corner.extend(1.0);
        if clip.w <= f32::EPSILON || clip.z <= 0.0 {
            return f32::INFINITY;
        }
        let point = crate::Vec2::new(clip.x, clip.y) / clip.w;
        min = min.min(point);
        max = max.max(point);
    }
    let pixels = (max - min) * crate::Vec2::new(width as f32, height as f32) * 0.5;
    pixels.max_element()
}
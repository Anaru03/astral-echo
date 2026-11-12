use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::{
    Aabb, Camera, Frustum, Mat4, MaterialHandle, MaterialTextureRegistry, MeshHandle, RasterMesh,
    RasterVertex, RenderSnapshot, SnapshotId, SnapshotNode, Transform, Vec3, Vec4,
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

impl CullingConfig {
    /// Low-resolution windows should prefer the bounded mesh copies. This is
    /// especially important for the full ECHO-7 mount, whose high meshes are
    /// intentionally detailed for inspection and close-up screenshots.
    pub fn for_viewport(width: u32, height: u32) -> Self {
        let mut config = Self {
            width,
            height,
            ..Default::default()
        };
        if width <= 640 && height <= 360 {
            config.low_threshold_px = f32::MAX;
            config.high_threshold_px = f32::MAX;
        }
        config
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
    textures: Arc<MaterialTextureRegistry>,
    draws: Vec<PreparedDraw>,
    shadow_draws: Option<Vec<PreparedDraw>>,
    pub lighting: crate::LightingEnvironment,
    materials: Arc<BTreeMap<MaterialHandle, crate::MaterialInfo>>,
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
    pub fn texture_path(&self, material: MaterialHandle) -> Option<&std::path::Path> {
        self.textures.get(material)
    }

    pub fn texture_image(
        &self,
        material: MaterialHandle,
    ) -> Option<&crate::material_texture::DecodedTexture> {
        self.textures.image_ref(material)
    }

    pub fn textures(&self) -> &MaterialTextureRegistry {
        &self.textures
    }

    pub fn from_draws(draws: Vec<PreparedDraw>) -> Self {
        Self {
            considered: draws.len(),
            visible: draws.len(),
            discarded: 0,
            triangles: draws.iter().map(|draw| draw.mesh.indices.len() / 3).sum(),
            draws,
            textures: Arc::new(MaterialTextureRegistry::default()),
            shadow_draws: None,
            lighting: crate::LightingEnvironment::default(),
            materials: Arc::new(BTreeMap::new()),
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
        Arc::make_mut(&mut self.materials).insert(handle, material.sanitized());
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
    MeshAlreadyRegistered(MeshHandle, MaterialHandle),
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
    pub const ENEMY_CHARGER: MaterialHandle = MaterialHandle::new(10);
    pub const ENEMY_SHIELDED: MaterialHandle = MaterialHandle::new(11);
    pub const DESTINATION: MaterialHandle = MaterialHandle::new(3);
    pub const COLLECTIBLE: MaterialHandle = MaterialHandle::new(4);
    pub const SUN: MaterialHandle = MaterialHandle::new(5);
    pub const PLANET: MaterialHandle = MaterialHandle::new(6);
    pub const STATION: MaterialHandle = MaterialHandle::new(7);
    pub const DERELICT: MaterialHandle = MaterialHandle::new(8);
    pub const ASTEROID: MaterialHandle = MaterialHandle::new(9);
    pub const STARS: MaterialHandle = MaterialHandle::new(10);

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

    pub fn register_mesh(
        &mut self,
        mesh: MeshHandle,
        material: MaterialHandle,
        raster_mesh: RasterMesh,
    ) {
        let raster_mesh = Arc::new(raster_mesh);
        let low_mesh = low_lod_mesh(&raster_mesh);
        self.bounds
            .insert((mesh, material), mesh_bounds(&raster_mesh));
        self.meshes.insert((mesh, material), raster_mesh);
        self.low.insert((mesh, material), low_mesh);
    }

    pub fn register_shared_mesh(
        &mut self,
        mesh: MeshHandle,
        material: MaterialHandle,
        raster_mesh: Arc<RasterMesh>,
    ) -> Result<(), VisualError> {
        let key = (mesh, material);
        if self.meshes.contains_key(&key)
            || self.low.contains_key(&key)
            || self.bounds.contains_key(&key)
        {
            return Err(VisualError::MeshAlreadyRegistered(mesh, material));
        }
        let bounds = mesh_bounds(&raster_mesh);
        let low_mesh = low_lod_mesh(&raster_mesh);
        self.bounds.insert(key, bounds);
        self.meshes.insert(key, Arc::clone(&raster_mesh));
        self.low.insert(key, low_mesh);
        Ok(())
    }
}

const MAX_LOW_LOD_TRIANGLES: usize = 2_048;

fn low_lod_mesh(mesh: &Arc<RasterMesh>) -> Arc<RasterMesh> {
    if mesh.indices.len() / 3 <= MAX_LOW_LOD_TRIANGLES {
        Arc::clone(mesh)
    } else {
        Arc::new(simplify_mesh_for_lod(mesh))
    }
}

fn simplify_mesh_for_lod(mesh: &RasterMesh) -> RasterMesh {
    let triangle_count = mesh.indices.len() / 3;
    if triangle_count <= MAX_LOW_LOD_TRIANGLES {
        return mesh.clone();
    }

    let step = (triangle_count + MAX_LOW_LOD_TRIANGLES - 1) / MAX_LOW_LOD_TRIANGLES;
    let mut vertices = Vec::with_capacity(MAX_LOW_LOD_TRIANGLES * 3);
    let mut indices = Vec::with_capacity(MAX_LOW_LOD_TRIANGLES * 3);
    let mut remap = BTreeMap::<u32, u32>::new();

    for triangle in mesh.indices.chunks_exact(3).step_by(step) {
        for &source_index in triangle {
            let destination_index = if let Some(&index) = remap.get(&source_index) {
                index
            } else {
                let index = vertices.len() as u32;
                let Some(&vertex) = mesh.vertices.get(source_index as usize) else {
                    return mesh.clone();
                };
                vertices.push(vertex);
                remap.insert(source_index, index);
                index
            };
            indices.push(destination_index);
        }
    }

    if indices.len() < 3 {
        mesh.clone()
    } else {
        RasterMesh::new(vertices, indices)
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
                diamond(1.8, Vec4::ONE),
            ),
            (
                (MeshHandle::ENEMY_PLACEHOLDER, Self::ENEMY_CHARGER),
                diamond(1.8, Vec4::ONE),
            ),
            (
                (MeshHandle::ENEMY_PLACEHOLDER, Self::ENEMY_SHIELDED),
                diamond(1.8, Vec4::ONE),
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
                RasterMesh::sphere(Vec3::ZERO, 1.0, Vec4::new(1.0, 0.55, 0.08, 1.0)),
            ),
            (
                (MeshHandle::PLANET_SPHERE, Self::PLANET),
                RasterMesh::planet_surface(Vec3::ZERO, 1.0),
            ),
            (
                (MeshHandle::LANDING_PLANET_PROXY, Self::PLANET),
                RasterMesh::sphere(
                    Vec3::ZERO,
                    1.0,
                    // Nareth's resident low-detail mesh. Keep it visually
                    // distinct from PLANET_SPHERE, the old blue/green Earth
                    // placeholder, so it remains recognisable at every LOD.
                    Vec4::new(0.55, 0.16, 0.68, 1.0),
                ),
            ),
            (
                (MeshHandle::BUILDING_PLANET, Self::PLANET),
                RasterMesh::planet_surface(Vec3::ZERO, 1.0),
            ),
            (
                (MeshHandle::SUN_MODEL, Self::SUN),
                RasterMesh::sphere(Vec3::ZERO, 1.0, Vec4::new(1.0, 0.55, 0.08, 1.0)),
            ),
            (
                (MeshHandle::KHARVEX_PLANET, Self::PLANET),
                RasterMesh::planet_surface(Vec3::ZERO, 1.0),
            ),
            (
                (MeshHandle::STATION, Self::STATION),
                RasterMesh::station(Vec4::new(0.2, 0.55, 0.85, 1.0)),
            ),
            (
                (MeshHandle::DERELICT, Self::DERELICT),
                RasterMesh::derelict(Vec4::new(0.28, 0.22, 0.18, 1.0)),
            ),
            (
                (MeshHandle::ASTEROID_FIELD, Self::ASTEROID),
                RasterMesh::asteroid_field(Vec4::new(0.34, 0.31, 0.27, 1.0), 1.0, 0xA57E_001D),
            ),
            (
                (MeshHandle::STARS, Self::STARS),
                RasterMesh::star_field(Vec4::new(0.72, 0.82, 1.0, 1.0), 70.0, 384, 0x51A7_2024),
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
        let mut assets = Self {
            meshes,
            low,
            bounds,
        };
        assets.register_mesh(
            MeshHandle::CUBE,
            Self::DESTINATION,
            cuboid(Vec3::new(1.2, 2.4, 1.2), Vec4::new(0.9, 0.9, 1.0, 1.0)),
        );
        assets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_triangle() -> Arc<RasterMesh> {
        Arc::new(RasterMesh::triangle(
            RasterVertex::new(Vec3::new(-1.0, -2.0, -3.0), Vec4::ONE),
            RasterVertex::new(Vec3::new(4.0, 5.0, 6.0), Vec4::ONE),
            RasterVertex::new(Vec3::new(0.0, 1.0, 2.0), Vec4::ONE),
        ))
    }

    fn shared_mesh_handles() -> (MeshHandle, MaterialHandle) {
        (MeshHandle::new(10_000), MaterialHandle::new(10_000))
    }

    fn snapshot_for(mesh: MeshHandle, material: MaterialHandle) -> RenderSnapshot {
        let camera = Camera::demo();
        let mut snapshot = RenderSnapshot::new(
            engine::Tick(0),
            camera.position(),
            camera.view_matrix(),
            camera.projection_matrix(),
        );
        snapshot.push_node(SnapshotNode::destination(
            10_000,
            Transform::default(),
            mesh,
            material,
            true,
        ));
        snapshot
    }

    #[test]
    fn default_assets_register_cube_with_destination_material() {
        let assets = VisualAssets::default();

        assert!(assets
            .mesh(MeshHandle::CUBE, VisualAssets::DESTINATION)
            .is_ok());

        let camera = Camera::demo();
        let mut snapshot = RenderSnapshot::new(
            engine::Tick(0),
            camera.position(),
            camera.view_matrix(),
            camera.projection_matrix(),
        );
        snapshot.push_node(SnapshotNode::destination(
            1,
            Transform::default(),
            MeshHandle::CUBE,
            VisualAssets::DESTINATION,
            true,
        ));

        assert!(SnapshotScene::default().update(&snapshot).is_ok());
    }

    #[test]
    fn enemy_archetype_fallbacks_are_registered_with_distinct_material_handles() {
        let assets = VisualAssets::default();
        for material in [
            VisualAssets::ENEMY,
            VisualAssets::ENEMY_CHARGER,
            VisualAssets::ENEMY_SHIELDED,
        ] {
            assert!(assets.mesh(MeshHandle::ENEMY_PLACEHOLDER, material).is_ok());
        }
        assert_ne!(VisualAssets::ENEMY, VisualAssets::ENEMY_CHARGER);
        assert_ne!(VisualAssets::ENEMY, VisualAssets::ENEMY_SHIELDED);
    }

    #[test]
    fn building_planet_has_a_procedural_fallback() {
        let assets = VisualAssets::default();
        let fallback = assets
            .mesh(MeshHandle::BUILDING_PLANET, VisualAssets::PLANET)
            .expect("building planet fallback should be registered");
        assert!(!fallback.vertices.is_empty());
        assert!(!fallback.indices.is_empty());
        assert_eq!(fallback.indices.len() % 3, 0);
    }

    #[test]
    fn shared_mesh_registration_preserves_arc_identity_and_bounds() {
        let mut assets = VisualAssets::default();
        let shared = shared_triangle();
        let (mesh, material) = shared_mesh_handles();
        assets
            .register_shared_mesh(mesh, material, Arc::clone(&shared))
            .expect("shared mesh should register");

        assert!(Arc::ptr_eq(
            assets.meshes.get(&(mesh, material)).expect("high mesh"),
            &shared
        ));
        assert!(Arc::ptr_eq(
            assets.low.get(&(mesh, material)).expect("low mesh"),
            &shared
        ));
        assert_eq!(
            assets.bounds.get(&(mesh, material)),
            Some(&Aabb {
                min: Vec3::new(-1.0, -2.0, -3.0),
                max: Vec3::new(4.0, 5.0, 6.0),
            })
        );
    }

    #[test]
    fn shared_mesh_is_resolved_and_prepared_by_snapshot_scene() {
        let mut scene = SnapshotScene::default();
        let shared = shared_triangle();
        let (mesh, material) = shared_mesh_handles();
        scene
            .assets
            .register_shared_mesh(mesh, material, Arc::clone(&shared))
            .expect("shared mesh should register");

        scene
            .update(&snapshot_for(mesh, material))
            .expect("snapshot should resolve shared mesh");
        let prepared = scene.prepare(CullingConfig::default());
        let draw = prepared.draws().next().expect("prepared draw");
        assert!(Arc::ptr_eq(&draw.mesh, &shared));
    }

    #[test]
    fn shared_mesh_registration_rejects_collision_without_mutation() {
        let mut assets = VisualAssets::default();
        let (mesh, material) = shared_mesh_handles();
        let original = shared_triangle();
        let replacement = Arc::new(RasterMesh::triangle(
            RasterVertex::new(Vec3::ZERO, Vec4::ONE),
            RasterVertex::new(Vec3::X, Vec4::ONE),
            RasterVertex::new(Vec3::Y, Vec4::ONE),
        ));
        assets
            .register_shared_mesh(mesh, material, Arc::clone(&original))
            .expect("original mesh should register");
        let original_bounds = *assets.bounds.get(&(mesh, material)).expect("bounds");

        assert_eq!(
            assets.register_shared_mesh(mesh, material, replacement),
            Err(VisualError::MeshAlreadyRegistered(mesh, material))
        );
        assert!(Arc::ptr_eq(
            assets
                .meshes
                .get(&(mesh, material))
                .expect("original high mesh"),
            &original
        ));
        assert_eq!(
            *assets
                .bounds
                .get(&(mesh, material))
                .expect("original bounds"),
            original_bounds
        );
    }

    #[test]
    fn different_shared_handle_registers_without_affecting_existing_mesh() {
        let mut assets = VisualAssets::default();
        let first = shared_triangle();
        let second = Arc::new(RasterMesh::triangle(
            RasterVertex::new(Vec3::ZERO, Vec4::ONE),
            RasterVertex::new(Vec3::X, Vec4::ONE),
            RasterVertex::new(Vec3::Y, Vec4::ONE),
        ));
        let first_key = shared_mesh_handles();
        let second_key = (MeshHandle::new(10_001), MaterialHandle::new(10_001));
        assets
            .register_shared_mesh(first_key.0, first_key.1, Arc::clone(&first))
            .expect("first mesh should register");
        assets
            .register_shared_mesh(second_key.0, second_key.1, Arc::clone(&second))
            .expect("second mesh should register");
        assert!(Arc::ptr_eq(
            assets.meshes.get(&first_key).expect("first mesh"),
            &first
        ));
        assert!(Arc::ptr_eq(
            assets.meshes.get(&second_key).expect("second mesh"),
            &second
        ));
    }

    #[test]
    fn old_prepared_scene_keeps_its_arc_after_new_registration() {
        let mut scene = SnapshotScene::default();
        let first = shared_triangle();
        let first_key = shared_mesh_handles();
        scene
            .assets
            .register_shared_mesh(first_key.0, first_key.1, Arc::clone(&first))
            .expect("first mesh should register");
        scene
            .update(&snapshot_for(first_key.0, first_key.1))
            .expect("first snapshot should resolve");
        let prepared = scene.prepare(CullingConfig::default());

        let second_key = (MeshHandle::new(10_002), MaterialHandle::new(10_002));
        scene
            .assets
            .register_shared_mesh(second_key.0, second_key.1, shared_triangle())
            .expect("second mesh should register");

        let draw = prepared.draws().next().expect("old prepared draw");
        assert!(Arc::ptr_eq(&draw.mesh, &first));
    }

    #[test]
    fn legacy_register_mesh_remains_available() {
        let mut assets = VisualAssets::default();
        let (mesh, material) = shared_mesh_handles();
        assets.register_mesh(mesh, material, (*shared_triangle()).clone());
        assert!(assets.mesh(mesh, material).is_ok());
    }

    #[test]
    fn large_meshes_get_a_real_low_lod_copy() {
        let mut vertices = Vec::with_capacity(6_000);
        let mut indices = Vec::with_capacity(6_000);
        for triangle in 0..2_000 {
            let base = vertices.len() as u32;
            vertices.extend([
                RasterVertex::new(Vec3::new(triangle as f32, 0.0, 0.0), Vec4::ONE),
                RasterVertex::new(Vec3::new(triangle as f32, 1.0, 0.0), Vec4::ONE),
                RasterVertex::new(Vec3::new(triangle as f32, 0.0, 1.0), Vec4::ONE),
            ]);
            indices.extend([base, base + 1, base + 2]);
        }
        let mesh = RasterMesh::new(vertices, indices);
        let low = simplify_mesh_for_lod(&mesh);
        assert_eq!(low.indices.len(), mesh.indices.len());

        let mut vertices = Vec::with_capacity(12_000);
        let mut indices = Vec::with_capacity(12_000);
        for triangle in 0..4_000 {
            let base = vertices.len() as u32;
            vertices.extend([
                RasterVertex::new(Vec3::new(triangle as f32, 0.0, 0.0), Vec4::ONE),
                RasterVertex::new(Vec3::new(triangle as f32, 1.0, 0.0), Vec4::ONE),
                RasterVertex::new(Vec3::new(triangle as f32, 0.0, 1.0), Vec4::ONE),
            ]);
            indices.extend([base, base + 1, base + 2]);
        }
        let reduced = simplify_mesh_for_lod(&RasterMesh::new(vertices, indices));
        assert!(reduced.indices.len() < 12_000);
        assert_eq!(reduced.indices.len() % 3, 0);
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
    textures: Arc<MaterialTextureRegistry>,
    assets: VisualAssets,
    snapshot: RenderSnapshot,
    lighting_mode: crate::LightingMode,
    shadow_config: crate::ShadowConfig,
    materials: Arc<BTreeMap<MaterialHandle, crate::MaterialInfo>>,
    update_seen: HashSet<SnapshotId>,
    update_old: HashMap<SnapshotId, usize>,
    lod_history: RefCell<BTreeMap<SnapshotId, (MeshHandle, MaterialHandle, LodLevel)>>,
}

impl Default for SnapshotScene {
    fn default() -> Self {
        let camera = Camera::demo();
        Self {
            textures: Arc::new(MaterialTextureRegistry::default()),
            assets: VisualAssets::default(),
            lod_history: RefCell::default(),
            lighting_mode: crate::LightingMode::Unlit,
            shadow_config: crate::ShadowConfig::default(),
            materials: Arc::new(BTreeMap::new()),
            update_seen: HashSet::new(),
            update_old: HashMap::new(),
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
    pub fn register_material_texture(
        &mut self,
        material: MaterialHandle,
        path: impl Into<std::path::PathBuf>,
    ) {
        Arc::make_mut(&mut self.textures).register(material, path);
    }

    pub fn try_register_material_texture(
        &mut self,
        material: MaterialHandle,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), String> {
        Arc::make_mut(&mut self.textures).try_register(material, path)
    }

    pub fn set_material_texture_address_mode(
        &mut self,
        material: MaterialHandle,
        mode: crate::TextureAddressMode,
    ) {
        Arc::make_mut(&mut self.textures).set_address_mode(material, mode);
    }

    pub fn texture_path(&self, material: MaterialHandle) -> Option<&std::path::Path> {
        self.textures.get(material)
    }

    pub fn texture_address_mode(&self, material: MaterialHandle) -> crate::TextureAddressMode {
        self.textures.address_mode(material)
    }

    pub fn texture_image(
        &self,
        material: MaterialHandle,
    ) -> Option<&crate::material_texture::DecodedTexture> {
        self.textures.image_ref(material)
    }

    pub fn register_mesh(
        &mut self,
        mesh: MeshHandle,
        material: MaterialHandle,
        raster_mesh: RasterMesh,
    ) {
        self.assets.register_mesh(mesh, material, raster_mesh);
    }

    pub fn mesh_statistics(
        &self,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Option<(usize, usize)> {
        self.assets
            .meshes
            .get(&(mesh, material))
            .map(|mesh| (mesh.vertices.len(), mesh.indices.len() / 3))
    }

    pub fn register_shared_mesh(
        &mut self,
        mesh: MeshHandle,
        material: MaterialHandle,
        raster_mesh: Arc<RasterMesh>,
    ) -> Result<(), VisualError> {
        self.assets
            .register_shared_mesh(mesh, material, raster_mesh)
    }

    pub fn set_lighting_mode(&mut self, mode: crate::LightingMode) {
        self.lighting_mode = mode;
    }

    pub fn set_shadow_config(&mut self, config: crate::ShadowConfig) {
        self.shadow_config = config;
    }

    pub fn set_material(&mut self, handle: MaterialHandle, material: crate::MaterialInfo) {
        Arc::make_mut(&mut self.materials).insert(handle, material.sanitized());
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
        self.update_seen.clear();
        self.update_old.clear();
        for (index, node) in self.snapshot.nodes.iter().enumerate() {
            self.update_old.insert(node.entity, index);
        }
        for node in &snapshot.nodes {
            if !self.update_seen.insert(node.entity) {
                return Err(VisualError::DuplicateIdentity);
            }
            if !node.transform.matrix().is_finite() {
                return Err(VisualError::NonFinite);
            }
            self.assets.mesh(node.mesh, node.material)?;
        }
        let update = SceneUpdate {
            added: snapshot
                .nodes
                .iter()
                .filter(|node| !self.update_old.contains_key(&node.entity))
                .count(),
            removed: self
                .update_old
                .keys()
                .filter(|id| !self.update_seen.contains(id))
                .count(),
            changed: snapshot
                .nodes
                .iter()
                .filter(|node| {
                    self.update_old
                        .get(&node.entity)
                        .is_some_and(|index| self.snapshot.nodes[*index] != **node)
                })
                .count(),
        };
        self.lod_history
            .borrow_mut()
            .retain(|id, _| self.update_seen.contains(id));
        self.snapshot.clone_from(snapshot);
        self.snapshot.nodes.sort_by_key(|n| n.entity);
        Ok(update)
    }

    pub fn snapshot(&self) -> &RenderSnapshot {
        &self.snapshot
    }

    pub fn overlay(&self) -> crate::RenderOverlay {
        self.snapshot.overlay.clone()
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

    pub fn shared_mesh(
        &self,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> Result<Arc<RasterMesh>, VisualError> {
        self.assets
            .meshes
            .get(&(mesh, material))
            .cloned()
            .ok_or(VisualError::UnknownAsset(mesh, material))
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
            textures: self.textures.clone(),
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

pub mod bounds;
pub mod camera;
pub mod demo;
pub mod geometry;
pub mod lighting;
pub mod material;
pub mod math;
pub mod mesh;
pub mod scene;
pub mod shadows;
pub mod snapshot;
pub mod transform;
pub mod visual;

pub use bounds::{Aabb, AabbError, Frustum, Plane};
pub use camera::{Camera, CameraError, Projection};
pub use material::{MaterialHandle, MaterialInfo};
pub use math::{
    from_engine, normal_matrix, normalize_direction, perspective_rh_wgpu, to_engine,
    transform_normal, Mat4, Quat, Vec2, Vec3, Vec4,
};
pub use mesh::{MeshHandle, Vertex};
pub use scene::{RenderCamera, RenderLight, RenderLightKind, RenderNode, RenderScene};
pub use snapshot::{RenderSnapshot, SnapshotId, SnapshotLight, SnapshotNode};
pub use transform::Transform;
pub use visual::{
    CullingConfig, LodLevel, PreparedDraw, PreparedScene, SceneUpdate, SnapshotScene,
    VisibilityClass, VisualAssets, VisualError,
};

pub use demo::{
    demo_camera, demo_scene, grid_mesh, terrain_height, GRID_SUBDIVISIONS, TERRAIN_EXTENT,
    TERRAIN_HEIGHT,
};
pub use geometry::{RasterMesh, RasterVertex};

pub use lighting::{
    material_fixture, shade_surface, LightingEnvironment, LightingMode, MAX_POINT_LIGHTS,
};

pub use shadows::{shadow_fixture, ShadowConfig, ShadowConfigError, ShadowResolution};

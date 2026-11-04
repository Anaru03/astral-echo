pub mod bounds;
pub mod camera;
pub mod demo;
pub mod geometry;
pub mod hud;
pub mod interpolation;
pub mod lighting;
pub mod material;
pub mod math;
pub mod mesh;
pub mod model_loader;
pub mod scene;
pub mod shadows;
pub mod snapshot;
pub mod transform;
pub mod visual;

pub use bounds::{Aabb, AabbError, Frustum, Plane};
pub use camera::{Camera, CameraError, Projection};
pub use interpolation::{InterpolationConfig, SnapshotBuffer};
pub use material::{MaterialHandle, MaterialInfo};
pub use math::{
    from_engine, normal_matrix, normalize_direction, perspective_rh_wgpu, to_engine,
    transform_normal, Mat4, Quat, Vec2, Vec3, Vec4,
};
pub use mesh::{MeshHandle, Vertex};
pub use scene::{RenderCamera, RenderLight, RenderLightKind, RenderNode, RenderScene};
pub use snapshot::{
    game_over_layout, HudAutopilotState, HudContextualAction, HudControlsPanel, HudFlightState, HudMissionObjective, HudObjectiveState,
    HudMissionState, HudState, RenderOverlay, RenderSnapshot,
    RenderVisualEvent, RenderVisualEventKind, SnapshotId, SnapshotLight, SnapshotNode,
    GAME_OVER_GLYPHS,
};
pub use transform::Transform;
pub use visual::{
    CullingConfig, LodLevel, PreparedDraw, PreparedScene, PreparedSceneStats, SceneUpdate,
    SnapshotScene, VisibilityClass, VisualAssets, VisualError,
};

pub use demo::{
    demo_camera, demo_scene, grid_mesh, terrain_height, GRID_SUBDIVISIONS, TERRAIN_EXTENT,
    TERRAIN_HEIGHT,
};
pub use geometry::{RasterMesh, RasterVertex};
pub use hud::{hud_contextual_button_hit, hud_controls_button_hit, hud_controls_panel_hit, hud_overlay, HudOverlay, HudOverlayRect};
pub use model_loader::{load_obj, load_obj_reader, ObjLoadError};

pub use lighting::{
    material_fixture, shade_surface, shade_surface_profiled, shade_surface_profiled_sanitized,
    shade_surface_profiled_sanitized_normalized, shade_surface_sanitized,
    shade_surface_sanitized_normalized, LightingEnvironment, LightingMode, ShadeTimings,
    MAX_POINT_LIGHTS,
};

pub use shadows::{shadow_fixture, ShadowConfig, ShadowConfigError, ShadowResolution};
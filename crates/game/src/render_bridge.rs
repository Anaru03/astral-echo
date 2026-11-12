use std::collections::BTreeSet;

use gameplay::domains::{mission::MissionState, objective::ObjectiveState};
use gameplay::types::MissionId;
use gameplay::GameplayWorld;
use render_core::{
    from_engine, Camera, CameraError, HudAutopilotState, HudFlightState, HudMissionObjective,
    HudMissionState, HudObjectiveState, HudState, MaterialHandle, MeshHandle, Projection, Quat,
    RenderSnapshot, RenderVisualEvent, RenderVisualEventKind, SnapshotLight, SnapshotNode,
    Transform, Vec3, VisualAssets, RenderLightKind,
};

pub const SUN_ENTITY: engine::EntityId = engine::EntityId::new(9_100);
pub const STARS_ENTITY: engine::EntityId = engine::EntityId::new(9_101);
pub const TESSARA_MOON_ENTITY: engine::EntityId = engine::EntityId::new(9_102);
pub const LANDING_INDICATOR_ENTITY: engine::EntityId = engine::EntityId::new(9_205);
pub const ATMOSPHERE_EFFECT_ENTITY: engine::EntityId = engine::EntityId::new(9_206);
pub const IMPACT_FLASH_ENTITY: engine::EntityId = engine::EntityId::new(9_207);
pub const SURFACE_OBJECT_IDS: [engine::EntityId; 4] = [
    engine::EntityId::new(9_201),
    engine::EntityId::new(9_202),
    engine::EntityId::new(9_203),
    engine::EntityId::new(9_204),
];
pub const ACADEMIC_PLANET_DESTINATION_ID: u32 = 9_002;
pub const TESSARA_DESTINATION_ID: u32 = 9_601;
pub const TESSARA_POSITION: Vec3 = Vec3::new(-49.17, 8.0, 31.17);
pub const TESSARA_ORBIT_RADIUS: f32 = 69.99556;
pub const TESSARA_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.02;
pub const TESSARA_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.05;
pub const TESSARA_MOON_ORBIT_RADIUS: f32 = 12.0;
pub const TESSARA_MOON_ORBIT_HEIGHT: f32 = 2.0;
pub const TESSARA_MOON_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.09;
pub const TESSARA_MOON_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.12;
pub const KHARVEX_DESTINATION_ID: u32 = 9_602;
pub const KHARVEX_POSITION: Vec3 = Vec3::new(-70.20, -12.0, -88.20);
pub const KHARVEX_ORBIT_RADIUS: f32 = 100.0004;
pub const KHARVEX_ORBIT_PHASE_RAD: f32 = 0.0;
pub const KHARVEX_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.014;
pub const KHARVEX_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.03;
pub const NARETH_DESTINATION_ID: u32 = 9_603;
pub const NARETH_POSITION: Vec3 = Vec3::new(105.30, 18.0, -123.30);
pub const NARETH_ORBIT_RADIUS: f32 = 150.0006;
pub const NARETH_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.005;
pub const NARETH_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.018;
// Deliberately stylized space scale. Real astronomical proportions would make
// ships invisible, but the hierarchy still needs to read clearly at a glance:
// Echo-7 < rocky planets < gas giant < sun.
pub const NARETH_VISUAL_RADIUS: f32 = 18.0;
const NARETH_PLANET_SCALE: f32 = NARETH_VISUAL_RADIUS / 3.06;
pub const PLANET_ORBIT_RADIUS: f32 = 42.0;
pub const PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.04;
pub const PLANET_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.08;
pub const PLANET_SURFACE_RADIUS: f32 = 11.0;
pub const PLANET_VISUAL_RADIUS: f32 = 11.0;
pub const TESSARA_VISUAL_RADIUS: f32 = 13.0;
pub const BUILDING_PLANET_MESH: MeshHandle = MeshHandle::BUILDING_PLANET;
const BUILDING_PLANET_CENTER: Vec3 = Vec3::new(0.022138, 0.575, -0.0159805);
const BUILDING_PLANET_MAX_DIMENSION: f32 = 15.45;
const BUILDING_PLANET_VISUAL_SCALE: f32 =
    (PLANET_VISUAL_RADIUS * 2.0) / BUILDING_PLANET_MAX_DIMENSION;
pub const TESSARA_PLANET_MESH: MeshHandle = MeshHandle::TESSARA_PLANET;
const TESSARA_PLANET_CENTER: Vec3 = Vec3::new(0.0, -0.95, 0.0);
const TESSARA_PLANET_MAX_DIMENSION: f32 = 14.0043;
const TESSARA_PLANET_VISUAL_SCALE: f32 =
    (TESSARA_VISUAL_RADIUS * 2.0) / TESSARA_PLANET_MAX_DIMENSION;
// Bounds measured directly from clock_planet.obj. The clock is a thin,
// mechanical disc rather than a spherical planet.
const TESSARA_MODEL_TOP_Y: f32 = 5.724097;
pub const TESSARA_MOON_MESH: MeshHandle = MeshHandle::MOON;
const TESSARA_MOON_CENTER: Vec3 = Vec3::new(-0.001604, 0.011636, -0.0036585);
const TESSARA_MOON_VISUAL_SCALE: f32 = 1.0;
pub const KHARVEX_PLANET_MESH: MeshHandle = MeshHandle::KHARVEX_PLANET;
pub const NARETH_PLANET_MESH: MeshHandle = MeshHandle::NARETH_PLANET;
const KHARVEX_PLANET_CENTER: Vec3 = Vec3::new(12.189077, 15.664048, 6.280951);
pub const KHARVEX_VISUAL_RADIUS: f32 = 15.0;
pub const KHARVEX_SURFACE_RADIUS: f32 = KHARVEX_VISUAL_RADIUS;
const KHARVEX_PLANET_MAX_RADIUS: f32 = 126.071908;
const KHARVEX_PLANET_VISUAL_SCALE: f32 = KHARVEX_SURFACE_RADIUS / KHARVEX_PLANET_MAX_RADIUS;
pub const SUN_VISUAL_RADIUS: f32 = 14.0;
const SUN_MODEL_CENTER: Vec3 = Vec3::new(-0.2698, 0.0864, -0.1784);
const SUN_MODEL_MAX_DIMENSION: f32 = 7.5163;
const SUN_MODEL_VISUAL_SCALE: f32 = (SUN_VISUAL_RADIUS * 2.0) / SUN_MODEL_MAX_DIMENSION;
pub const SURFACE_OBJECT_RADIAL_OFFSET: f32 = PLANET_VISUAL_RADIUS + 0.4;
pub const ACADEMIC_SURFACE_OBJECT_SEED: u64 = 0xA57A_1E5D;
pub const MAX_ACADEMIC_SURFACE_OBJECTS: usize = 64;
pub const PLANET_OBSERVATION_RADIUS: f32 = 14.0;
pub const OBSERVATION_TRANSITION_DURATION_MS: u64 = 1_000;
pub const LANDING_CINEMATIC_DURATION_MS: u64 = 1_800;
pub const TAKEOFF_CINEMATIC_DURATION_MS: u64 = 1_500;
pub const SURFACE_RETURN_DISTANCE: f32 = 8.5;
const SUN_POSITION: Vec3 = Vec3::new(0.0, 0.0, -18.0);
const FLIGHT_CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 28.0, 46.0);
pub const PILOT_CAMERA_DEFAULT_DISTANCE: f32 = 8.0;
pub const PILOT_CAMERA_MIN_DISTANCE: f32 = 3.0;
pub const PILOT_CAMERA_MAX_DISTANCE: f32 = 36.0;
pub const SHIP_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ENEMY_MESH: MeshHandle = MeshHandle::ENEMY_PLACEHOLDER;
pub const PLAYER_MESH: MeshHandle = MeshHandle::CUBE;
pub const ERICK_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const PLACEHOLDER_MESH: MeshHandle = MeshHandle::CUBE;
pub const DESTINATION_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const PLANET_MESH: MeshHandle = MeshHandle::PLANET_SPHERE;
pub const SUN_MESH: MeshHandle = MeshHandle::SUN_SPHERE;
pub const SUN_MODEL_MESH: MeshHandle = MeshHandle::SUN_MODEL;
pub const STARS_MESH: MeshHandle = MeshHandle::STARS;
pub const COLLECTIBLE_MESH: MeshHandle = MeshHandle::COLLECTIBLE_PLACEHOLDER;
const STAR_SHELL_SCALE: f32 = 6.0;
pub const STATION_MESH: MeshHandle = MeshHandle::STATION;
pub const DERELICT_MESH: MeshHandle = MeshHandle::DERELICT;
pub const ASTEROID_FIELD_MESH: MeshHandle = MeshHandle::ASTEROID_FIELD;
pub const ECHO7_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ECHO7_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const STARS_MATERIAL: MaterialHandle = VisualAssets::STARS;
const ECHO7_VISUAL_SCALE_FACTOR: f32 = 1.0;
pub const ECHO7_VISUAL_SCALE: Vec3 = Vec3::splat(ECHO7_VISUAL_SCALE_FACTOR);
pub const ECHO7_VISUAL_LENGTH: f32 = 93.4 * ECHO7_VISUAL_SCALE_FACTOR;
// Bounds measured from ECHO7_Exterior_DosCubiertas.obj. The source model uses
// X for its longitudinal axis and Z as up.
const ECHO7_MODEL_CENTER: Vec3 = Vec3::new(3.7, 0.0, 0.0);
const ECHO7_MODEL_AFT_EDGE_X: f32 = 50.4;
const ECHO7_AFT_EDGE_OFFSET: f32 =
    (ECHO7_MODEL_AFT_EDGE_X - ECHO7_MODEL_CENTER.x) * ECHO7_VISUAL_SCALE.x;
// The OBJ faces -X, while gameplay considers yaw=0 forward along -Z.
// Rotate the visual model by -90° so the ship, controls and solar system use
// the same forward direction.
const ECHO7_MODEL_YAW_OFFSET_RAD: f32 = -core::f32::consts::FRAC_PI_2;
// Aim at the rear third of the hull. Aiming at the bow made the camera look
// through the entire ship even though its position was already behind the
// engines.
const ECHO7_CAMERA_TARGET_AFT_OFFSET: f32 = 2.0;

fn echo7_local_to_world_rotation() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}
pub const STATION_VISUAL_SCALE: Vec3 = Vec3::splat(0.35);
pub const ENEMY_SHIP_VISUAL_SCALE: Vec3 = Vec3::splat(0.5);
pub const PLAYER_VISUAL_SCALE: Vec3 = Vec3::splat(0.5);
// The player mesh is 2.4 units high and is rendered at 0.5 scale. Its pivot
// is in the middle, so the feet are 0.6 units below the gameplay position.
pub const PLAYER_SURFACE_CLEARANCE: f32 = 0.6;
pub const ECHO7_INTERIOR_HALF_WIDTH: f32 = 9.0;
pub const ECHO7_INTERIOR_HALF_DEPTH: f32 = 7.0;
pub const ECHO7_INTERIOR_FLOOR_Y: f32 = -1.2;
pub const ECHO7_INTERIOR_CEILING_Y: f32 = 6.4;
pub const ECHO7_INTERIOR_EYE_HEIGHT: f32 = 1.65;
const ECHO7_LOWER_DECK_Y: f32 = 0.0;
const ECHO7_UPPER_DECK_Y: f32 = 4.2 * ECHO7_VISUAL_SCALE.z;
const ECHO7_STAIR_LOCAL_X_MIN: f32 = -4.2;
const ECHO7_STAIR_LOCAL_X_MAX: f32 = 3.0;
const ECHO7_STAIR_LOCAL_Y_MIN: f32 = -7.8;
const ECHO7_STAIR_LOCAL_Y_MAX: f32 = -6.2;
const ECHO7_STAIR_LOCAL_Z_MAX: f32 = 4.2;
const ECHO7_BALCONY_LOCAL_INNER_RADIUS: f32 = 8.0;
const ECHO7_BALCONY_LOCAL_OUTER_RADIUS: f32 = 11.0;
const ECHO7_BALCONY_LOCAL_CONNECTION_X_MIN: f32 = -4.65;
const ECHO7_BALCONY_LOCAL_CONNECTION_X_MAX: f32 = 3.45;
const ECHO7_BALCONY_LOCAL_CONNECTION_Y_MIN: f32 = -8.5;
const ECHO7_BALCONY_LOCAL_CONNECTION_Y_MAX: f32 = -6.0;
pub const ECHO7_PLAYER_RADIUS: f32 = 0.45;
pub const INTERIOR_FLOOR_ENTITY: engine::EntityId = engine::EntityId::new(9_401);
pub const INTERIOR_BACK_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_402);
pub const INTERIOR_LEFT_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_403);
pub const INTERIOR_RIGHT_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_404);
pub const INTERIOR_CEILING_ENTITY: engine::EntityId = engine::EntityId::new(9_405);
pub const INTERIOR_LIGHT_ENTITY: engine::EntityId = engine::EntityId::new(9_406);
pub const INTERIOR_LOWER_ENTITY: engine::EntityId = engine::EntityId::new(9_407);
pub const INTERIOR_UPPER_ENTITY: engine::EntityId = engine::EntityId::new(9_408);
pub const INTERIOR_CORE_ENTITY: engine::EntityId = engine::EntityId::new(9_409);
pub const EXTERIOR_STATION_IDS: [engine::EntityId; 4] = [
    engine::EntityId::new(9_500),
    engine::EntityId::new(9_501),
    engine::EntityId::new(9_502),
    engine::EntityId::new(9_503),
];
const EXTERIOR_STATION_POSITIONS: [Vec3; 4] = [
    Vec3::new(420.0, 80.0, 420.0),
    Vec3::new(-380.0, -60.0, 300.0),
    Vec3::new(360.0, 120.0, -260.0),
    Vec3::new(-420.0, 60.0, -360.0),
];
const AMBIENT_DERELICT_IDS: [u32; 4] = [9_600, 9_601, 9_602, 9_603];
const AMBIENT_DERELICT_POSITIONS: [Vec3; 4] = [
    Vec3::new(300.0, 36.0, 360.0),
    Vec3::new(-330.0, -42.0, 260.0),
    Vec3::new(280.0, 72.0, -300.0),
    Vec3::new(-360.0, 48.0, -280.0),
];

pub const SHIP_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ENEMY_MATERIAL: MaterialHandle = VisualAssets::ENEMY;
pub const CHARGER_MATERIAL: MaterialHandle = VisualAssets::ENEMY_CHARGER;
pub const SHIELDED_MATERIAL: MaterialHandle = VisualAssets::ENEMY_SHIELDED;
pub const PLAYER_MATERIAL: MaterialHandle = VisualAssets::PLAYER;
pub const ERICK_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLACEHOLDER_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const DESTINATION_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLANET_MATERIAL: MaterialHandle = VisualAssets::PLANET;
pub const SUN_MATERIAL: MaterialHandle = VisualAssets::SUN;
pub const COLLECTIBLE_MATERIAL: MaterialHandle = VisualAssets::COLLECTIBLE;
pub const STATION_MATERIAL: MaterialHandle = VisualAssets::STATION;
pub const DERELICT_MATERIAL: MaterialHandle = VisualAssets::DERELICT;
pub const ASTEROID_MATERIAL: MaterialHandle = VisualAssets::ASTEROID;

pub(crate) fn echo7_world_to_local(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> Vec3 {
    let rotation =
        Quat::from_rotation_y(ship_yaw_degrees.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD);
    let model_local = echo7_local_to_world_rotation().inverse()
        * rotation.inverse()
        * (from_engine(engine::Vec3 {
            x: position.x - ship_position.x,
            y: position.y - ship_position.y,
            z: position.z - ship_position.z,
        }) + rotation
            * (echo7_local_to_world_rotation() * (ECHO7_VISUAL_SCALE * ECHO7_MODEL_CENTER)));
    model_local / ECHO7_VISUAL_SCALE
}

pub fn echo7_player_model_position(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> Vec3 {
    echo7_world_to_local(position, ship_position, ship_yaw_degrees)
}

pub fn echo7_model_to_world(
    local: Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> engine::Vec3 {
    let rotation =
        Quat::from_rotation_y(ship_yaw_degrees.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD);
    let position = from_engine(ship_position)
        + rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)));
    engine::Vec3 {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

pub const ECHO7_PILOT_CONTROL_LOCAL: Vec3 = Vec3::new(-38.09, -2.4, 5.53);
pub const ECHO7_PILOT_INTERACTION_LOCAL: Vec3 = Vec3::new(-37.5, -2.4, 5.1);
pub const ECHO7_PILOT_SEATED_LOCAL: Vec3 = Vec3::new(-35.5, -2.4, 5.8);
pub const ECHO7_PILOT_EXIT_LOCAL: Vec3 = Vec3::new(-34.2, 0.0, 4.2);
pub const ECHO7_DESPERTAR_SPAWN_LOCAL: Vec3 = Vec3::new(-14.0, -17.5, 0.85);
const ECHO7_DESPERTAR_ROOM_X: (f32, f32) = (-24.0, -12.0);
const ECHO7_DESPERTAR_ROOM_Y: (f32, f32) = (-20.0, -10.0);

#[derive(Clone, Copy, Debug)]
pub struct Echo7RoomDoor {
    pub name: &'static str,
    pub center: Vec3,
    pub control: Vec3,
    pub upper: bool,
}

pub const ECHO7_ROOM_DOORS: [Echo7RoomDoor; 16] = [
    Echo7RoomDoor {
        name: "Cabina",
        center: Vec3::new(-27.4, 0.0, 4.2),
        control: Vec3::new(-27.6, 1.78, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Control auxiliar",
        center: Vec3::new(-27.4, 0.0, 0.0),
        control: Vec3::new(-27.6, 1.78, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Carga",
        center: Vec3::new(29.0, 0.0, 0.0),
        control: Vec3::new(28.85, 1.78, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Esclusa intermedia",
        center: Vec3::new(32.3, -4.1, 0.0),
        control: Vec3::new(33.88, -4.3, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Clonacion",
        center: Vec3::new(-20.0, 11.0, 0.0),
        control: Vec3::new(-18.32, 10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Despertar",
        center: Vec3::new(-20.0, -11.0, 0.0),
        control: Vec3::new(-21.68, -10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Enfermeria",
        center: Vec3::new(0.0, 11.0, 0.0),
        control: Vec3::new(1.68, 10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Archivo biologico",
        center: Vec3::new(0.0, -11.0, 0.0),
        control: Vec3::new(-1.68, -10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Ingenieria",
        center: Vec3::new(20.0, 11.0, 0.0),
        control: Vec3::new(21.68, 10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Almacen",
        center: Vec3::new(20.0, -11.0, 0.0),
        control: Vec3::new(18.32, -10.84, 1.35),
        upper: false,
    },
    Echo7RoomDoor {
        name: "Observatorio",
        center: Vec3::new(-20.0, 11.0, 4.2),
        control: Vec3::new(-18.32, 10.84, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Huerto",
        center: Vec3::new(-20.0, -11.0, 4.2),
        control: Vec3::new(-21.68, -10.84, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Sala comun",
        center: Vec3::new(0.0, 11.0, 4.2),
        control: Vec3::new(1.68, 10.84, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Dormitorios",
        center: Vec3::new(0.0, -11.0, 4.2),
        control: Vec3::new(-1.68, -10.84, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Control navegacion",
        center: Vec3::new(20.0, 11.0, 4.2),
        control: Vec3::new(21.68, 10.84, 5.55),
        upper: true,
    },
    Echo7RoomDoor {
        name: "Cocina",
        center: Vec3::new(20.0, -11.0, 4.2),
        control: Vec3::new(18.32, -10.84, 5.55),
        upper: true,
    },
];

fn echo7_upper_corridor_or_cabin(local: Vec3) -> bool {
    echo7_model_room_floor_contains(local, true)
}

fn echo7_model_rect_contains(local: Vec3, bounds: (f32, f32, f32, f32)) -> bool {
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    local.x >= bounds.0 + radius
        && local.x <= bounds.1 - radius
        && local.y >= bounds.2 + radius
        && local.y <= bounds.3 - radius
}

fn echo7_model_room_floor_contains(local: Vec3, upper: bool) -> bool {
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    let module = [-20.0, 0.0, 20.0].iter().any(|center| {
        [-16.0, 16.0].iter().any(|side| {
            echo7_model_rect_contains(local, (center - 8.0, center + 8.0, side - 5.0, side + 5.0))
        })
    });
    let branch = [-20.0, 0.0, 20.0].iter().any(|center| {
        [-1.0, 1.0].iter().any(|side| {
            let (min_y, max_y) = if *side < 0.0 {
                (-11.05 + radius, -1.59 + radius)
            } else {
                (1.59 - radius, 11.05 - radius)
            };
            local.x >= center - 1.52 + radius
                && local.x <= center + 1.52 - radius
                && local.y >= min_y
                && local.y <= max_y
        })
    });
    let longitudinal = (local.x >= -27.5 + radius && local.x <= -10.8 + radius
        || local.x >= 10.8 - radius && local.x <= 29.1 - radius)
        && local.y.abs() <= 1.82 - radius;
    let central_corridor =
        local.x >= -10.8 + radius && local.x <= 10.8 - radius && local.y.abs() <= 1.82 - radius;
    // The lower corridor continues to the stair footprint. Keep this narrow
    // connector open so the modeled bulkheads do not seal off the stairwell.
    let stair_connector = local.x >= -6.0 + radius
        && local.x <= 4.5 - radius
        && local.y >= ECHO7_STAIR_LOCAL_Y_MIN - radius
        && local.y <= 1.82 - radius;
    let bow = echo7_model_rect_contains(local, (-42.3, -27.3, -4.0, 4.0));
    let lower_extra = echo7_model_rect_contains(local, (28.9, 35.9, -4.2, 4.2))
        || echo7_model_rect_contains(local, (30.4, 34.2, -10.3, -4.0));
    module
        || branch
        || longitudinal
        || central_corridor
        || stair_connector
        || bow
        || (!upper && lower_extra)
}

fn echo7_model_door_portal(local: Vec3, index: usize) -> bool {
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    let door = ECHO7_ROOM_DOORS[index];
    if index <= 2 {
        (local.x - door.center.x).abs() <= radius && local.y.abs() <= 1.63 - radius
    } else if index == 3 {
        (local.y - door.center.y).abs() <= radius
            && (local.x - door.center.x).abs() <= 1.32 - radius
    } else {
        (local.y - door.center.y).abs() <= radius
            && (local.x - door.center.x).abs() <= 1.52 - radius
    }
}

pub fn echo7_model_floor_position_valid(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
    upper: bool,
    open_doors: &[bool; ECHO7_ROOM_DOORS.len()],
) -> bool {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    let stair_approach = local.x >= ECHO7_STAIR_LOCAL_X_MIN - ECHO7_PLAYER_RADIUS
        && local.x <= ECHO7_STAIR_LOCAL_X_MAX + ECHO7_PLAYER_RADIUS
        && local.y >= ECHO7_STAIR_LOCAL_Y_MIN - ECHO7_PLAYER_RADIUS
        && local.y <= ECHO7_STAIR_LOCAL_Y_MAX + ECHO7_PLAYER_RADIUS;
    if stair_approach {
        return true;
    }
    if !upper {
        let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
        let inside_despertar = local.x >= ECHO7_DESPERTAR_ROOM_X.0 + radius
            && local.x <= ECHO7_DESPERTAR_ROOM_X.1 - radius
            && local.y >= ECHO7_DESPERTAR_ROOM_Y.0 + radius
            && local.y <= ECHO7_DESPERTAR_ROOM_Y.1 - radius;
        let despertar_connector = local.x >= ECHO7_DESPERTAR_ROOM_X.0 + radius
            && local.x <= -ECHO7_INTERIOR_HALF_WIDTH + radius
            && local.y >= -20.0 + radius
            && local.y <= 1.82 - radius;
        if inside_despertar || despertar_connector {
            return echo7_lower_core_obstacle_free(local, position.y);
        }
    }
    if !upper
        && (local.x * local.x + local.y * local.y).sqrt()
            <= ECHO7_BALCONY_LOCAL_OUTER_RADIUS - ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x
        && !echo7_lower_core_obstacle_free(local, position.y)
    {
        return false;
    }
    if open_doors.iter().enumerate().any(|(index, open)| {
        !*open && ECHO7_ROOM_DOORS[index].upper == upper && echo7_model_door_portal(local, index)
    }) {
        return false;
    }
    if open_doors.iter().enumerate().any(|(index, open)| {
        *open && ECHO7_ROOM_DOORS[index].upper == upper && echo7_model_door_portal(local, index)
    }) {
        return true;
    }
    if echo7_model_room_floor_contains(local, upper) {
        return true;
    }
    if upper {
        echo7_upper_or_stair_surface_contains(position, ship_position, ship_yaw_degrees)
    } else {
        (local.x * local.x + local.y * local.y).sqrt()
            <= ECHO7_BALCONY_LOCAL_OUTER_RADIUS - ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x
    }
}

fn echo7_upper_landing_contains(local: Vec3) -> bool {
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    local.x >= 3.0 && local.x <= 5.2 && local.y >= -7.9 + radius && local.y <= -6.1 - radius
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Echo7SupportSurface {
    LowerDeck,
    Stair,
    UpperBalcony,
}

pub fn echo7_support_surface(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> Echo7SupportSurface {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    let stair_lateral_radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.y;
    let on_stair = local.x >= ECHO7_STAIR_LOCAL_X_MIN
        && local.x <= ECHO7_STAIR_LOCAL_X_MAX
        && local.y >= ECHO7_STAIR_LOCAL_Y_MIN + stair_lateral_radius
        && local.y <= ECHO7_STAIR_LOCAL_Y_MAX - stair_lateral_radius;
    if on_stair {
        return Echo7SupportSurface::Stair;
    }
    if echo7_upper_or_stair_surface_contains(position, ship_position, ship_yaw_degrees)
        && position.y > 1.0
    {
        return Echo7SupportSurface::UpperBalcony;
    }
    Echo7SupportSurface::LowerDeck
}

pub fn echo7_stair_path_position_valid(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> bool {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    let stair_lateral_radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.y;
    let within_lateral_clearance = local.y >= ECHO7_STAIR_LOCAL_Y_MIN + stair_lateral_radius
        && local.y <= ECHO7_STAIR_LOCAL_Y_MAX - stair_lateral_radius;
    within_lateral_clearance
        && (local.x < ECHO7_STAIR_LOCAL_X_MIN
            || local.x > ECHO7_STAIR_LOCAL_X_MAX
            || matches!(
                echo7_support_surface(position, ship_position, ship_yaw_degrees),
                Echo7SupportSurface::Stair
            ))
}

pub fn echo7_upper_or_stair_surface_contains(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> bool {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    let stair_lateral_radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.y;
    let on_stair = local.x >= ECHO7_STAIR_LOCAL_X_MIN
        && local.x <= ECHO7_STAIR_LOCAL_X_MAX
        && local.y >= ECHO7_STAIR_LOCAL_Y_MIN + stair_lateral_radius
        && local.y <= ECHO7_STAIR_LOCAL_Y_MAX - stair_lateral_radius;
    if on_stair {
        return true;
    }
    if !local.y.is_finite() || !local.x.is_finite() {
        return false;
    }
    let radius = (local.x * local.x + local.y * local.y).sqrt();
    let on_balcony_ring = radius >= ECHO7_BALCONY_LOCAL_INNER_RADIUS + ECHO7_PLAYER_RADIUS
        && radius <= ECHO7_BALCONY_LOCAL_OUTER_RADIUS - ECHO7_PLAYER_RADIUS;
    let at_stair_connection = local.x >= ECHO7_BALCONY_LOCAL_CONNECTION_X_MIN
        && local.x <= ECHO7_BALCONY_LOCAL_CONNECTION_X_MAX
        && local.y >= ECHO7_BALCONY_LOCAL_CONNECTION_Y_MIN
        && local.y <= ECHO7_BALCONY_LOCAL_CONNECTION_Y_MAX;
    on_balcony_ring
        || at_stair_connection
        || echo7_upper_landing_contains(local)
        || echo7_upper_corridor_or_cabin(local)
        || ECHO7_ROOM_DOORS
            .iter()
            .enumerate()
            .any(|(index, door)| door.upper && echo7_model_door_portal(local, index))
}

pub fn echo7_walkable_height(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
) -> f32 {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    if matches!(
        echo7_support_surface(position, ship_position, ship_yaw_degrees),
        Echo7SupportSurface::Stair
    ) {
        let progress = ((local.x - ECHO7_STAIR_LOCAL_X_MIN)
            / (ECHO7_STAIR_LOCAL_X_MAX - ECHO7_STAIR_LOCAL_X_MIN))
            .clamp(0.0, 1.0);
        return ECHO7_LOWER_DECK_Y + (ECHO7_STAIR_LOCAL_Z_MAX * ECHO7_VISUAL_SCALE.z) * progress;
    }
    if position.y > 1.0
        && echo7_upper_or_stair_surface_contains(position, ship_position, ship_yaw_degrees)
    {
        if echo7_upper_corridor_or_cabin(local)
            || echo7_upper_landing_contains(local)
            || ECHO7_ROOM_DOORS
                .iter()
                .enumerate()
                .any(|(index, door)| door.upper && echo7_model_door_portal(local, index))
        {
            return ECHO7_UPPER_DECK_Y;
        }
        if (local.x * local.x + local.y * local.y).sqrt()
            >= ECHO7_BALCONY_LOCAL_INNER_RADIUS + ECHO7_PLAYER_RADIUS
        {
            return ECHO7_UPPER_DECK_Y;
        }
        if local.x >= ECHO7_BALCONY_LOCAL_CONNECTION_X_MIN
            && local.x <= ECHO7_BALCONY_LOCAL_CONNECTION_X_MAX
            && local.y >= ECHO7_BALCONY_LOCAL_CONNECTION_Y_MIN
            && local.y <= ECHO7_BALCONY_LOCAL_CONNECTION_Y_MAX
        {
            return ECHO7_UPPER_DECK_Y;
        }
    }
    ECHO7_LOWER_DECK_Y
}

fn echo7_lower_core_obstacle_free(local: Vec3, feet_y: f32) -> bool {
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    let furniture = [
        (0.0, 4.0, 0.8, 0.4, 1.4),
        (-5.0, 2.8, 1.0, 0.5, 1.2),
        (6.0, -2.8, 0.5, 0.8, 2.4),
        (-6.0, -2.5, 0.2, 0.25, 1.0),
    ];
    let blocked_by_furniture =
        furniture
            .iter()
            .any(|(center_x, center_y, half_x, half_y, height)| {
                feet_y >= ECHO7_LOWER_DECK_Y
                    && feet_y <= *height
                    && local.x >= center_x - half_x - radius
                    && local.x <= center_x + half_x + radius
                    && local.y >= center_y - half_y - radius
                    && local.y <= center_y + half_y + radius
            });
    let tree_trunks = [
        (0.0, 0.0, 0.24),
        (5.5, 5.5, 0.24),
        (-5.5, 5.5, 0.24),
        (-5.5, -5.5, 0.24),
        (5.5, -4.0, 0.24),
    ];
    let blocked_by_tree_trunk = tree_trunks
        .iter()
        .any(|(center_x, center_y, trunk_radius)| {
            feet_y >= ECHO7_LOWER_DECK_Y
                && feet_y <= 1.8
                && (local.x - center_x).powi(2) + (local.y - center_y).powi(2)
                    <= (trunk_radius + radius).powi(2)
        });
    !blocked_by_furniture && !blocked_by_tree_trunk
}

pub fn echo7_lower_deck_position_valid(
    position: engine::Vec3,
    ship_position: engine::Vec3,
    ship_yaw_degrees: f32,
    door_position: Option<engine::Vec3>,
    door_open: bool,
) -> bool {
    let local = echo7_world_to_local(position, ship_position, ship_yaw_degrees);
    if position.y > ECHO7_INTERIOR_CEILING_Y {
        return true;
    }
    let radius = ECHO7_PLAYER_RADIUS / ECHO7_VISUAL_SCALE.x;
    let stair_exit = local.x >= ECHO7_STAIR_LOCAL_X_MIN - radius
        && local.x <= ECHO7_STAIR_LOCAL_X_MAX + radius
        && local.y >= ECHO7_STAIR_LOCAL_Y_MIN - radius
        && local.y <= ECHO7_STAIR_LOCAL_Y_MAX + radius;
    if stair_exit {
        return true;
    }
    let bounds = (
        -ECHO7_INTERIOR_HALF_WIDTH + radius,
        ECHO7_INTERIOR_HALF_WIDTH - radius,
        -ECHO7_INTERIOR_HALF_DEPTH + radius,
        ECHO7_INTERIOR_HALF_DEPTH - radius,
    );
    let door_local =
        door_position.map(|door| echo7_world_to_local(door, ship_position, ship_yaw_degrees));
    let at_open_door = door_open
        && door_local.is_some_and(|door| {
            let on_x_wall = door.x.abs() >= ECHO7_INTERIOR_HALF_WIDTH - 0.6;
            let on_y_wall = door.y.abs() >= ECHO7_INTERIOR_HALF_DEPTH - 0.6;
            (on_x_wall || on_y_wall)
                && (local.x - door.x).abs() < 1.0
                && (local.y - door.y).abs() < 1.0
        });
    if at_open_door {
        return true;
    }
    let door_gap = |coordinate: f32, other: f32, limit: f32| {
        door_open
            && door_local.is_some_and(|door| {
                (door.x.abs() >= limit - 0.6 && (door.y - other).abs() < 1.0)
                    || (door.y.abs() >= limit - 0.6 && (door.x - other).abs() < 1.0)
            })
            && coordinate.abs() >= limit - radius
    };
    let inside_x = local.x >= bounds.0 && local.x <= bounds.1;
    let inside_y = local.y >= bounds.2 && local.y <= bounds.3;
    let inside_despertar = local.x >= ECHO7_DESPERTAR_ROOM_X.0 + radius
        && local.x <= ECHO7_DESPERTAR_ROOM_X.1 - radius
        && local.y >= ECHO7_DESPERTAR_ROOM_Y.0 + radius
        && local.y <= ECHO7_DESPERTAR_ROOM_Y.1 - radius;
    let despertar_connector = local.x >= ECHO7_DESPERTAR_ROOM_X.0 + radius
        && local.x <= -ECHO7_INTERIOR_HALF_WIDTH + radius
        && local.y >= -20.0 + radius
        && local.y <= -6.0 - radius;
    if inside_despertar || despertar_connector {
        return echo7_lower_core_obstacle_free(local, position.y);
    }
    inside_x
        && inside_y
        // The rectangular hull bounds are only the outer shell. Restrict the
        // lower deck to the modeled rooms/corridors as well, otherwise the
        // player can walk through the internal bulkheads between modules.
        && echo7_model_room_floor_contains(local, false)
        && echo7_lower_core_obstacle_free(local, position.y)
        && !(local.x <= bounds.0 && !door_gap(local.x, local.y, ECHO7_INTERIOR_HALF_WIDTH))
        && !(local.x >= bounds.1 && !door_gap(local.x, local.y, ECHO7_INTERIOR_HALF_WIDTH))
        && !(local.y <= bounds.2 && !door_gap(local.y, local.x, ECHO7_INTERIOR_HALF_DEPTH))
        && !(local.y >= bounds.3 && !door_gap(local.y, local.x, ECHO7_INTERIOR_HALF_DEPTH))
}

fn enemy_material(archetype: Option<gameplay::EnemyArchetype>) -> MaterialHandle {
    match archetype {
        Some(gameplay::EnemyArchetype::Charger) => CHARGER_MATERIAL,
        Some(gameplay::EnemyArchetype::Shielded) => SHIELDED_MATERIAL,
        Some(gameplay::EnemyArchetype::Shooter) | None => ENEMY_MATERIAL,
    }
}

fn echo7_visual_rotation(orientation: gameplay::ShipOrientation) -> Quat {
    let yaw = orientation.yaw_degrees.to_radians();
    let pitch = orientation.pitch_degrees.to_radians().clamp(-1.2, 1.2);
    // Rotation order matters: first convert the Z-up OBJ to Y-up, then align
    // its -X bow with gameplay -Z, and finally apply the ship controls.
    Quat::from_rotation_y(yaw)
        * Quat::from_rotation_x(pitch)
        * Quat::from_rotation_y(ECHO7_MODEL_YAW_OFFSET_RAD)
        * echo7_local_to_world_rotation()
}

pub fn station_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    station_nodes_for_view(world, false)
}

fn station_nodes_for_view(world: &GameplayWorld, interior: bool) -> Vec<SnapshotNode> {
    let mut nodes = Vec::new();
    let mut seen = BTreeSet::new();
    for interactable in world.interactables() {
        if !matches!(
            interactable.behavior,
            gameplay::InteractionBehavior::Station { .. }
        ) || !seen.insert(interactable.entity_id)
        {
            continue;
        }
        if !interior && is_echo_interior_position(world, interactable.position) {
            continue;
        }
        nodes.push(SnapshotNode::new(
            interactable.entity_id,
            Transform::new(
                from_engine(interactable.position),
                Quat::IDENTITY,
                if interior {
                    Vec3::splat(0.55)
                } else {
                    STATION_VISUAL_SCALE
                },
            ),
            if interior {
                PLACEHOLDER_MESH
            } else {
                STATION_MESH
            },
            if interior {
                DESTINATION_MATERIAL
            } else {
                STATION_MATERIAL
            },
            interactable.enabled,
        ));
    }
    if !interior {
        nodes.extend(
            EXTERIOR_STATION_IDS
                .into_iter()
                .zip(EXTERIOR_STATION_POSITIONS)
                .map(|(entity, position)| {
                    SnapshotNode::new(
                        entity,
                        Transform::new(position, Quat::IDENTITY, STATION_VISUAL_SCALE),
                        STATION_MESH,
                        STATION_MATERIAL,
                        true,
                    )
                }),
        );
    }
    nodes
}

fn mission_interaction_nodes(
    world: &GameplayWorld,
    interior: bool,
) -> impl Iterator<Item = SnapshotNode> + '_ {
    world
        .interactables()
        .filter(move |interactable| {
            interior || !is_echo_interior_position(world, interactable.position)
        })
        .filter(move |_| !(interior && ECHO7_MESH != PLACEHOLDER_MESH))
        .filter(|interactable| {
            matches!(
                interactable.behavior,
                gameplay::InteractionBehavior::Console { .. }
                    | gameplay::InteractionBehavior::ElectricalPanel
                    | gameplay::InteractionBehavior::LifeSupportPanel
                    | gameplay::InteractionBehavior::Door { .. }
            )
        })
        .map(|interactable| {
            SnapshotNode::new(
                interactable.entity_id,
                Transform::from_translation(from_engine(interactable.position)),
                PLACEHOLDER_MESH,
                PLACEHOLDER_MATERIAL,
                interactable.enabled,
            )
        })
}

fn echo7_guidance_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    let Some(player) = world.players().find(|value| value.is_active) else {
        return Vec::new();
    };
    let target = world
        .interactables()
        .filter_map(|value| {
            let priority = match value.behavior {
                gameplay::InteractionBehavior::ElectricalPanel => world
                    .electrical_panel(value.entity_id)
                    .is_some_and(|panel| !panel.is_repaired())
                    .then_some(1),
                gameplay::InteractionBehavior::LifeSupportPanel => world
                    .life_support_panel(value.entity_id)
                    .is_some_and(|panel| !panel.progress().repaired)
                    .then_some(1),
                gameplay::InteractionBehavior::Console {
                    state,
                    role: gameplay::ConsoleRole::ShipStatus,
                } => matches!(state, gameplay::ConsoleState::Off).then_some(0),
                gameplay::InteractionBehavior::Door { door }
                    if door.role == gameplay::DoorRole::CommandBridge => {
                        (!matches!(
                            door.state,
                            gameplay::domains::door::DoorState::Open
                        ))
                        .then_some(2)
                    }
                gameplay::InteractionBehavior::Station { station }
                    if station.station_type == gameplay::StationType::Piloting => {
                        station.occupant.is_none().then_some(3)
                    }
                _ => None,
            };
            let Some(priority) = priority else {
                return None;
            };
            if !value.enabled {
                return None;
            }
            let dx = value.position.x - player.position.x;
            let dz = value.position.z - player.position.z;
            Some((priority, dx * dx + dz * dz, value.position))
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| {
                    left.1
                    .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|(_, _, position)| position);
    let Some(target) = target else {
        return Vec::new();
    };
    let distance = ((target.x - player.position.x).powi(2)
        + (target.z - player.position.z).powi(2))
    .sqrt();
    let count = (distance / 1.8).ceil().clamp(1.0, 12.0) as usize;
    (1..=count)
        .map(|index| {
            let fraction = index as f32 / count as f32;
            let position = Vec3::new(
                player.position.x + (target.x - player.position.x) * fraction,
                player.position.y + 0.22,
                player.position.z + (target.z - player.position.z) * fraction,
            );
            let scale = if index == count {
                Vec3::splat(0.72)
            } else {
                Vec3::splat(0.32)
            };
            SnapshotNode::new(
                engine::EntityId::new(9_700 + index as u64),
                Transform::new(position, Quat::IDENTITY, scale),
                COLLECTIBLE_MESH,
                COLLECTIBLE_MATERIAL,
                true,
            )
        })
        .collect()
}

fn echo7_interaction_sign_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    let mut signs = Vec::new();
    let mut next_id = 9_801;
    for interactable in world.interactables() {
        let is_minigame_point = matches!(
            interactable.behavior,
            gameplay::InteractionBehavior::ElectricalPanel
                | gameplay::InteractionBehavior::LifeSupportPanel
                | gameplay::InteractionBehavior::Console {
                    role: gameplay::ConsoleRole::ShipStatus,
                    ..
                }
                | gameplay::InteractionBehavior::Station {
                    station: gameplay::Station {
                        station_type: gameplay::StationType::Piloting,
                        ..
                    }
                }
        );
        if is_minigame_point && interactable.enabled {
            signs.push(SnapshotNode::new(
                engine::EntityId::new(next_id),
                Transform::new(
                    from_engine(interactable.position) + Vec3::new(0.0, 0.28, 0.0),
                    Quat::IDENTITY,
                    Vec3::splat(0.62),
                ),
                COLLECTIBLE_MESH,
                COLLECTIBLE_MATERIAL,
                true,
            ));
            next_id += 1;
        }
    }
    signs
}

fn is_echo_interior_position(world: &GameplayWorld, position: engine::Vec3) -> bool {
    let origin = world
        .destination(gameplay::DestinationId::new(9_001))
        .map(|destination| destination.position)
        .or_else(|| world.ship_position());
    let Some(origin) = origin else {
        return false;
    };
    if origin == engine::Vec3::ZERO {
        return false;
    }
    let dx = position.x - origin.x;
    let dz = position.z - origin.z;
    dx.abs() <= ECHO7_INTERIOR_HALF_WIDTH
        && dz.abs() <= ECHO7_INTERIOR_HALF_DEPTH
        && position.y >= ECHO7_INTERIOR_FLOOR_Y
        && position.y <= ECHO7_INTERIOR_CEILING_Y
}

pub fn derelict_nodes(world: &GameplayWorld) -> impl Iterator<Item = SnapshotNode> + '_ {
    world
        .derelicts()
        .filter(|derelict| derelict.state != gameplay::DerelictState::Undiscovered)
        .map(|derelict| {
            SnapshotNode::derelict(
                derelict.id.value(),
                Transform::from_translation(from_engine(derelict.position)),
                DERELICT_MESH,
                DERELICT_MATERIAL,
                true,
            )
        })
}

fn ambient_derelict_nodes() -> [SnapshotNode; 4] {
    std::array::from_fn(|index| {
        let id = AMBIENT_DERELICT_IDS[index];
        SnapshotNode::derelict(
            id,
            Transform::new(
                AMBIENT_DERELICT_POSITIONS[index],
                Quat::from_rotation_y((id as f32) * 0.37),
                Vec3::splat(0.5),
            ),
            DERELICT_MESH,
            DERELICT_MATERIAL,
            true,
        )
    })
}

pub fn asteroid_field_nodes(world: &GameplayWorld) -> impl Iterator<Item = SnapshotNode> + '_ {
    world
        .active_environmental_hazards()
        .filter(|hazard| hazard.hazard_type == gameplay::EnvironmentalHazardType::DebrisField)
        .map(|hazard| {
            SnapshotNode::hazard(
                hazard.id.value(),
                Transform::from_translation(from_engine(hazard.position)),
                ASTEROID_FIELD_MESH,
                ASTEROID_MATERIAL,
                true,
            )
        })
}

pub fn academic_surface_objects(elapsed_seconds: f64) -> [SnapshotNode; 4] {
    let objects = academic_surface_objects_with_seed(
        elapsed_seconds,
        ACADEMIC_SURFACE_OBJECT_SEED,
        SURFACE_OBJECT_IDS.len(),
    );
    [objects[0], objects[1], objects[2], objects[3]]
}

pub fn academic_surface_objects_with_seed(
    elapsed_seconds: f64,
    seed: u64,
    count: usize,
) -> Vec<SnapshotNode> {
    let planet = academic_planet_transform(elapsed_seconds);
    let mut rng = SurfaceDistribution::new(seed);
    let mut objects = Vec::with_capacity(count.min(MAX_ACADEMIC_SURFACE_OBJECTS));
    for index in 0..count.min(MAX_ACADEMIC_SURFACE_OBJECTS) {
        let direction = rng.unit_direction();
        let spin = rng.next_f32() * core::f32::consts::TAU;
        let scale = Vec3::new(
            0.08 + rng.next_f32() * 0.08,
            0.1 + rng.next_f32() * 0.12,
            0.08 + rng.next_f32() * 0.08,
        );
        let (mesh, material) = match index % 4 {
            0 => (PLAYER_MESH, PLAYER_MATERIAL),
            1 => (PLACEHOLDER_MESH, PLACEHOLDER_MATERIAL),
            2 => (DESTINATION_MESH, DESTINATION_MATERIAL),
            _ => (COLLECTIBLE_MESH, COLLECTIBLE_MATERIAL),
        };
        let id = SURFACE_OBJECT_IDS
            .get(index)
            .copied()
            .unwrap_or_else(|| engine::EntityId::new(9_300 + index as u64));
        objects.push(SnapshotNode::new(
            id,
            planet.composed(Transform::new(
                direction * SURFACE_OBJECT_RADIAL_OFFSET,
                surface_object_orientation(direction, spin),
                scale,
            )),
            mesh,
            material,
            true,
        ));
    }
    objects
}

struct SurfaceDistribution {
    state: u64,
}

impl SurfaceDistribution {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(2_685_821_657_736_338_717)
    }

    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) - 1) as f32
    }

    fn unit_direction(&mut self) -> Vec3 {
        let y = self.next_f32() * 2.0 - 1.0;
        let longitude = self.next_f32() * core::f32::consts::TAU;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        Vec3::new(ring * longitude.cos(), y, ring * longitude.sin()).normalize()
    }
}

fn surface_object_orientation(direction: Vec3, spin: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, direction) * Quat::from_rotation_y(spin)
}

#[derive(Clone, Copy, Debug)]
pub struct BridgeParams<'a> {
    pub destinations: &'a [gameplay::DestinationId],
    pub include_collectibles: bool,
}

pub fn presentation_mission(world: &GameplayWorld) -> Option<MissionId> {
    crate::campaign::CAMPAIGN_ORDER
        .iter()
        .copied()
        .find(|mission| {
            world
                .mission(*mission)
                .is_some_and(|value| value.state() == MissionState::Active)
        })
        .or_else(|| {
            crate::campaign::CAMPAIGN_ORDER
                .iter()
                .copied()
                .find(|mission| {
                    world
                        .mission(*mission)
                        .is_some_and(|value| value.state() == MissionState::Available)
                })
        })
        .or_else(|| {
            crate::campaign::CAMPAIGN_ORDER
                .iter()
                .copied()
                .find(|mission| world.mission(*mission).is_some())
        })
}

impl Default for BridgeParams<'_> {
    fn default() -> Self {
        Self {
            destinations: &[],
            include_collectibles: true,
        }
    }
}

pub fn snapshot_from_world(world: &GameplayWorld, params: BridgeParams<'_>) -> RenderSnapshot {
    snapshot_from_world_with_camera(world, params, &Camera::demo())
}

pub fn snapshot_from_world_with_camera(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
) -> RenderSnapshot {
    snapshot_from_world_with_camera_and_view(world, params, camera, false)
}

pub fn snapshot_from_world_with_camera_and_view(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
) -> RenderSnapshot {
    snapshot_from_world_with_camera_and_view_and_nareth(world, params, camera, interior, false)
}

pub fn snapshot_from_world_with_camera_and_view_and_nareth(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
    nareth_mesh_ready: bool,
) -> RenderSnapshot {
    snapshot_from_world_with_camera_and_view_and_nareth_and_pilot(
        world,
        params,
        camera,
        interior,
        nareth_mesh_ready,
        false,
    )
}

pub fn snapshot_from_world_with_camera_and_view_and_nareth_and_pilot(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
    nareth_mesh_ready: bool,
    pilot_mode: bool,
) -> RenderSnapshot {
    snapshot_from_world_with_camera_and_view_and_nareth_and_pilot_and_surface(
        world,
        params,
        camera,
        interior,
        nareth_mesh_ready,
        pilot_mode,
        None,
    )
}

pub fn snapshot_from_world_with_camera_and_view_and_nareth_and_pilot_and_surface(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
    nareth_mesh_ready: bool,
    pilot_mode: bool,
    surface_planet: Option<gameplay::DestinationId>,
) -> RenderSnapshot {
    snapshot_from_world_with_camera_and_view_and_nareth_and_pilot_and_surface_transform(
        world,
        params,
        camera,
        interior,
        nareth_mesh_ready,
        pilot_mode,
        surface_planet,
        None,
    )
}

pub fn snapshot_from_world_with_camera_and_view_and_nareth_and_pilot_and_surface_transform(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
    nareth_mesh_ready: bool,
    pilot_mode: bool,
    surface_planet: Option<gameplay::DestinationId>,
    surface_transform: Option<Transform>,
) -> RenderSnapshot {
    let tick = world.time().tick;
    let academic_planet = academic_planet_id(world);
    let tessara_planet = tessara_planet_id(world);
    let kharvex_planet = kharvex_planet_id(world);
    let nareth_planet = nareth_planet_id(world);
    let elapsed_seconds = world.time().elapsed.0 as f64 / 1_000.0;
    let landing_planet = match world.planet_flight_state() {
        // Navigation reaches `Landed` on the first simulation tick, while
        // the visual descent keeps running for the rest of the cinematic.
        // Both states must therefore retain the lightweight landing scene.
        gameplay::PlanetFlightState::Landing { planet }
        | gameplay::PlanetFlightState::Landed { planet } => Some(planet),
        _ => None,
    };
    // Cinematics and surface exploration keep a small deterministic scene
    // budget, while the selected planet remains present as the gravity and
    // visual reference for the player.
    let limited_scene = surface_planet.is_some() || landing_planet.is_some();
    let mut snapshot = RenderSnapshot::new(
        tick,
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );
    snapshot.simulation_time = world.time().elapsed;
    snapshot
        .nodes
        .reserve(1 + expected_snapshot_node_count(world, params, academic_planet, interior));
    let star_center = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    snapshot.push_node(SnapshotNode::new(
        STARS_ENTITY,
        Transform::new(star_center, Quat::IDENTITY, Vec3::splat(STAR_SHELL_SCALE)),
        STARS_MESH,
        STARS_MATERIAL,
        !interior && !limited_scene,
    ));
    if let Some(fade) = world.impact_fade_to_black() {
        snapshot.overlay.fade_to_black = fade;
        snapshot.overlay.game_over = fade >= 1.0;
    }
    snapshot.overlay.hud = hud_state(world, academic_planet);
    let player_defeated = world.players().any(|player| {
        world
            .combatant(player.entity_id)
            .is_some_and(|combatant| combatant.is_defeated())
    });
    if player_defeated {
        snapshot.overlay.game_over = true;
        snapshot.overlay.hud.feedback = Some("JUGADOR DERROTADO".to_owned());
    }
    snapshot.visual_events = world
        .visual_events()
        .map(|event| RenderVisualEvent {
            id: event.id,
            kind: match event.kind {
                gameplay::GameplayVisualEventKind::Attack => RenderVisualEventKind::Attack,
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    RenderVisualEventKind::EnemyDefeated
                }
                gameplay::GameplayVisualEventKind::ItemCollected => {
                    RenderVisualEventKind::ItemCollected
                }
                gameplay::GameplayVisualEventKind::Scan => RenderVisualEventKind::Scan,
            },
            source: event.source,
            target: event.target,
            position: event.position.map(from_engine),
            emitted_at: event.emitted_at,
            expires_at: event.expires_at,
        })
        .collect();
    for node in visual_event_nodes(&snapshot) {
        snapshot.push_node(node);
    }

    if interior {
        let interior_origin = world.ship_position().map(from_engine).unwrap_or(Vec3::ZERO);
        let ship_yaw = world
            .ship_orientation()
            .map(|orientation| orientation.yaw_degrees)
            .unwrap_or_default();
        let local_light = |local: Vec3| {
            echo7_model_to_world(local, render_core::to_engine(interior_origin), ship_yaw)
        };
        snapshot.push_light(SnapshotLight::new(render_core::RenderLight {
            color: Vec3::new(1.0, 0.93, 0.82),
            intensity: 52.0,
            kind: render_core::RenderLightKind::Point {
                position: from_engine(local_light(Vec3::new(0.0, 5.6, 0.0))),
                range: 32.0,
            },
            casts_shadow: false,
        }));
        for local_position in [Vec3::new(-6.0, 3.5, 0.0), Vec3::new(6.0, 3.5, 0.0)] {
            snapshot.push_light(SnapshotLight::new(render_core::RenderLight {
                color: Vec3::new(0.86, 0.92, 1.0),
                intensity: 34.0,
                kind: render_core::RenderLightKind::Point {
                    position: from_engine(local_light(local_position)),
                    range: 24.0,
                },
                casts_shadow: false,
            }));
        }
    }

    if !limited_scene && world.navigation().ship_id().is_some() {
        snapshot.push_node(SnapshotNode::new(
            SUN_ENTITY,
            sun_model_transform(),
            SUN_MODEL_MESH,
            SUN_MATERIAL,
            true,
        ));
    }

    let surface_ship = surface_planet.and_then(|planet| surface_ship_position(world, planet));
    if let (Some(ship), Some(pos)) = (
        world.navigation().ship_id(),
        surface_ship.or_else(|| visual_ship_position(world)),
    ) {
        let orientation = world.ship_orientation().unwrap_or_default();
        let rotation = if surface_ship.is_some() {
            echo7_visual_rotation(gameplay::ShipOrientation::default())
        } else {
            Quat::from_rotation_y(orientation.yaw_degrees.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD)
                * Quat::from_rotation_x(orientation.pitch_degrees.to_radians())
        };
        let model_rotation = rotation * echo7_local_to_world_rotation();
        let visual_position = if interior || pilot_mode {
            world.ship_position().map(from_engine).unwrap_or(pos)
        } else {
            pos
        };
        let mut transform = Transform::new(visual_position, model_rotation, ECHO7_VISUAL_SCALE);
        transform.translation -= rotation
            * (echo7_local_to_world_rotation() * (ECHO7_VISUAL_SCALE * ECHO7_MODEL_CENTER));
        let meshes = if interior || pilot_mode {
            [
                ECHO7_MESH,
                MeshHandle::ECHO7_INTERIOR_LOWER,
                MeshHandle::ECHO7_INTERIOR_UPPER,
                MeshHandle::ECHO7_INTERIOR_CORE,
            ]
        } else {
            [
                ECHO7_MESH,
                MeshHandle::new(0),
                MeshHandle::new(0),
                MeshHandle::new(0),
            ]
        };
        let count = if interior || pilot_mode { 4 } else { 1 };
        for (index, mesh) in meshes.into_iter().take(count).enumerate() {
            let entity = if interior || pilot_mode {
                [
                    ship,
                    INTERIOR_LOWER_ENTITY,
                    INTERIOR_UPPER_ENTITY,
                    INTERIOR_CORE_ENTITY,
                ][index]
            } else {
                ship
            };
            snapshot.push_node(SnapshotNode::new(
                entity,
                transform,
                mesh,
                ECHO7_MATERIAL,
                world.entities().contains(ship),
            ));
        }
    }

    if !limited_scene {
        for node in station_nodes_for_view(world, interior) {
            snapshot.push_node(node);
        }

        for node in mission_interaction_nodes(world, interior) {
            snapshot.push_node(node);
        }
        // The visual runtime is created in interior mode so the first gameplay
        // camera is ready immediately, but the title screen has not started
        // the simulation yet. Do not build gameplay guidance for that frame:
        // it can make the initial snapshot do unnecessary world work and can
        // leave the start screen looking frozen on slower machines.
        if interior && world.time().elapsed.0 > 0 {
            for node in echo7_guidance_nodes(world) {
                snapshot.push_node(node);
            }
            for node in echo7_interaction_sign_nodes(world) {
                snapshot.push_node(node);
            }
        }

        for node in derelict_nodes(world) {
            snapshot.push_node(node);
        }
        if !interior {
            for node in ambient_derelict_nodes() {
                snapshot.push_node(node);
            }
        }

        for node in asteroid_field_nodes(world) {
            snapshot.push_node(node);
        }
    }

    for player in world.players() {
        let transform = if let Some(planet) = surface_planet {
            surface_player_transform(world, player, planet)
        } else {
            visual_transform(
                player.position,
                player.orientation.yaw_degrees,
                player.orientation.pitch_degrees,
            )
        };
        snapshot.push_node(SnapshotNode::new(
            player.entity_id,
            transform,
            PLAYER_MESH,
            PLAYER_MATERIAL,
            player.is_active && !pilot_mode && world.entities().contains(player.entity_id),
        ));
    }

    if !limited_scene {
        for companion in world.companions() {
            snapshot.push_node(SnapshotNode::new(
                companion.entity,
                Transform::from_translation(from_engine(companion.position)),
                ERICK_MESH,
                ERICK_MATERIAL,
                companion.active && world.entities().contains(companion.entity),
            ));
        }

        for enemy in world.enemies() {
            if enemy.is_player {
                continue;
            }
            let visible =
                enemy.active && !enemy.is_defeated() && world.entities().contains(enemy.entity);
            snapshot.push_node(SnapshotNode::new(
                enemy.entity,
                Transform::new(
                    from_engine(enemy.position),
                    Quat::IDENTITY,
                    ENEMY_SHIP_VISUAL_SCALE,
                ),
                ENEMY_MESH,
                enemy_material(enemy.archetype),
                visible,
            ));
        }
    }

    for dest in world.destinations() {
        if limited_scene
            && landing_planet != Some(dest.id)
            && surface_planet != Some(dest.id)
        {
            continue;
        }
        if dest.id == gameplay::DestinationId::new(9_001) {
            // Echo-7's origin is the player's spawn marker, not a world object.
            continue;
        }
        if params.destinations.is_empty() || params.destinations.contains(&dest.id) {
            let (mesh, material, scale) = match dest.destination_type {
                gameplay::DestinationType::Planet => (
                    planet_mesh_for_destination(
                        dest.id,
                        academic_planet,
                        tessara_planet,
                        kharvex_planet,
                        nareth_planet,
                        nareth_mesh_ready,
                    ),
                    PLANET_MATERIAL,
                    Vec3::splat(planet_visual_radius(dest.id)),
                ),
                _ => (ASTEROID_FIELD_MESH, ASTEROID_MATERIAL, Vec3::splat(0.65)),
            };
            let transform = if academic_planet == Some(dest.id) {
                let mut transform = building_planet_transform(elapsed_seconds);
                if let Some(position) = world.planet_position_at(dest.id, world.time().elapsed) {
                    transform.translation = from_engine(position);
                    transform.translation -=
                        transform.rotation * (transform.scale * BUILDING_PLANET_CENTER);
                }
                transform
            } else if tessara_planet == Some(dest.id) {
                let position = world
                    .planet_position_at(dest.id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| from_engine(dest.position));
                if surface_planet == Some(dest.id) {
                    surface_transform
                        .unwrap_or_else(|| tessara_planet_transform(position, elapsed_seconds))
                } else {
                    tessara_planet_transform(position, elapsed_seconds)
                }
            } else if kharvex_planet == Some(dest.id) {
                let position = world
                    .planet_position_at(dest.id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| from_engine(dest.position));
                kharvex_planet_transform(position, elapsed_seconds)
            } else if nareth_planet == Some(dest.id) && nareth_mesh_ready {
                let position = world
                    .planet_position_at(dest.id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| from_engine(dest.position));
                nareth_planet_transform(position, elapsed_seconds)
            } else if nareth_planet == Some(dest.id) {
                let position = world
                    .planet_position_at(dest.id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| from_engine(dest.position));
                Transform::new(
                    position,
                    Quat::from_rotation_y(
                        NARETH_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND * elapsed_seconds as f32,
                    ),
                    Vec3::splat(NARETH_VISUAL_RADIUS),
                )
            } else {
                Transform::new(from_engine(dest.position), Quat::IDENTITY, scale)
            };
            snapshot.push_node(SnapshotNode::destination(
                dest.id.value(),
                transform,
                mesh,
                material,
                dest.discovery != gameplay::DiscoveryState::Unknown,
            ));
        }
    }

    if !limited_scene {
        if let Some(tessara_id) = tessara_planet {
            let tessara_visible = world.destination(tessara_id).is_some_and(|destination| {
                destination.discovery != gameplay::DiscoveryState::Unknown
            });
            if tessara_visible {
                let tessara_position = world
                    .planet_position_at(tessara_id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| {
                        world
                            .destination(tessara_id)
                            .map(|destination| from_engine(destination.position))
                            .unwrap_or(TESSARA_POSITION)
                    });
                let moon_transform = tessara_moon_transform(tessara_position, elapsed_seconds);
                snapshot.push_node(SnapshotNode::new(
                    TESSARA_MOON_ENTITY,
                    moon_transform,
                    TESSARA_MOON_MESH,
                    PLANET_MATERIAL,
                    true,
                ));
            }
        }
    }

    let academic_planet_visible = academic_planet.is_some_and(|id| {
        (params.destinations.is_empty() || params.destinations.contains(&id))
            && world.destination(id).is_some_and(|destination| {
                destination.discovery != gameplay::DiscoveryState::Unknown
            })
    });
    if !limited_scene && academic_planet_visible {
        if let Some(planet_id) = academic_planet {
            if world.landing_available(planet_id) {
                let position = world
                    .planet_position_at(planet_id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| academic_planet_transform(elapsed_seconds).translation);
                snapshot.push_node(SnapshotNode::new(
                    LANDING_INDICATOR_ENTITY,
                    Transform::new(
                        position + Vec3::new(0.0, PLANET_VISUAL_RADIUS + 0.8, 0.0),
                        Quat::IDENTITY,
                        Vec3::splat(0.65),
                    ),
                    DESTINATION_MESH,
                    DESTINATION_MATERIAL,
                    true,
                ));
            }
        }
        for node in academic_surface_objects(elapsed_seconds) {
            snapshot.push_node(node);
        }
    }

    if let Some(ship_position) = visual_ship_position(world) {
        match world.planet_flight_state() {
            gameplay::PlanetFlightState::Atmospheric { intensity, .. } => {
                snapshot.push_node(SnapshotNode::new(
                    ATMOSPHERE_EFFECT_ENTITY,
                    Transform::new(
                        ship_position,
                        Quat::IDENTITY,
                        Vec3::splat(0.35 + intensity.clamp(0.0, 1.0) * 0.8),
                    ),
                    SUN_MESH,
                    SUN_MATERIAL,
                    true,
                ));
            }
            gameplay::PlanetFlightState::Impacted { .. } => {
                if let Some(intensity) = world.impact_flash_intensity() {
                    snapshot.push_node(SnapshotNode::new(
                        IMPACT_FLASH_ENTITY,
                        Transform::new(
                            ship_position,
                            Quat::IDENTITY,
                            Vec3::splat(0.5 + intensity * 1.5),
                        ),
                        SUN_MESH,
                        SUN_MATERIAL,
                        true,
                    ));
                }
            }
            _ => {}
        }
    }

    if params.include_collectibles {
        for c in world.collectibles() {
            snapshot.push_node(SnapshotNode::new(
                c.entity,
                Transform::new(from_engine(c.position), Quat::IDENTITY, Vec3::splat(0.65)),
                ASTEROID_FIELD_MESH,
                ASTEROID_MATERIAL,
                c.active && world.entities().contains(c.entity),
            ));
        }
    }

    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_point()));
    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_sun()));
    if let Some(planet) = surface_planet {
        if let Some(center) = world
            .planet_position_at(planet, world.time().elapsed)
            .map(from_engine)
            .or_else(|| {
                world
                    .destination(planet)
                    .map(|value| from_engine(value.position))
            })
        {
            // A local fill light follows the explored planet. The fixed space
            // lights are useful at orbital scale but leave the clock's
            // detailed geometry almost black from the surface.
            snapshot.push_light(SnapshotLight::new(render_core::RenderLight {
                color: Vec3::new(1.0, 0.88, 0.72),
                intensity: 2.4,
                kind: RenderLightKind::Point {
                    position: center + Vec3::new(0.0, 32.0, 12.0),
                    range: 95.0,
                },
                casts_shadow: false,
            }));
            snapshot.push_light(SnapshotLight::new(render_core::RenderLight {
                color: Vec3::new(0.55, 0.7, 1.0),
                intensity: 0.8,
                kind: RenderLightKind::Point {
                    position: center + Vec3::new(-28.0, 8.0, -24.0),
                    range: 80.0,
                },
                casts_shadow: false,
            }));
        }
    }

    snapshot
}

fn visual_event_nodes(snapshot: &RenderSnapshot) -> Vec<SnapshotNode> {
    snapshot
        .visual_events
        .iter()
        .filter_map(|event| {
            let position = event.position?;
            let duration = event.expires_at.0.saturating_sub(event.emitted_at.0);
            if duration == 0 || snapshot.simulation_time.0 < event.emitted_at.0 {
                return None;
            }
            let progress = ((snapshot.simulation_time.0 - event.emitted_at.0) as f32
                / duration as f32)
                .clamp(0.0, 1.0);
            let intensity = 1.0 - progress;
            let (mesh, material, scale) = match event.kind {
                RenderVisualEventKind::Attack => {
                    (SUN_MESH, SUN_MATERIAL, Vec3::splat(0.2 + intensity * 0.45))
                }
                RenderVisualEventKind::EnemyDefeated => {
                    (SUN_MESH, SUN_MATERIAL, Vec3::splat(0.45 + intensity * 1.4))
                }
                RenderVisualEventKind::ItemCollected => (
                    COLLECTIBLE_MESH,
                    COLLECTIBLE_MATERIAL,
                    Vec3::splat(0.15 + intensity * 0.35),
                ),
                RenderVisualEventKind::Scan => (
                    DESTINATION_MESH,
                    DESTINATION_MATERIAL,
                    Vec3::splat(0.3 + intensity * 0.8),
                ),
            };
            Some(SnapshotNode::visual_event(
                event.id,
                Transform::new(position, Quat::IDENTITY, scale),
                mesh,
                material,
            ))
        })
        .collect()
}

fn hud_state(world: &GameplayWorld, academic_planet: Option<gameplay::DestinationId>) -> HudState {
    let flight = match world.planet_flight_state() {
        gameplay::PlanetFlightState::Free => HudFlightState::Free,
        gameplay::PlanetFlightState::Atmospheric { .. } => HudFlightState::Atmospheric,
        gameplay::PlanetFlightState::Landing { .. } => HudFlightState::Landing,
        gameplay::PlanetFlightState::Landed { .. } => HudFlightState::Landed,
        gameplay::PlanetFlightState::Impacted { .. } => HudFlightState::Impacted,
    };
    let (autopilot, autopilot_target) = match world.autopilot_state() {
        gameplay::AutopilotState::Inactive => (HudAutopilotState::Inactive, None),
        gameplay::AutopilotState::Planned { destination, .. } => {
            (HudAutopilotState::Planned, Some(destination.value()))
        }
        gameplay::AutopilotState::InTransit { destination, .. } => {
            (HudAutopilotState::InTransit, Some(destination.value()))
        }
        gameplay::AutopilotState::Completed { destination } => {
            (HudAutopilotState::Completed, Some(destination.value()))
        }
        gameplay::AutopilotState::Interrupted { destination } => {
            (HudAutopilotState::Interrupted, Some(destination.value()))
        }
        gameplay::AutopilotState::Failed { destination } => {
            (HudAutopilotState::Failed, Some(destination.value()))
        }
    };
    let target = autopilot_target.or_else(|| match world.travel_state() {
        gameplay::TravelState::InProgress { destination, .. }
        | gameplay::TravelState::Completed { destination }
        | gameplay::TravelState::Cancelled { destination } => Some(destination.value()),
        gameplay::TravelState::Idle => None,
    });
    let mut completed_mission = None;
    for event in world.events().iter() {
        if let gameplay::GameplayEvent::MissionCompleted { mission, .. } = event {
            completed_mission = Some(*mission);
        }
    }
    let presented_mission = presentation_mission(world);
    let mission = presented_mission.and_then(|mission| world.mission(mission));
    let current_objective = mission.and_then(|value| {
        value
            .objectives()
            .iter()
            .find(|objective| objective.state() == ObjectiveState::Active)
            .or_else(|| {
                value
                    .objectives()
                    .iter()
                    .find(|objective| objective.state() == ObjectiveState::Available)
            })
            .or_else(|| value.objectives().first())
    });
    let mission_progress = mission.map(|value| value.progress());
    let mission_state = mission_progress.map(|value| hud_mission_state(value.state));
    let mission_objectives = mission
        .map(|value| {
            value
                .objectives()
                .iter()
                .map(|objective| HudMissionObjective {
                    id: objective.id.0,
                    name: objective.name.clone(),
                    state: hud_objective_state(objective.state()),
                    progress: objective.progress(),
                    required_progress: objective.required_progress(),
                    required: objective.required,
                })
                .collect()
        })
        .unwrap_or_default();
    HudState {
        flight,
        autopilot,
        ship_shield_active: world.ship_control_active() == Some(true)
            && world
                .navigation()
                .ship_id()
                .is_some_and(|ship| world.ship_shield_active(ship)),
        ship_boost_active: world.ship_control_active() == Some(true)
            && world.ship_boost_active().unwrap_or(false),
        ship_speed: (world.ship_control_active() == Some(true))
            .then(|| world.ship_speed().zip(world.ship_speed_limit()))
            .flatten()
            .map(|(speed, limit)| (speed.round().max(0.0) as u32, limit.round().max(0.0) as u32)),
        weapon_name: (world.ship_control_active() == Some(true))
            .then(|| {
                world
                    .combatants()
                    .find(|value| value.is_player)
                    .and_then(|value| value.equipped_weapon)
            })
            .flatten()
            .map(|weapon| match weapon.attack_type {
                gameplay::AttackType::Kinetic => "CINETICA".to_owned(),
                gameplay::AttackType::Energy => "ENERGIA".to_owned(),
                gameplay::AttackType::Plasma => "PLASMA".to_owned(),
            }),
        weapon_ammunition: (world.ship_control_active() == Some(true))
            .then(|| {
                world
                    .players()
                    .next()
                    .and_then(|player| world.player_weapon_ammunition(player.entity_id))
                    .map(|(item, amount)| (item.0, amount))
            })
            .flatten(),
        ship_fuel: (world.ship_control_active() == Some(true))
            .then(|| {
                world
                    .navigation()
                    .ship_id()
                    .and_then(|ship| world.ship_fuel(ship))
            })
            .flatten()
            .map(|fuel| (fuel.current(), fuel.capacity())),
        ship_oxygen: (world.ship_control_active() == Some(true))
            .then(|| {
                world
                    .life_support_panels()
                    .next()
                    .map(|panel| (panel.allocated_total(), panel.capacity()))
            })
            .flatten(),
        ship_supplies: (world.ship_control_active() == Some(true))
            .then(|| {
                world
                    .players()
                    .next()
                    .and_then(|player| world.inventory(player.entity_id))
                    .map(|inventory| inventory.quantity(gameplay::types::ItemId(1)))
            })
            .flatten(),
        ship_resource_alert: if world.ship_control_active() == Some(true) {
            let fuel_alert = world
                .navigation()
                .ship_id()
                .and_then(|ship| world.ship_fuel(ship))
                .and_then(|fuel| {
                    (fuel.current() == 0)
                        .then_some("COMBUSTIBLE AGOTADO")
                        .or_else(|| {
                            (fuel.current() * 4 <= fuel.capacity()).then_some("COMBUSTIBLE BAJO")
                        })
                });
            let oxygen_alert = world.life_support_panels().next().and_then(|panel| {
                (panel.allocated_total() == 0)
                    .then_some("OXIGENO AGOTADO")
                    .or_else(|| {
                        (panel.allocated_total() * 4 <= panel.capacity()).then_some("OXIGENO BAJO")
                    })
            });
            fuel_alert.or(oxygen_alert).map(str::to_owned)
        } else {
            None
        },
        target,
        active_enemies: world
            .enemies()
            .filter(|enemy| enemy.active && !enemy.is_defeated())
            .count() as u32,
        landing_available: academic_planet.is_some_and(|planet| world.landing_available(planet)),
        mission_name: mission.map(|value| value.name.clone()),
        mission_description: mission.and_then(|value| value.description.clone()),
        objective_name: current_objective.map(|value| value.name.clone()),
        mission_current_objective: current_objective.map(|value| value.id.0),
        mission_objectives,
        mission_progress: mission_progress
            .map(|value| (value.required_completed as u8, value.required_total as u8)),
        mission_state,
        mission_notification: completed_mission
            .or(presented_mission)
            .and_then(|value| mission_notification(world, value)),
        contextual_action: None,
        controls_panel: Default::default(),
        interaction_prompt: None,
        guidance: None,
        controls: None,
        instructions: None,
        feedback: None,
        inventory_open: false,
        inventory_items: Vec::new(),
    }
}

fn hud_objective_state(state: ObjectiveState) -> HudObjectiveState {
    match state {
        ObjectiveState::Locked => HudObjectiveState::Locked,
        ObjectiveState::Available => HudObjectiveState::Available,
        ObjectiveState::Active => HudObjectiveState::Active,
        ObjectiveState::Failed => HudObjectiveState::Failed,
        ObjectiveState::Completed => HudObjectiveState::Completed,
    }
}

fn mission_notification(world: &GameplayWorld, mission: MissionId) -> Option<String> {
    let mut selected = None;
    let mut selected_priority = 0;
    for event in world.events().iter() {
        let (priority, text) = match event {
            gameplay::GameplayEvent::MissionStarted {
                mission: actual, ..
            } if *actual == mission => (3, "NUEVA MISION"),
            gameplay::GameplayEvent::ObjectiveAvailable { objective, .. }
            | gameplay::GameplayEvent::ObjectiveStarted { objective, .. }
                if world
                    .objective(*objective)
                    .is_some_and(|value| value.mission_id == mission) =>
            {
                (2, "OBJETIVO ACTUALIZADO")
            }
            gameplay::GameplayEvent::ObjectiveCompleted { objective, .. }
                if world
                    .objective(*objective)
                    .is_some_and(|value| value.mission_id == mission) =>
            {
                (4, "OBJETIVO COMPLETADO")
            }
            gameplay::GameplayEvent::MissionCompleted {
                mission: actual, ..
            } if *actual == mission => (5, "MISION COMPLETADA"),
            _ => continue,
        };
        if priority >= selected_priority {
            selected_priority = priority;
            selected = Some(text.to_owned());
        }
    }
    selected
}

fn hud_mission_state(state: gameplay::domains::mission::MissionState) -> HudMissionState {
    match state {
        gameplay::domains::mission::MissionState::Locked => HudMissionState::Locked,
        gameplay::domains::mission::MissionState::Available => HudMissionState::Available,
        gameplay::domains::mission::MissionState::Active => HudMissionState::Active,
        gameplay::domains::mission::MissionState::Failed => HudMissionState::Failed,
        gameplay::domains::mission::MissionState::Completed => HudMissionState::Completed,
    }
}

fn expected_snapshot_node_count(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    academic_planet: Option<gameplay::DestinationId>,
    interior: bool,
) -> usize {
    let mut count = 0;
    if world.navigation().ship_id().is_some() {
        count += 1;
    }
    if world.navigation().ship_id().is_some() && visual_ship_position(world).is_some() {
        count += 1;
    }
    count += world.players().count();
    count += world.companions().count();
    if !interior {
        count += EXTERIOR_STATION_IDS.len() + AMBIENT_DERELICT_IDS.len();
    }
    count += world
        .derelicts()
        .filter(|derelict| derelict.state != gameplay::DerelictState::Undiscovered)
        .count();
    count += world
        .active_environmental_hazards()
        .filter(|hazard| hazard.hazard_type == gameplay::EnvironmentalHazardType::DebrisField)
        .count();
    count += world.enemies().filter(|enemy| !enemy.is_player).count();
    count += world
        .destinations()
        .filter(|destination| {
            params.destinations.is_empty() || params.destinations.contains(&destination.id)
        })
        .count();

    let academic_planet_visible = academic_planet.is_some_and(|id| {
        (params.destinations.is_empty() || params.destinations.contains(&id))
            && world.destination(id).is_some_and(|destination| {
                destination.discovery != gameplay::DiscoveryState::Unknown
            })
    });
    if academic_planet_visible {
        count += SURFACE_OBJECT_IDS.len();
        if academic_planet.is_some_and(|id| world.landing_available(id)) {
            count += 1;
        }
    }
    if matches!(
        world.planet_flight_state(),
        gameplay::PlanetFlightState::Atmospheric { .. }
    ) || world.impact_flash_intensity().is_some()
    {
        if visual_ship_position(world).is_some() {
            count += 1;
        }
    }
    if params.include_collectibles {
        count += world.collectibles().count();
    }
    count
}

pub fn visual_ship_position(world: &GameplayWorld) -> Option<Vec3> {
    let current = from_engine(world.ship_position()?);
    let gameplay::TravelState::InProgress { destination, .. } = world.travel_state() else {
        return Some(current);
    };
    if world
        .planet_position_at(destination, world.time().elapsed)
        .is_some()
    {
        return Some(current);
    }
    let progress = world.travel_progress()?.progress.clamp(0.0, 1.0);
    let target = world
        .planet_position_at(destination, world.time().elapsed)
        .map(from_engine)
        .or_else(|| {
            world
                .destination(destination)
                .map(|value| from_engine(value.position))
        })?;
    Some(interpolate_visual_ship_position(current, target, progress))
}

pub fn interpolate_visual_ship_position(current: Vec3, target: Vec3, progress: f32) -> Vec3 {
    current + (target - current) * progress.clamp(0.0, 1.0)
}

pub fn flight_camera(world: &GameplayWorld, aspect: f32) -> Result<Camera, CameraError> {
    pilot_camera(world, aspect, PILOT_CAMERA_DEFAULT_DISTANCE)
}

pub fn pilot_camera(
    world: &GameplayWorld,
    aspect: f32,
    distance: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).ok_or(CameraError::InvalidAspect)?;
    let orientation = world.ship_orientation().unwrap_or_default();
    let yaw = orientation.yaw_degrees.to_radians();
    let pitch = orientation.pitch_degrees.to_radians().clamp(-1.2, 1.2);
    let rotation =
        Quat::from_rotation_y(yaw + ECHO7_MODEL_YAW_OFFSET_RAD) * Quat::from_rotation_x(pitch);
    // echo-7.obj is longitudinal on X and its propulsion groups are on +X.
    // Keep the ship's existing visual orientation and derive the chase camera
    // from that same local axis, so the camera starts behind the engines.
    let aft = (rotation * Vec3::X).normalize_or_zero();
    let up = (rotation * Vec3::Z).normalize_or_zero();
    let distance = distance.clamp(PILOT_CAMERA_MIN_DISTANCE, PILOT_CAMERA_MAX_DISTANCE);
    // `ship` is the gameplay/model-center anchor, not the rear of the mesh.
    // Start immediately behind the actual propulsion edge.
    let position = ship + aft * (ECHO7_AFT_EDGE_OFFSET + distance) + Vec3::new(0.0, 3.0, 0.0);
    let target = ship + aft * ECHO7_CAMERA_TARGET_AFT_OFFSET + Vec3::new(0.0, 1.15, 0.0);
    Camera::new(
        position,
        target,
        up,
        Projection::Perspective {
            fov_y_rad: 65.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 2_000.0,
        },
    )
}

pub fn pilot_seated_camera(world: &GameplayWorld, aspect: f32) -> Result<Camera, CameraError> {
    let ship = world.ship_position().ok_or(CameraError::InvalidAspect)?;
    let yaw = world
        .ship_orientation()
        .map(|orientation| orientation.yaw_degrees)
        .unwrap_or_default();
    let position = echo7_model_to_world(ECHO7_PILOT_SEATED_LOCAL, ship, yaw);
    let target = echo7_model_to_world(Vec3::new(-40.0, -2.4, 5.8), ship, yaw);
    Camera::new(
        from_engine(position),
        from_engine(target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 72.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 2_000.0,
        },
    )
}

pub fn observation_camera(
    world: &GameplayWorld,
    aspect: f32,
    transition: f32,
    azimuth: f32,
    elevation: f32,
) -> Result<Camera, CameraError> {
    observation_camera_for_destination(world, aspect, transition, azimuth, elevation, None)
}

pub fn observation_camera_for_destination(
    world: &GameplayWorld,
    aspect: f32,
    transition: f32,
    azimuth: f32,
    elevation: f32,
    target_destination: Option<gameplay::DestinationId>,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let elapsed_seconds = world.time().elapsed.0 as f64 / 1_000.0;
    let target = target_destination
        .filter(|id| is_observable_planet(world, *id))
        .or_else(|| academic_planet_id(world));
    let planet = target
        .and_then(|id| world.planet_position_at(id, world.time().elapsed))
        .map(from_engine)
        .or_else(|| {
            target.and_then(|id| {
                world
                    .destination(id)
                    .map(|value| from_engine(value.position))
            })
        })
        .unwrap_or_else(|| academic_planet_transform(elapsed_seconds).translation);
    let observation_radius = target
        .map(planet_visual_radius)
        .unwrap_or(PLANET_VISUAL_RADIUS);
    let azimuth = if azimuth.is_finite() { azimuth } else { 0.0 };
    let elevation = if elevation.is_finite() {
        elevation.clamp(-1.2, 1.2)
    } else {
        0.0
    };
    let flight_radius = FLIGHT_CAMERA_OFFSET.length();
    let flight_elevation = (FLIGHT_CAMERA_OFFSET.y
        / (FLIGHT_CAMERA_OFFSET.x * FLIGHT_CAMERA_OFFSET.x
            + FLIGHT_CAMERA_OFFSET.z * FLIGHT_CAMERA_OFFSET.z)
            .sqrt())
    .atan()
        + elevation;
    let flight_position = ship
        + Vec3::new(
            flight_radius * flight_elevation.cos() * azimuth.sin(),
            flight_radius * flight_elevation.sin(),
            flight_radius * flight_elevation.cos() * azimuth.cos(),
        );
    let space_position = flight_position;
    let space_target = (SUN_POSITION + ship + planet) / 3.0;
    let surface_direction = Vec3::new(
        elevation.cos() * azimuth.cos(),
        elevation.sin(),
        elevation.cos() * azimuth.sin(),
    );
    let close_position = planet + surface_direction * observation_distance(observation_radius);
    let transition = transition.clamp(0.0, 1.0);
    let mut position = space_position.lerp(close_position, transition);
    if let gameplay::PlanetFlightState::Atmospheric { intensity, .. } = world.planet_flight_state()
    {
        let phase = elapsed_seconds as f32 * 28.0;
        let amplitude = intensity.clamp(0.0, 1.0) * 0.22;
        position.x += phase.sin() * amplitude;
        position.y += (phase * 1.37).cos() * amplitude;
    }
    let target = space_target.lerp(planet, transition);
    Camera::new(
        position,
        target,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 65.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 500.0,
        },
    )
}

pub fn observable_planets(world: &GameplayWorld) -> Vec<gameplay::DestinationId> {
    let mut planets = world
        .destinations()
        .filter(|destination| destination.destination_type == gameplay::DestinationType::Planet)
        .filter(|destination| is_observable_planet(world, destination.id))
        .map(|destination| destination.id)
        .collect::<Vec<_>>();
    planets.sort_unstable_by_key(|id| id.value());
    planets
}

pub fn is_observable_planet(world: &GameplayWorld, destination: gameplay::DestinationId) -> bool {
    world.destination(destination).is_some_and(|value| {
        value.destination_type == gameplay::DestinationType::Planet
            && (world
                .planet_position_at(destination, world.time().elapsed)
                .is_some()
                || (value.position.x.is_finite()
                    && value.position.y.is_finite()
                    && value.position.z.is_finite()))
    })
}

pub fn planet_visual_radius(destination: gameplay::DestinationId) -> f32 {
    match destination.value() {
        ACADEMIC_PLANET_DESTINATION_ID => PLANET_VISUAL_RADIUS,
        TESSARA_DESTINATION_ID => TESSARA_VISUAL_RADIUS,
        KHARVEX_DESTINATION_ID => KHARVEX_VISUAL_RADIUS,
        NARETH_DESTINATION_ID => NARETH_VISUAL_RADIUS,
        _ => PLANET_VISUAL_RADIUS,
    }
}

pub fn planet_surface_origin(
    world: &GameplayWorld,
    destination: gameplay::DestinationId,
) -> Option<Vec3> {
    let center = world
        .planet_position_at(destination, world.time().elapsed)
        .map(from_engine)
        .or_else(|| {
            world
                .destination(destination)
                .map(|value| from_engine(value.position))
        })?;
    Some(center + Vec3::new(0.0, planet_visual_radius(destination) + 1.0, 0.0))
}

pub fn surface_ship_position(
    world: &GameplayWorld,
    destination: gameplay::DestinationId,
) -> Option<Vec3> {
    planet_surface_origin(world, destination).map(|origin| origin + Vec3::new(4.0, 0.0, 4.0))
}

fn surface_player_transform(
    world: &GameplayWorld,
    player: &gameplay::Player,
    destination: gameplay::DestinationId,
) -> Transform {
    let position = from_engine(player.position);
    let center = world
        .planet_position_at(destination, world.time().elapsed)
        .map(from_engine)
        .or_else(|| world.destination(destination).map(|value| from_engine(value.position)))
        .unwrap_or(Vec3::ZERO);
    let up = if destination.value() == TESSARA_DESTINATION_ID {
        Vec3::Y
    } else {
        (position - center).normalize_or_zero()
    };
    let up = if up.length_squared() > 0.0 { up } else { Vec3::Y };
    let gravity_alignment = Quat::from_rotation_arc(Vec3::Y, up);
    let local_view = Quat::from_rotation_y(player.orientation.yaw_degrees.to_radians())
        * Quat::from_rotation_x(player.orientation.pitch_degrees.to_radians());
    Transform::new(position, gravity_alignment * local_view, PLAYER_VISUAL_SCALE)
}

fn observation_distance(visual_radius: f32) -> f32 {
    (visual_radius.max(0.1) * 1.75).max(PLANET_OBSERVATION_RADIUS)
}

pub fn interior_camera(world: &GameplayWorld, aspect: f32) -> Result<Camera, CameraError> {
    let player = world.players().next().ok_or(CameraError::InvalidAspect)?;
    interior_camera_for_player(world, player.entity_id, aspect)
}

pub fn interior_camera_for_player(
    world: &GameplayWorld,
    player_id: engine::EntityId,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let player = world.player(player_id).ok_or(CameraError::InvalidAspect)?;
    let position = from_engine(player.position) + Vec3::new(0.0, ECHO7_INTERIOR_EYE_HEIGHT, 0.0);
    let yaw = player.orientation.yaw_degrees.to_radians();
    let pitch = player
        .orientation
        .pitch_degrees
        .to_radians()
        .clamp(-1.2, 1.2);
    let horizontal = pitch.cos();
    let direction =
        Vec3::new(yaw.sin() * horizontal, pitch.sin(), -yaw.cos() * horizontal).normalize_or_zero();
    Camera::new(
        position,
        position + direction,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 72.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 120.0,
        },
    )
}

pub fn surface_camera_for_player_with_view(
    world: &GameplayWorld,
    player_id: engine::EntityId,
    destination: gameplay::DestinationId,
    aspect: f32,
    camera_yaw_offset: f32,
    camera_pitch_offset: f32,
) -> Result<Camera, CameraError> {
    let player = world.player(player_id).ok_or(CameraError::InvalidAspect)?;
    let center = world.planet_position_at(destination, world.time().elapsed).map(from_engine)
        .or_else(|| world.destination(destination).map(|value| from_engine(value.position)))
        .ok_or(CameraError::InvalidAspect)?;
    let player_position = from_engine(player.position);
    let up = if destination.value() == TESSARA_DESTINATION_ID { Vec3::Y }
        else { (player_position - center).normalize_or_zero() };
    let up = if up.length_squared() > 0.0 { up } else { Vec3::Y };
    let yaw = (player.orientation.yaw_degrees + camera_yaw_offset).to_radians();
    let pitch = (player.orientation.pitch_degrees + camera_pitch_offset).to_radians().clamp(-1.2, 1.2);
    let horizontal = pitch.cos();
    let raw_forward = Vec3::new(yaw.sin() * horizontal, pitch.sin(), -yaw.cos() * horizontal);
    let forward = (raw_forward - up * raw_forward.dot(up)).normalize_or_zero();
    let forward = if forward.length_squared() > 0.0 { forward } else {
        up.cross(if up.dot(Vec3::Y).abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or_zero()
    };
    Camera::new(
        player_position - forward * 4.5 + up * 2.2,
        player_position + up * 0.25,
        up,
        Projection::Perspective { fov_y_rad: 72.0_f32.to_radians(), aspect, near: 0.05, far: 220.0 },
    )
}

pub fn lower_corridor_inspection_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let reverse = matches!(
        std::env::var("ASTRAL_ECHO_LOWER_CORRIDOR_SIDE").as_deref(),
        Ok("reverse") | Ok("1") | Ok("true")
    );
    let local_position = Vec3::new(-24.0, 0.0, 1.65);
    let local_target = Vec3::new(-24.0, if reverse { -6.0 } else { 6.0 }, 1.65);
    let to_world = |local: Vec3| {
        let rotation = Quat::from_rotation_y(
            world
                .ship_orientation()
                .map(|orientation| orientation.yaw_degrees)
                .unwrap_or_default()
                .to_radians()
                + ECHO7_MODEL_YAW_OFFSET_RAD,
        );
        let offset = rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)));
        ship + offset
    };
    Camera::new(
        to_world(local_position),
        to_world(local_target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 150.0,
        },
    )
}

pub fn upper_corridor_inspection_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let reverse = matches!(
        std::env::var("ASTRAL_ECHO_LOWER_CORRIDOR_SIDE").as_deref(),
        Ok("reverse") | Ok("1") | Ok("true")
    );
    let local_position = Vec3::new(-24.0, 0.0, 5.6);
    let local_target = Vec3::new(-24.0, if reverse { -6.0 } else { 6.0 }, 5.6);
    let rotation = Quat::from_rotation_y(
        world
            .ship_orientation()
            .map(|orientation| orientation.yaw_degrees)
            .unwrap_or_default()
            .to_radians()
            + ECHO7_MODEL_YAW_OFFSET_RAD,
    );
    let to_world = |local: Vec3| {
        ship + rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)))
    };
    Camera::new(
        to_world(local_position),
        to_world(local_target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 150.0,
        },
    )
}

pub fn cloning_inspection_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let reverse = matches!(
        std::env::var("ASTRAL_ECHO_CLONACION_SIDE").as_deref(),
        Ok("reverse") | Ok("1") | Ok("true")
    );
    let local_position = Vec3::new(-20.0, 15.0, 1.65 / ECHO7_VISUAL_SCALE.z);
    let local_target = Vec3::new(
        -20.0,
        if reverse { 21.0 } else { 11.0 },
        1.65 / ECHO7_VISUAL_SCALE.z,
    );
    let rotation = Quat::from_rotation_y(
        world
            .ship_orientation()
            .map(|orientation| orientation.yaw_degrees)
            .unwrap_or_default()
            .to_radians()
            + ECHO7_MODEL_YAW_OFFSET_RAD,
    );
    let to_world = |local: Vec3| {
        ship + rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)))
    };
    Camera::new(
        to_world(local_position),
        to_world(local_target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 150.0,
        },
    )
}

pub fn despertar_inspection_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let reverse = matches!(
        std::env::var("ASTRAL_ECHO_DESPERTAR_SIDE").as_deref(),
        Ok("reverse") | Ok("1") | Ok("true")
    );
    let capsule = std::env::var("ASTRAL_ECHO_DESPERTAR_CAPSULE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (1..=3).contains(value))
        .unwrap_or(1);
    let capsule_x = match capsule {
        1 => -24.0,
        2 => -20.0,
        _ => -16.0,
    };
    let bench_view = matches!(
        std::env::var("ASTRAL_ECHO_DESPERTAR_BANCO").as_deref(),
        Ok("1") | Ok("true")
    );
    if bench_view {
        let local_position = if reverse {
            Vec3::new(-11.0, -10.0, 1.65 / ECHO7_VISUAL_SCALE.z)
        } else {
            Vec3::new(-20.0, -15.0, 1.65 / ECHO7_VISUAL_SCALE.z)
        };
        let local_target = Vec3::new(-15.4, -12.1, 0.15 / ECHO7_VISUAL_SCALE.z);
        let rotation = Quat::from_rotation_y(
            world
                .ship_orientation()
                .map(|orientation| orientation.yaw_degrees)
                .unwrap_or_default()
                .to_radians()
                + ECHO7_MODEL_YAW_OFFSET_RAD,
        );
        let to_world = |local: Vec3| {
            ship + rotation
                * (echo7_local_to_world_rotation()
                    * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)))
        };
        return Camera::new(
            to_world(local_position),
            to_world(local_target),
            Vec3::Y,
            Projection::Perspective {
                fov_y_rad: 60.0_f32.to_radians(),
                aspect,
                near: 0.05,
                far: 150.0,
            },
        );
    }
    let local_position = Vec3::new(capsule_x - 3.0, -15.0, 1.65 / ECHO7_VISUAL_SCALE.z);
    let local_target = Vec3::new(
        capsule_x,
        if reverse { -12.5 } else { -18.0 },
        1.25 / ECHO7_VISUAL_SCALE.z,
    );
    let rotation = Quat::from_rotation_y(
        world
            .ship_orientation()
            .map(|orientation| orientation.yaw_degrees)
            .unwrap_or_default()
            .to_radians()
            + ECHO7_MODEL_YAW_OFFSET_RAD,
    );
    let to_world = |local: Vec3| {
        ship + rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)))
    };
    Camera::new(
        to_world(local_position),
        to_world(local_target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 150.0,
        },
    )
}

pub fn enfermeria_inspection_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let reverse = matches!(
        std::env::var("ASTRAL_ECHO_ENFERMERIA_SIDE").as_deref(),
        Ok("reverse") | Ok("1") | Ok("true")
    );
    let local_position = Vec3::new(-4.0, 18.0, 1.65 / ECHO7_VISUAL_SCALE.z);
    let local_target = Vec3::new(
        if reverse { -4.0 } else { 4.0 },
        13.0,
        1.65 / ECHO7_VISUAL_SCALE.z,
    );
    let rotation = Quat::from_rotation_y(
        world
            .ship_orientation()
            .map(|orientation| orientation.yaw_degrees)
            .unwrap_or_default()
            .to_radians()
            + ECHO7_MODEL_YAW_OFFSET_RAD,
    );
    let to_world = |local: Vec3| {
        ship + rotation
            * (echo7_local_to_world_rotation()
                * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)))
    };
    Camera::new(
        to_world(local_position),
        to_world(local_target),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 150.0,
        },
    )
}

fn academic_planet_id(world: &GameplayWorld) -> Option<gameplay::DestinationId> {
    let planets: Vec<_> = world
        .destinations()
        .filter(|destination| destination.destination_type == gameplay::DestinationType::Planet)
        .map(|destination| destination.id)
        .collect();
    planets
        .iter()
        .copied()
        .find(|id| id.value() == ACADEMIC_PLANET_DESTINATION_ID)
        .or_else(|| {
            if planets.len() == 1 {
                Some(planets[0])
            } else {
                None
            }
        })
}

fn tessara_planet_id(world: &GameplayWorld) -> Option<gameplay::DestinationId> {
    world
        .destinations()
        .find(|destination| destination.id.value() == TESSARA_DESTINATION_ID)
        .map(|destination| destination.id)
}

fn kharvex_planet_id(world: &GameplayWorld) -> Option<gameplay::DestinationId> {
    world
        .destinations()
        .find(|destination| destination.id.value() == KHARVEX_DESTINATION_ID)
        .map(|destination| destination.id)
}

fn nareth_planet_id(world: &GameplayWorld) -> Option<gameplay::DestinationId> {
    world
        .destinations()
        .find(|destination| destination.id.value() == NARETH_DESTINATION_ID)
        .map(|destination| destination.id)
}

pub fn academic_planet_transform(elapsed_seconds: f64) -> Transform {
    let orbit_angle = PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND as f64 * elapsed_seconds;
    let rotation_angle = PLANET_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND as f64 * elapsed_seconds;
    Transform::new(
        SUN_POSITION
            + Vec3::new(
                (orbit_angle.cos() as f32) * PLANET_ORBIT_RADIUS,
                0.0,
                (orbit_angle.sin() as f32) * PLANET_ORBIT_RADIUS,
            ),
        Quat::from_rotation_y(rotation_angle as f32),
        Vec3::splat(PLANET_VISUAL_RADIUS),
    )
}

fn planet_mesh_for_destination(
    destination: gameplay::DestinationId,
    academic_planet: Option<gameplay::DestinationId>,
    tessara_planet: Option<gameplay::DestinationId>,
    kharvex_planet: Option<gameplay::DestinationId>,
    nareth_planet: Option<gameplay::DestinationId>,
    nareth_mesh_ready: bool,
) -> MeshHandle {
    if academic_planet == Some(destination) {
        BUILDING_PLANET_MESH
    } else if tessara_planet == Some(destination) {
        TESSARA_PLANET_MESH
    } else if kharvex_planet == Some(destination) {
        KHARVEX_PLANET_MESH
    } else if nareth_planet == Some(destination) {
        if nareth_mesh_ready {
            NARETH_PLANET_MESH
        } else {
            // This is Nareth's runtime LOD, deliberately separate from the
            // generic Earth test sphere. It is always resident and therefore
            // cannot vanish or trigger an OBJ upload during a cinematic.
            MeshHandle::LANDING_PLANET_PROXY
        }
    } else {
        PLANET_MESH
    }
}

fn building_planet_transform(elapsed_seconds: f64) -> Transform {
    let mut transform = academic_planet_transform(elapsed_seconds);
    transform.scale = Vec3::splat(BUILDING_PLANET_VISUAL_SCALE);
    transform.translation -= transform.rotation * (transform.scale * BUILDING_PLANET_CENTER);
    transform
}

fn tessara_planet_transform(position: Vec3, elapsed_seconds: f64) -> Transform {
    let mut transform = Transform::new(
        position,
        Quat::from_rotation_y(
            TESSARA_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND * elapsed_seconds as f32,
        ),
        Vec3::splat(TESSARA_PLANET_VISUAL_SCALE),
    );
    transform.translation -= transform.rotation * (transform.scale * TESSARA_PLANET_CENTER);
    transform
}

pub fn clock_planet_transform(position: Vec3, elapsed_seconds: f64) -> Transform {
    tessara_planet_transform(position, elapsed_seconds)
}

fn tessara_moon_transform(tessara_position: Vec3, elapsed_seconds: f64) -> Transform {
    let elapsed = elapsed_seconds as f32;
    let angle = TESSARA_MOON_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND * elapsed;
    let offset = Vec3::new(
        TESSARA_MOON_ORBIT_RADIUS * angle.cos(),
        TESSARA_MOON_ORBIT_HEIGHT,
        TESSARA_MOON_ORBIT_RADIUS * angle.sin(),
    );
    let rotation =
        Quat::from_rotation_y(TESSARA_MOON_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND * elapsed);
    let mut transform = Transform::new(
        tessara_position + offset,
        rotation,
        Vec3::splat(TESSARA_MOON_VISUAL_SCALE),
    );
    transform.translation -= rotation * (transform.scale * TESSARA_MOON_CENTER);
    transform
}

fn kharvex_planet_transform(position: Vec3, elapsed_seconds: f64) -> Transform {
    let mut transform = Transform::new(
        position,
        Quat::from_rotation_y(
            KHARVEX_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND * elapsed_seconds as f32,
        ),
        Vec3::splat(KHARVEX_PLANET_VISUAL_SCALE),
    );
    transform.translation -= transform.rotation * (transform.scale * KHARVEX_PLANET_CENTER);
    transform
}

fn nareth_planet_transform(position: Vec3, elapsed_seconds: f64) -> Transform {
    Transform::new(
        position,
        Quat::from_rotation_y(
            NARETH_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND * elapsed_seconds as f32,
        ),
        Vec3::splat(NARETH_PLANET_SCALE),
    )
}

fn sun_model_transform() -> Transform {
    let mut transform = Transform::new(
        SUN_POSITION,
        Quat::IDENTITY,
        Vec3::splat(SUN_MODEL_VISUAL_SCALE),
    );
    transform.translation -= transform.rotation * (transform.scale * SUN_MODEL_CENTER);
    transform
}

fn visual_transform(position: engine::Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::new(
        from_engine(position),
        Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(pitch.to_radians()),
        PLAYER_VISUAL_SCALE,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        academic_planet_transform, building_planet_transform, echo7_local_to_world_rotation,
        hud_mission_state, kharvex_planet_transform, planet_mesh_for_destination,
        tessara_moon_transform, tessara_planet_transform, BUILDING_PLANET_CENTER,
        BUILDING_PLANET_MESH, ECHO7_LOWER_DECK_Y, ECHO7_MODEL_CENTER, ECHO7_MODEL_YAW_OFFSET_RAD,
        ECHO7_UPPER_DECK_Y, ECHO7_VISUAL_SCALE, KHARVEX_PLANET_CENTER, KHARVEX_PLANET_MAX_RADIUS,
        KHARVEX_PLANET_MESH, KHARVEX_PLANET_VISUAL_SCALE, KHARVEX_POSITION, KHARVEX_SURFACE_RADIUS,
        PLANET_MESH, PLANET_ORBIT_RADIUS, PLANET_SURFACE_RADIUS, PLANET_VISUAL_RADIUS,
        SUN_MODEL_CENTER, SUN_MODEL_MAX_DIMENSION, SUN_MODEL_MESH, SUN_MODEL_VISUAL_SCALE,
        SUN_POSITION, SUN_VISUAL_RADIUS, TESSARA_MOON_CENTER,
        TESSARA_MOON_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND, TESSARA_MOON_ORBIT_HEIGHT,
        TESSARA_MOON_ORBIT_RADIUS, TESSARA_MOON_VISUAL_SCALE, TESSARA_PLANET_CENTER,
        TESSARA_PLANET_MAX_DIMENSION, TESSARA_PLANET_MESH, TESSARA_PLANET_VISUAL_SCALE,
        TESSARA_POSITION,
    };
    use gameplay::{
        commands::GameplayCommand, domains::mission::MissionState, mission_system::MissionSystem,
        systems::GameplaySystem, types::MissionId, GameplayWorld,
    };
    use render_core::MeshHandle;
    use render_core::Quat;
    use render_core::{HudMissionState, Vec3};

    fn world_with_missions() -> (GameplayWorld, engine::EntityId) {
        let mut world = GameplayWorld::new();
        let player = world.create_player().expect("player should be created");
        world
            .register_mission(crate::el_despertar::definition().expect("awakening is valid"))
            .expect("awakening should register");
        world
            .register_mission(crate::ecos_anomalia::definition().expect("anomaly is valid"))
            .expect("anomaly should register");
        (world, player)
    }

    fn apply(system: &mut MissionSystem, world: &mut GameplayWorld, command: GameplayCommand) {
        let result = system
            .process(world, command)
            .expect("mission command should be handled");
        assert!(matches!(
            result.status,
            gameplay::CommandStatus::Success { .. }
        ));
    }

    fn unlock_and_start(system: &mut MissionSystem, world: &mut GameplayWorld, mission: MissionId) {
        apply(system, world, GameplayCommand::UnlockMission { mission });
        apply(system, world, GameplayCommand::StartMission { mission });
    }

    #[test]
    fn mission_state_conversion_preserves_all_domain_states() {
        let states = [
            (MissionState::Locked, HudMissionState::Locked),
            (MissionState::Available, HudMissionState::Available),
            (MissionState::Active, HudMissionState::Active),
            (MissionState::Failed, HudMissionState::Failed),
            (MissionState::Completed, HudMissionState::Completed),
        ];

        for (domain_state, hud_state) in states {
            assert_eq!(hud_mission_state(domain_state), hud_state);
        }
    }

    #[test]
    fn interior_view_has_a_player_camera_and_spatial_lighting() {
        let mut world = GameplayWorld::new();
        let player = world.create_player().expect("player should exist");
        let camera = super::interior_camera(&world, 16.0 / 9.0).expect("camera should be valid");
        assert_eq!(
            camera.position(),
            render_core::Vec3::new(0.0, super::ECHO7_INTERIOR_EYE_HEIGHT, 0.0)
        );

        let snapshot = super::snapshot_from_world_with_camera_and_view(
            &world,
            Default::default(),
            &camera,
            true,
        );
        assert!(snapshot
            .nodes
            .iter()
            .any(|node| { node.entity == render_core::SnapshotId::Entity(player) }));
        assert!(snapshot.lights.iter().any(|light| {
            matches!(
                light.light.kind,
                render_core::RenderLightKind::Point { position, range }
                    if position == render_core::Vec3::new(0.0, 5.6, 0.0)
                        && range == 18.0
            )
        }));
        assert_eq!(
            snapshot
                .lights
                .iter()
                .filter(|light| {
                    matches!(light.light.kind, render_core::RenderLightKind::Point { .. })
                })
                .count(),
            4
        );
    }

    #[test]
    fn echo7_z_up_conversion_maps_to_world_y_without_reflection() {
        let conversion = super::echo7_local_to_world_rotation();
        let right = conversion * render_core::Vec3::X;
        let forward = conversion * render_core::Vec3::Y;
        let up = conversion * render_core::Vec3::Z;

        assert!((up - render_core::Vec3::Y).length() < 1e-5);
        assert!((right.cross(forward) - up).length() < 1e-5);
    }

    #[test]
    fn interior_eye_height_matches_model_space_height() {
        let ship = engine::Vec3::ZERO;
        let feet = super::echo7_model_to_world(super::ECHO7_DESPERTAR_SPAWN_LOCAL, ship, 0.0);
        let eye = engine::Vec3 {
            x: feet.x,
            y: feet.y + super::ECHO7_INTERIOR_EYE_HEIGHT,
            z: feet.z,
        };
        let local_eye = super::echo7_player_model_position(eye, ship, 0.0);
        let delta = local_eye.z - super::ECHO7_DESPERTAR_SPAWN_LOCAL.z;
        assert!((delta - super::ECHO7_INTERIOR_EYE_HEIGHT).abs() < 1e-5);
    }

    #[test]
    fn echo7_walkable_surfaces_connect_both_decks_without_jumping_under_balcony() {
        assert_eq!(
            super::echo7_walkable_height(
                engine::Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                },
                engine::Vec3::ZERO,
                0.0
            ),
            0.0
        );
        assert!(
            super::echo7_walkable_height(
                engine::Vec3 {
                    x: -5.1,
                    y: 0.0,
                    z: 0.0
                },
                engine::Vec3::ZERO,
                0.0
            ) > 1.2
        );
        assert!(
            (super::echo7_walkable_height(
                engine::Vec3 {
                    x: -5.1,
                    y: 0.0,
                    z: 1.57
                },
                engine::Vec3::ZERO,
                0.0,
            ) - 2.73)
                .abs()
                < 0.01
        );
        assert_eq!(
            super::echo7_walkable_height(
                engine::Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 3.1
                },
                engine::Vec3::ZERO,
                0.0
            ),
            0.0
        );
        assert_eq!(
            super::echo7_walkable_height(
                engine::Vec3 {
                    x: 8.0,
                    y: 0.0,
                    z: 0.0
                },
                engine::Vec3::ZERO,
                0.0
            ),
            0.0
        );
    }

    #[test]
    fn echo7_balcony_is_transformed_ring_with_stair_connection_and_clearance() {
        let ship = engine::Vec3 {
            x: 13.0,
            y: 0.0,
            z: -4.0,
        };
        let yaw: f32 = 37.0;
        let rotation = Quat::from_rotation_y(yaw.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD);
        let at_local = |local: Vec3, height: f32| {
            let offset = rotation
                * (echo7_local_to_world_rotation()
                    * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)));
            engine::Vec3 {
                x: ship.x + offset.x,
                y: height,
                z: ship.z + offset.z,
            }
        };

        assert!(
            (super::echo7_walkable_height(at_local(Vec3::new(0.0, -9.0, 0.0), 2.7), ship, yaw)
                - ECHO7_UPPER_DECK_Y)
                .abs()
                < 1e-5
        );
        assert!(super::echo7_upper_or_stair_surface_contains(
            at_local(Vec3::new(0.0, -9.0, 0.0), 2.7),
            ship,
            yaw,
        ));
        assert!(!super::echo7_upper_or_stair_surface_contains(
            at_local(Vec3::new(0.0, 0.0, 0.0), 2.7),
            ship,
            yaw,
        ));
        assert!(!super::echo7_upper_or_stair_surface_contains(
            at_local(Vec3::new(0.0, -11.0, 0.0), 2.7),
            ship,
            yaw,
        ));
        assert_eq!(
            super::echo7_walkable_height(at_local(Vec3::ZERO, 2.7), ship, yaw),
            ECHO7_LOWER_DECK_Y
        );
        assert_eq!(
            super::echo7_walkable_height(at_local(Vec3::new(0.0, -11.0, 0.0), 2.7), ship, yaw),
            ECHO7_LOWER_DECK_Y
        );
        assert!(
            (super::echo7_walkable_height(at_local(Vec3::new(3.2, -7.0, 0.0), 2.7), ship, yaw,)
                - ECHO7_UPPER_DECK_Y)
                .abs()
                < 1e-5
        );
        assert_eq!(
            super::echo7_walkable_height(at_local(Vec3::new(3.2, -7.0, 0.0), 0.0), ship, yaw),
            ECHO7_LOWER_DECK_Y
        );
    }

    #[test]
    fn echo7_lower_deck_walls_keep_holes_and_follow_authoritative_door_state() {
        let ship = engine::Vec3 {
            x: 8.0,
            y: 0.0,
            z: -6.0,
        };
        let yaw: f32 = 29.0;
        let rotation = Quat::from_rotation_y(yaw.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD);
        let at_local = |local: Vec3| {
            let offset = rotation
                * (echo7_local_to_world_rotation()
                    * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)));
            engine::Vec3 {
                x: ship.x + offset.x,
                y: 0.0,
                z: ship.z + offset.z,
            }
        };
        let wall_door = at_local(Vec3::new(5.0, -7.0, 0.0));
        assert!(super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(0.0, 6.0, 0.0)),
            ship,
            yaw,
            Some(wall_door),
            false,
        ));
        assert!(!super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(5.0, 7.1, 0.0)),
            ship,
            yaw,
            Some(wall_door),
            false,
        ));
        assert!(!super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(8.4, 6.4, 0.0)),
            ship,
            yaw,
            Some(wall_door),
            false,
        ));
        assert!(super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(5.0, -7.2, 0.0)),
            ship,
            yaw,
            Some(wall_door),
            true,
        ));
    }

    #[test]
    fn echo7_lower_deck_furniture_blocks_large_families_without_closing_corridors() {
        let ship = engine::Vec3 {
            x: -7.0,
            y: 0.0,
            z: 11.0,
        };
        let yaw: f32 = -23.0;
        let rotation = Quat::from_rotation_y(yaw.to_radians() + ECHO7_MODEL_YAW_OFFSET_RAD);
        let at_local = |local: Vec3| {
            let offset = rotation
                * (echo7_local_to_world_rotation()
                    * (ECHO7_VISUAL_SCALE * (local - ECHO7_MODEL_CENTER)));
            engine::Vec3 {
                x: ship.x + offset.x,
                y: 0.0,
                z: ship.z + offset.z,
            }
        };
        for center in [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.5, 5.5, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(-5.0, 2.8, 0.0),
            Vec3::new(6.0, -2.8, 0.0),
            Vec3::new(-6.0, -2.5, 0.0),
        ] {
            assert!(!super::echo7_lower_deck_position_valid(
                at_local(center),
                ship,
                yaw,
                None,
                false,
            ));
        }
        assert!(super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(3.0, 4.0, 0.0)),
            ship,
            yaw,
            None,
            false,
        ));
        assert!(super::echo7_lower_deck_position_valid(
            at_local(Vec3::new(2.5, 0.0, 0.0)),
            ship,
            yaw,
            None,
            false,
        ));
    }

    #[test]
    fn lower_corridor_inspection_camera_is_independent_of_player_position() {
        let world = GameplayWorld::new();
        let camera = super::lower_corridor_inspection_camera(&world, 16.0 / 9.0)
            .expect("inspection camera should be valid");
        assert!(camera.position().is_finite());
        assert!(camera.target().is_finite());
        assert_ne!(camera.position(), camera.target());
    }

    #[test]
    fn interior_station_representation_is_distinct_from_exterior_station_model() {
        let mut world = GameplayWorld::new();
        let station = world
            .create_station(engine::Vec3::ZERO, 3.0, gameplay::StationType::Piloting)
            .expect("station should register");

        let interior = super::station_nodes_for_view(&world, true);
        let exterior = super::station_nodes_for_view(&world, false);
        assert_eq!(interior.len(), 1);
        assert_eq!(exterior.len(), 1 + super::EXTERIOR_STATION_IDS.len());
        assert_eq!(interior[0].entity, render_core::SnapshotId::Entity(station));
        assert_eq!(exterior[0].entity, render_core::SnapshotId::Entity(station));
        assert_eq!(interior[0].mesh, super::PLACEHOLDER_MESH);
        assert_eq!(interior[0].material, super::DESTINATION_MATERIAL);
        assert_eq!(exterior[0].mesh, super::STATION_MESH);
        assert_eq!(exterior[0].material, super::STATION_MATERIAL);
        assert!(exterior[1..].iter().all(|node| {
            node.mesh == super::STATION_MESH && node.material == super::STATION_MATERIAL
        }));
    }

    #[test]
    fn hud_objective_belongs_to_presented_active_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("El despertar"));
        assert_eq!(
            hud.objective_name.as_deref(),
            Some("Evaluar el estado de la nave")
        );
    }

    #[test]
    fn hud_exposes_authoritative_objective_list_progress_and_transition_notice() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let started = super::hud_state(&world, None);
        assert_eq!(started.mission_description.as_deref(), Some("La tripulación de ECHO-7 despierta tras un incidente desconocido y debe recuperar el control de la nave."));
        assert_eq!(
            started.mission_current_objective,
            Some(crate::el_despertar::EVALUAR_ESTADO_NAVE.0)
        );
        assert_eq!(started.mission_objectives.len(), 6);
        assert_eq!(
            started.mission_objectives[0].id,
            crate::el_despertar::EVALUAR_ESTADO_NAVE.0
        );
        assert_eq!(started.mission_objectives[0].progress, 0);
        assert_eq!(started.mission_objectives[0].required_progress, 1);
        assert_eq!(
            started.mission_notification.as_deref(),
            Some("NUEVA MISION")
        );

        apply(
            &mut system,
            &mut world,
            GameplayCommand::StartObjective {
                objective: crate::el_despertar::EVALUAR_ESTADO_NAVE,
            },
        );
        apply(
            &mut system,
            &mut world,
            GameplayCommand::CompleteObjective {
                objective: crate::el_despertar::EVALUAR_ESTADO_NAVE,
            },
        );

        let progressed = super::hud_state(&world, None);
        assert_eq!(
            progressed.mission_objectives[0].state,
            render_core::HudObjectiveState::Completed
        );
        assert_eq!(progressed.mission_objectives[0].progress, 0);
        assert_eq!(
            progressed.mission_current_objective,
            Some(crate::el_despertar::RESTAURAR_ENERGIA.0)
        );
        assert_eq!(
            progressed.objective_name.as_deref(),
            Some("Restaurar la energía")
        );
        assert_eq!(
            progressed.mission_notification.as_deref(),
            Some("OBJETIVO COMPLETADO")
        );
        assert_eq!(progressed, super::hud_state(&world, None));

        world.take_events();
        let quiet = super::hud_state(&world, None);
        assert_eq!(quiet.mission_objectives, progressed.mission_objectives);
        assert_eq!(
            quiet.mission_current_objective,
            progressed.mission_current_objective
        );
        assert!(quiet.mission_notification.is_none());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_available_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Available));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_active_mission_after_terminal_state() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        for objective in [
            crate::el_despertar::EVALUAR_ESTADO_NAVE,
            crate::el_despertar::RESTAURAR_ENERGIA,
            crate::el_despertar::RESTABLECER_SOPORTE_VITAL,
            crate::el_despertar::ACCEDER_PUENTE_MANDO,
            crate::el_despertar::TOMAR_CONTROL_NAVE,
        ] {
            apply(
                &mut system,
                &mut world,
                GameplayCommand::StartObjective { objective },
            );
            apply(
                &mut system,
                &mut world,
                GameplayCommand::CompleteObjective { objective },
            );
        }
        unlock_and_start(
            &mut system,
            &mut world,
            crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
        );

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Active));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_active_mission_after_failure() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        apply(
            &mut system,
            &mut world,
            GameplayCommand::FailMission {
                mission: crate::el_despertar::EL_DESPERTAR,
            },
        );
        unlock_and_start(
            &mut system,
            &mut world,
            crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
        );

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Active));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_keeps_optional_awakening_objective_within_same_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let hud = super::hud_state(&world, None);
        let guidance = world
            .guidance_for_player(_player)
            .expect("guidance should be available");

        assert_eq!(hud.mission_name.as_deref(), Some("El despertar"));
        assert!(guidance.objectives.iter().any(|objective| {
            objective.mission == crate::el_despertar::EL_DESPERTAR
                && !objective.required
                && objective.recommended_action.is_some()
        }));
        assert_eq!(
            hud.objective_name.as_deref(),
            Some("Evaluar el estado de la nave")
        );
    }

    #[test]
    fn presentation_prefers_active_mission_over_another_available_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        assert_eq!(
            super::presentation_mission(&world),
            Some(crate::el_despertar::EL_DESPERTAR)
        );
    }

    #[test]
    fn presentation_selects_first_available_in_campaign_order() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        assert_eq!(
            super::presentation_mission(&world),
            Some(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        );
    }

    #[test]
    fn hud_switches_to_available_successor_without_starting_it() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        for objective in [
            crate::el_despertar::EVALUAR_ESTADO_NAVE,
            crate::el_despertar::RESTAURAR_ENERGIA,
            crate::el_despertar::RESTABLECER_SOPORTE_VITAL,
            crate::el_despertar::ACCEDER_PUENTE_MANDO,
            crate::el_despertar::TOMAR_CONTROL_NAVE,
        ] {
            apply(
                &mut system,
                &mut world,
                GameplayCommand::StartObjective { objective },
            );
            apply(
                &mut system,
                &mut world,
                GameplayCommand::CompleteObjective { objective },
            );
        }
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        let hud = super::hud_state(&world, None);
        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Available));
        assert_eq!(
            hud.mission_notification.as_deref(),
            Some("MISION COMPLETADA")
        );
        assert_eq!(
            world
                .mission(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .unwrap()
                .state(),
            MissionState::Available
        );
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn visual_scale_policy_separates_celestial_and_physical_scales() {
        assert!(PLANET_VISUAL_RADIUS >= ECHO7_VISUAL_SCALE.max_element() * 2.0);
        assert!(SUN_VISUAL_RADIUS > PLANET_VISUAL_RADIUS);
        assert_eq!(PLANET_SURFACE_RADIUS, 5.0);
        assert!(PLANET_ORBIT_RADIUS > PLANET_VISUAL_RADIUS * 4.0);
        assert_eq!(
            academic_planet_transform(0.0).scale,
            Vec3::splat(PLANET_VISUAL_RADIUS)
        );
    }

    #[test]
    fn building_planet_mesh_is_limited_to_the_academic_planet() {
        let academic = gameplay::DestinationId::new(super::ACADEMIC_PLANET_DESTINATION_ID);
        let other = gameplay::DestinationId::new(9_003);

        let tessara = gameplay::DestinationId::new(super::TESSARA_DESTINATION_ID);
        let kharvex = gameplay::DestinationId::new(super::KHARVEX_DESTINATION_ID);
        assert_eq!(
            planet_mesh_for_destination(
                academic,
                Some(academic),
                Some(tessara),
                Some(kharvex),
                None,
                false
            ),
            BUILDING_PLANET_MESH
        );
        assert_eq!(
            planet_mesh_for_destination(
                tessara,
                Some(academic),
                Some(tessara),
                Some(kharvex),
                None,
                false
            ),
            TESSARA_PLANET_MESH
        );
        assert_eq!(
            planet_mesh_for_destination(
                kharvex,
                Some(academic),
                Some(tessara),
                Some(kharvex),
                None,
                false
            ),
            KHARVEX_PLANET_MESH
        );
        assert_eq!(
            planet_mesh_for_destination(
                other,
                Some(academic),
                Some(tessara),
                Some(kharvex),
                None,
                false
            ),
            PLANET_MESH
        );
        assert_eq!(
            planet_mesh_for_destination(academic, None, Some(tessara), Some(kharvex), None, false),
            PLANET_MESH
        );
    }

    #[test]
    fn kharvex_planet_transform_centers_and_preserves_uniform_visual_scale() {
        let transform = kharvex_planet_transform(KHARVEX_POSITION, 0.0);
        let centered =
            transform.translation + transform.rotation * (transform.scale * KHARVEX_PLANET_CENTER);

        assert!((centered - KHARVEX_POSITION).length() < 1e-5);
        assert_eq!(transform.scale, Vec3::splat(KHARVEX_PLANET_VISUAL_SCALE));
        assert!(
            (transform.scale.x * KHARVEX_PLANET_MAX_RADIUS - KHARVEX_SURFACE_RADIUS).abs() < 1e-5
        );
        assert!(transform.scale.x.is_finite() && transform.scale.x > 0.0);
    }

    #[test]
    fn kharvex_planet_transform_rotates_without_changing_center_or_scale() {
        let initial = kharvex_planet_transform(KHARVEX_POSITION, 0.0);
        let later = kharvex_planet_transform(KHARVEX_POSITION, 10.0);
        let initial_center =
            initial.translation + initial.rotation * (initial.scale * KHARVEX_PLANET_CENTER);
        let later_center =
            later.translation + later.rotation * (later.scale * KHARVEX_PLANET_CENTER);

        assert_ne!(initial.rotation, later.rotation);
        assert!((initial_center - KHARVEX_POSITION).length() < 1e-5);
        assert!((later_center - KHARVEX_POSITION).length() < 1e-5);
        assert_eq!(initial.scale, later.scale);
    }

    #[test]
    fn building_planet_transform_centers_and_preserves_visual_diameter() {
        let transform = building_planet_transform(0.0);
        let centered =
            transform.translation + transform.rotation * (transform.scale * BUILDING_PLANET_CENTER);
        let academic_position = academic_planet_transform(0.0).translation;

        assert!((centered - academic_position).length() < 1e-5);
        assert!((transform.scale.x * 15.45 - PLANET_VISUAL_RADIUS * 2.0).abs() < 1e-5);
    }

    #[test]
    fn tessara_transform_centers_at_its_static_position_and_preserves_visual_diameter() {
        let transform = tessara_planet_transform(TESSARA_POSITION, 0.0);
        let centered =
            transform.translation + transform.rotation * (transform.scale * TESSARA_PLANET_CENTER);

        assert!((centered - TESSARA_POSITION).length() < 1e-5);
        assert!(
            (transform.scale.x * TESSARA_PLANET_MAX_DIMENSION - PLANET_VISUAL_RADIUS * 2.0).abs()
                < 1e-5
        );
        assert_eq!(transform.scale, Vec3::splat(TESSARA_PLANET_VISUAL_SCALE));
    }

    #[test]
    fn tessara_planet_transform_rotates_without_changing_center_or_scale() {
        let initial = tessara_planet_transform(TESSARA_POSITION, 0.0);
        let later = tessara_planet_transform(TESSARA_POSITION, 10.0);
        let initial_center =
            initial.translation + initial.rotation * (initial.scale * TESSARA_PLANET_CENTER);
        let later_center =
            later.translation + later.rotation * (later.scale * TESSARA_PLANET_CENTER);

        assert_ne!(initial.rotation, later.rotation);
        assert!((initial_center - TESSARA_POSITION).length() < 1e-5);
        assert!((later_center - TESSARA_POSITION).length() < 1e-5);
        assert_eq!(initial.scale, later.scale);
    }

    #[test]
    fn tessara_moon_orbits_the_current_tessara_position() {
        let initial = tessara_moon_transform(TESSARA_POSITION, 0.0);
        let later = tessara_moon_transform(TESSARA_POSITION, 10.0);
        let initial_center =
            initial.translation + initial.rotation * (initial.scale * TESSARA_MOON_CENTER);
        let later_center = later.translation + later.rotation * (later.scale * TESSARA_MOON_CENTER);

        assert_ne!(initial_center, later_center);
        assert!(
            (initial_center
                - (TESSARA_POSITION
                    + Vec3::new(TESSARA_MOON_ORBIT_RADIUS, TESSARA_MOON_ORBIT_HEIGHT, 0.0,)))
            .length()
                < 1e-5
        );
        assert!(
            ((later_center - TESSARA_POSITION).length()
                - (TESSARA_MOON_ORBIT_RADIUS.powi(2) + TESSARA_MOON_ORBIT_HEIGHT.powi(2)).sqrt())
            .abs()
                < 1e-4
        );
    }

    #[test]
    fn tessara_moon_rotation_is_deterministic_and_centered() {
        let first = tessara_moon_transform(TESSARA_POSITION, 10.0);
        let second = tessara_moon_transform(TESSARA_POSITION, 10.0);
        let centered = first.translation + first.rotation * (first.scale * TESSARA_MOON_CENTER);

        assert_eq!(first, second);
        assert!(
            (centered
                - (TESSARA_POSITION
                    + Vec3::new(
                        TESSARA_MOON_ORBIT_RADIUS
                            * (TESSARA_MOON_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND * 10.0).cos(),
                        TESSARA_MOON_ORBIT_HEIGHT,
                        TESSARA_MOON_ORBIT_RADIUS
                            * (TESSARA_MOON_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND * 10.0).sin(),
                    )))
            .length()
                < 1e-4
        );
        assert_eq!(first.scale, Vec3::splat(TESSARA_MOON_VISUAL_SCALE));
    }

    #[test]
    fn sun_model_is_exclusive_and_centered_at_the_physical_sun_position() {
        let transform = super::sun_model_transform();
        let centered =
            transform.translation + transform.rotation * (transform.scale * SUN_MODEL_CENTER);

        assert_eq!(SUN_MODEL_MESH, MeshHandle::SUN_MODEL);
        assert!((centered - SUN_POSITION).length() < 1e-5);
        assert!(
            (transform.scale.x * SUN_MODEL_MAX_DIMENSION - SUN_VISUAL_RADIUS * 2.0).abs() < 1e-5
        );
        assert_eq!(transform.scale, Vec3::splat(SUN_MODEL_VISUAL_SCALE));
    }
}

use engine::{Duration, EntityId, Vec3};
use std::collections::BTreeMap;

use crate::resources::FuelTank;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DestinationId(u32);

impl DestinationId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

pub const SUN_DESTINATION_ID: DestinationId = DestinationId::new(9_000);
pub const VEYRA_DESTINATION_ID: DestinationId = DestinationId::new(9_002);
pub const TESSARA_DESTINATION_ID: DestinationId = DestinationId::new(9_601);

pub const fn is_free_navigation_destination(id: DestinationId) -> bool {
    matches!(id, VEYRA_DESTINATION_ID | TESSARA_DESTINATION_ID)
}

#[derive(Clone, Debug, PartialEq)]
pub enum FreeNavigationRisk {
    SolarCollision,
    PlanetCollision(DestinationId),
    InsufficientFuel { required: u32, available: u32 },
    IncompleteBodyData(DestinationId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreeNavigationEvaluation {
    Viable,
    NotViable,
    Incomplete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FreeNavigationProposal {
    pub destination: DestinationId,
    pub origin_position: Vec3,
    pub destination_position: Vec3,
    pub distance: f32,
    pub duration: Duration,
    pub fuel_required: u32,
    pub fuel_available: u32,
    pub evaluation: FreeNavigationEvaluation,
    pub risks: Vec<FreeNavigationRisk>,
    pub evaluated_at: Duration,
}

impl FreeNavigationProposal {
    pub fn is_stale(
        &self,
        now: Duration,
        current_position: Vec3,
        current_destination_position: Vec3,
        fuel_available: u32,
        destination_moves: bool,
    ) -> bool {
        self.origin_position != current_position
            || self.destination_position != current_destination_position
            || self.fuel_available != fuel_available
            || (destination_moves && self.evaluated_at != now)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestinationType {
    Planet,
    SpacePoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationRouteState {
    Available,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavigationRoute {
    pub from: DestinationId,
    pub to: DestinationId,
    pub distance: f32,
    pub duration: Duration,
    pub fuel_cost: u32,
    pub state: NavigationRouteState,
}

impl NavigationRoute {
    pub fn validate(self) -> Result<(), NavigationRouteError> {
        if self.from.value() == 0 || self.to.value() == 0 || self.from == self.to {
            return Err(NavigationRouteError::InvalidEndpoints);
        }
        if !self.distance.is_finite() || self.distance <= 0.0 {
            return Err(NavigationRouteError::InvalidDistance);
        }
        if self.duration.0 == 0 || self.fuel_cost == 0 {
            return Err(NavigationRouteError::InvalidTravelParameters);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationRouteError {
    InvalidEndpoints,
    InvalidDistance,
    InvalidTravelParameters,
    DuplicateRoute,
    DestinationNotFound,
    DestinationNotDiscovered,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutopilotState {
    Inactive,
    Planned {
        origin: DestinationId,
        destination: DestinationId,
        route: Vec<DestinationId>,
        eta: Duration,
    },
    InTransit {
        origin: DestinationId,
        destination: DestinationId,
        route: Vec<DestinationId>,
        next_leg: usize,
        eta: Duration,
        player: EntityId,
        pilot_station: EntityId,
    },
    Completed {
        destination: DestinationId,
    },
    Interrupted {
        destination: DestinationId,
    },
    Failed {
        destination: DestinationId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryState {
    Unknown,
    Known,
    Discovered,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SignalKnowledge {
    Unknown,
    Detected,
    Identified,
    Discovered,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalDetection {
    pub destination: DestinationId,
    pub distance: f32,
    pub knowledge: SignalKnowledge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalScanner {
    pub enabled: bool,
    pub range: f32,
    pub cooldown: Duration,
    pub last_scan: Option<Duration>,
}

impl Default for SignalScanner {
    fn default() -> Self {
        Self {
            enabled: true,
            range: 100.0,
            cooldown: Duration(1_000),
            last_scan: None,
        }
    }
}

impl SignalScanner {
    pub fn validate(self) -> Result<(), ScannerError> {
        if !self.range.is_finite() || self.range <= 0.0 || self.cooldown.0 == 0 {
            return Err(ScannerError::InvalidConfiguration);
        }
        Ok(())
    }

    pub fn cooldown_remaining(self, now: Duration) -> Duration {
        let Some(last) = self.last_scan else {
            return Duration(0);
        };
        let elapsed = now.0.saturating_sub(last.0);
        Duration(self.cooldown.0.saturating_sub(elapsed))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScannerError {
    InvalidConfiguration,
    Disabled,
    Cooldown,
    InvalidPosition,
    ShipNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Destination {
    pub id: DestinationId,
    pub destination_type: DestinationType,
    pub position: Vec3,
    pub travel_duration: Duration,
    pub fuel_cost: u32,
    pub discovery: DiscoveryState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetPhysics {
    pub destination: DestinationId,
    pub orbit_center: Vec3,
    pub orbit_radius: f32,
    pub orbit_start: Vec3,
    pub orbit_axis: Vec3,
    pub orbit_phase: f32,
    pub orbit_angular_speed: f32,
    pub surface_radius: f32,
    pub ship_radius: f32,
    pub gravity_influence_radius: f32,
    pub gravitational_parameter: f32,
    pub atmosphere_height: f32,
}

impl PlanetPhysics {
    pub fn validate(self) -> bool {
        finite_position(self.orbit_center)
            && self.orbit_radius.is_finite()
            && self.orbit_radius >= 0.0
            && finite_position(self.orbit_start)
            && finite_position(self.orbit_axis)
            && length_squared(self.orbit_axis) > 0.0
            && self.orbit_phase.is_finite()
            && self.orbit_angular_speed.is_finite()
            && self.surface_radius.is_finite()
            && self.surface_radius > 0.0
            && self.ship_radius.is_finite()
            && self.ship_radius >= 0.0
            && self.gravity_influence_radius.is_finite()
            && self.gravity_influence_radius > self.surface_radius
            && self.gravitational_parameter.is_finite()
            && self.gravitational_parameter > 0.0
            && self.atmosphere_height.is_finite()
            && self.atmosphere_height >= 0.0
    }

    pub fn position_at(self, elapsed: Duration) -> Vec3 {
        if self.orbit_radius == 0.0 {
            return self.orbit_center;
        }
        let angle = self.orbit_phase + self.orbit_angular_speed * elapsed.0 as f32 / 1_000.0;
        if self.orbit_start != Vec3::ZERO {
            let radial = sub_vec(self.orbit_start, self.orbit_center);
            let initial = scale_vec(normalize_vec(radial), self.orbit_radius);
            let tangent = scale_vec(
                normalize_vec(cross_vec(self.orbit_axis, initial)),
                self.orbit_radius,
            );
            return add_vec(
                self.orbit_center,
                add_vec(
                    scale_vec(initial, angle.cos()),
                    scale_vec(tangent, angle.sin()),
                ),
            );
        }
        add_vec(
            self.orbit_center,
            Vec3 {
                x: angle.cos() * self.orbit_radius,
                y: 0.0,
                z: angle.sin() * self.orbit_radius,
            },
        )
    }

    pub fn collision_radius(self) -> f32 {
        self.surface_radius + self.ship_radius
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanetFlightState {
    Free,
    Atmospheric {
        planet: DestinationId,
        intensity: f32,
    },
    Landing {
        planet: DestinationId,
    },
    Landed {
        planet: DestinationId,
    },
    Impacted {
        planet: DestinationId,
    },
}

impl PlanetFlightState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Landed { .. } | Self::Impacted { .. })
    }

    pub const fn landed_planet(self) -> Option<DestinationId> {
        match self {
            Self::Landed { planet } => Some(planet),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationRegistrationError {
    InvalidDestinationId,
    DuplicateDestination,
    InvalidPosition,
    InvalidDuration,
    InvalidFuelCost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationInitializationError {
    ShipNotFound,
    AlreadyInitialized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TravelState {
    Idle,
    InProgress {
        destination: DestinationId,
        started_at: Duration,
        duration: Duration,
        fuel_cost: u32,
    },
    Completed {
        destination: DestinationId,
    },
    Cancelled {
        destination: DestinationId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TravelProgress {
    pub elapsed: Duration,
    pub duration: Duration,
    pub fuel_cost: u32,
    pub progress: f32,
    pub distance_remaining: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipOrientation {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
}

impl Default for ShipOrientation {
    fn default() -> Self {
        Self {
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipMovementParameters {
    pub acceleration: f32,
    pub deceleration: f32,
    pub max_speed: f32,
    pub turn_rate_degrees: f32,
}

const MAX_GRAVITY_ACCELERATION: f32 = 0.55;
// Flight-assist margins keep manual travel forgiving without turning it into
// an autopilot. This is the same kind of safety envelope used by assisted
// supercruise/approach systems in space games: brake before the hard collider
// and bend away from bodies that are not the selected destination.
const COLLISION_ASSIST_BUFFER: f32 = 2.5;

impl Default for ShipMovementParameters {
    fn default() -> Self {
        Self {
            acceleration: 4.0,
            deceleration: 6.0,
            max_speed: 20.0,
            turn_rate_degrees: 75.0,
        }
    }
}

impl ShipMovementParameters {
    pub fn validate(self) -> Result<(), NavigationMovementError> {
        if !self.acceleration.is_finite()
            || !self.deceleration.is_finite()
            || !self.max_speed.is_finite()
            || !self.turn_rate_degrees.is_finite()
            || self.acceleration <= 0.0
            || self.deceleration <= 0.0
            || self.max_speed <= 0.0
            || self.turn_rate_degrees <= 0.0
        {
            return Err(NavigationMovementError::InvalidParameters);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationMovementError {
    InvalidParameters,
    InvalidInput,
    ShipNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationState {
    ship_id: Option<EntityId>,
    current_position: Vec3,
    selected_destination: Option<DestinationId>,
    free_navigation_destination: Option<DestinationId>,
    free_navigation_proposal: Option<FreeNavigationProposal>,
    travel_state: TravelState,
    destinations: BTreeMap<DestinationId, Destination>,
    orientation: ShipOrientation,
    velocity: Vec3,
    movement: ShipMovementParameters,
    control_active: bool,
    boost_active: bool,
    acceleration_input: Vec3,
    braking: bool,
    scanner: SignalScanner,
    signal_knowledge: BTreeMap<DestinationId, SignalKnowledge>,
    last_scan_results: Vec<SignalDetection>,
    routes: BTreeMap<(DestinationId, DestinationId), NavigationRoute>,
    autopilot: AutopilotState,
    autopilot_continuation_authorized: bool,
    planet_physics: BTreeMap<DestinationId, PlanetPhysics>,
    solar_physics: Option<PlanetPhysics>,
    planet_flight_state: PlanetFlightState,
    travel_origin: Vec3,
    impact_started_at: Option<Duration>,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            ship_id: None,
            current_position: Vec3::ZERO,
            selected_destination: None,
            free_navigation_destination: None,
            free_navigation_proposal: None,
            travel_state: TravelState::Idle,
            destinations: BTreeMap::new(),
            orientation: ShipOrientation::default(),
            velocity: Vec3::ZERO,
            movement: ShipMovementParameters::default(),
            control_active: false,
            boost_active: false,
            acceleration_input: Vec3::ZERO,
            braking: false,
            scanner: SignalScanner::default(),
            signal_knowledge: BTreeMap::new(),
            last_scan_results: Vec::new(),
            routes: BTreeMap::new(),
            autopilot: AutopilotState::Inactive,
            autopilot_continuation_authorized: false,
            planet_physics: BTreeMap::new(),
            solar_physics: None,
            planet_flight_state: PlanetFlightState::Free,
            travel_origin: Vec3::ZERO,
            impact_started_at: None,
        }
    }
}

impl NavigationState {
    pub(crate) fn restore_destinations_and_routes(
        &mut self,
        destinations: Vec<Destination>,
        routes: Vec<NavigationRoute>,
    ) -> Result<(), NavigationRouteError> {
        let mut next_destinations = BTreeMap::new();
        for destination in destinations {
            if destination.id.value() == 0
                || !finite_position(destination.position)
                || destination.travel_duration.0 == 0
                || destination.fuel_cost == 0
                || next_destinations
                    .insert(destination.id, destination)
                    .is_some()
            {
                return Err(NavigationRouteError::InvalidEndpoints);
            }
        }
        let mut next_routes = BTreeMap::new();
        for route in routes {
            route.validate()?;
            if !next_destinations.contains_key(&route.from)
                || !next_destinations.contains_key(&route.to)
                || next_routes.insert((route.from, route.to), route).is_some()
            {
                return Err(NavigationRouteError::DestinationNotFound);
            }
        }
        self.destinations = next_destinations;
        self.routes = next_routes;
        self.selected_destination = None;
        self.free_navigation_destination = None;
        self.free_navigation_proposal = None;
        self.travel_state = TravelState::Idle;
        self.autopilot = AutopilotState::Inactive;
        Ok(())
    }

    pub(crate) fn restore_signal_knowledge(
        &mut self,
        destination: DestinationId,
        knowledge: SignalKnowledge,
    ) {
        if !self.destinations.contains_key(&destination) {
            return;
        }
        if knowledge == SignalKnowledge::Unknown {
            self.signal_knowledge.remove(&destination);
        } else {
            self.signal_knowledge.insert(destination, knowledge);
        }
    }

    pub(crate) fn restore_position(
        &mut self,
        position: Vec3,
    ) -> Result<(), NavigationMovementError> {
        if !finite_position(position) {
            return Err(NavigationMovementError::InvalidInput);
        }
        self.current_position = position;
        Ok(())
    }

    pub fn register_destination(
        &mut self,
        destination: Destination,
    ) -> Result<(), NavigationRegistrationError> {
        if destination.id.value() == 0 {
            return Err(NavigationRegistrationError::InvalidDestinationId);
        }
        if self.destinations.contains_key(&destination.id) {
            return Err(NavigationRegistrationError::DuplicateDestination);
        }
        if !finite_position(destination.position) {
            return Err(NavigationRegistrationError::InvalidPosition);
        }
        if destination.travel_duration.0 == 0 {
            return Err(NavigationRegistrationError::InvalidDuration);
        }
        if destination.fuel_cost == 0 {
            return Err(NavigationRegistrationError::InvalidFuelCost);
        }
        self.destinations.insert(destination.id, destination);
        Ok(())
    }

    pub fn ship_id(&self) -> Option<EntityId> {
        self.ship_id
    }

    pub fn current_position(&self) -> Vec3 {
        self.current_position
    }

    pub fn configure_planet_physics(&mut self, physics: PlanetPhysics) -> bool {
        if !physics.validate()
            || physics.orbit_radius < 0.0
            || self.destination(physics.destination).is_none()
        {
            return false;
        }
        self.planet_physics.insert(physics.destination, physics);
        true
    }

    pub fn configure_solar_physics(&mut self, physics: PlanetPhysics) -> bool {
        if !physics.validate()
            || physics.orbit_radius != 0.0
            || physics.destination != SUN_DESTINATION_ID
        {
            return false;
        }
        self.solar_physics = Some(physics);
        true
    }

    pub fn planet_physics(&self, destination: DestinationId) -> Option<PlanetPhysics> {
        self.planet_physics.get(&destination).copied()
    }

    pub fn solar_physics(&self) -> Option<PlanetPhysics> {
        self.solar_physics
    }

    pub fn planet_position_at(
        &self,
        destination: DestinationId,
        elapsed: Duration,
    ) -> Option<Vec3> {
        self.planet_physics
            .get(&destination)
            .copied()
            .map(|physics| physics.position_at(elapsed))
    }

    pub const fn planet_flight_state(&self) -> PlanetFlightState {
        self.planet_flight_state
    }

    pub fn impact_flash_intensity(&self, now: Duration) -> Option<f32> {
        let started = self.impact_started_at?;
        let elapsed = now.0.saturating_sub(started.0);
        if elapsed >= 250 {
            return None;
        }
        Some(1.0 - elapsed as f32 / 250.0)
    }

    pub fn impact_fade_to_black(&self, now: Duration) -> Option<f32> {
        let started = self.impact_started_at?;
        Some((now.0.saturating_sub(started.0) as f32 / 1_000.0).clamp(0.0, 1.0))
    }

    pub fn landing_available(&self, destination: DestinationId, now: Duration) -> bool {
        let Some(physics) = self.planet_physics.get(&destination).copied() else {
            return false;
        };
        if self.planet_flight_state != PlanetFlightState::Free
            && !matches!(
                self.planet_flight_state,
                PlanetFlightState::Atmospheric { .. }
            )
        {
            return false;
        }
        let planet = physics.position_at(now);
        let distance = distance(self.current_position, planet);
        let landing_radius = physics.collision_radius() + physics.atmosphere_height;
        if distance <= landing_radius && dot(self.velocity, self.velocity) <= 36.0 {
            true
        } else {
            false
        }
    }

    pub fn request_landing(&mut self, destination: DestinationId, now: Duration) -> bool {
        if !self.landing_available(destination, now) {
            return false;
        }
        self.planet_flight_state = PlanetFlightState::Landing {
            planet: destination,
        };
        true
    }

    pub const fn orientation(&self) -> ShipOrientation {
        self.orientation
    }
    pub const fn velocity(&self) -> Vec3 {
        self.velocity
    }
    pub const fn movement_parameters(&self) -> ShipMovementParameters {
        self.movement
    }
    pub const fn control_active(&self) -> bool {
        self.control_active
    }
    pub const fn boost_active(&self) -> bool {
        self.boost_active
    }

    pub(crate) fn resume_from_landing(&mut self) -> bool {
        if !matches!(self.planet_flight_state, PlanetFlightState::Landed { .. }) {
            return false;
        }
        self.planet_flight_state = PlanetFlightState::Free;
        self.travel_state = TravelState::Idle;
        self.braking = false;
        self.velocity = Vec3::ZERO;
        true
    }
    pub fn speed_limit(&self) -> f32 {
        if self.boost_active {
            self.movement.max_speed * 1.5
        } else {
            self.movement.max_speed
        }
    }
    pub fn is_at_rest(&self) -> bool {
        self.velocity.x == 0.0 && self.velocity.y == 0.0 && self.velocity.z == 0.0
    }

    pub(crate) fn configure_movement(
        &mut self,
        parameters: ShipMovementParameters,
    ) -> Result<(), NavigationMovementError> {
        parameters.validate()?;
        self.movement = parameters;
        self.velocity = clamp_speed(self.velocity, parameters.max_speed);
        Ok(())
    }

    pub(crate) fn set_control_active(&mut self, active: bool) -> bool {
        if self.control_active == active {
            return false;
        }
        self.control_active = active;
        if !active {
            self.acceleration_input = Vec3::ZERO;
            self.boost_active = false;
        }
        true
    }

    pub(crate) fn set_boost_active(&mut self, active: bool) -> bool {
        if self.boost_active == active {
            return false;
        }
        self.boost_active = active;
        self.velocity = clamp_speed(self.velocity, self.speed_limit());
        true
    }

    pub(crate) fn set_acceleration_input(
        &mut self,
        input: Vec3,
    ) -> Result<bool, NavigationMovementError> {
        if !finite_vec3(input) {
            return Err(NavigationMovementError::InvalidInput);
        }
        let magnitude = magnitude(input);
        if !magnitude.is_finite() {
            return Err(NavigationMovementError::InvalidInput);
        }
        let normalized = if magnitude > 1.0 {
            Vec3 {
                x: input.x / magnitude,
                y: input.y / magnitude,
                z: input.z / magnitude,
            }
        } else {
            input
        };
        if self.acceleration_input == normalized && !self.braking {
            return Ok(false);
        }
        self.acceleration_input = normalized;
        self.braking = false;
        Ok(true)
    }

    pub(crate) fn rotate(
        &mut self,
        yaw_input: f32,
        pitch_input: f32,
        delta: Duration,
    ) -> Result<bool, NavigationMovementError> {
        if !yaw_input.is_finite()
            || !pitch_input.is_finite()
            || yaw_input.abs() > 1.0
            || pitch_input.abs() > 1.0
            || delta.0 == 0
        {
            return Err(NavigationMovementError::InvalidInput);
        }
        let seconds = delta.0 as f32 / 1000.0;
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(NavigationMovementError::InvalidInput);
        }
        let next = ShipOrientation {
            yaw_degrees: normalize_yaw(
                self.orientation.yaw_degrees
                    + yaw_input * self.movement.turn_rate_degrees * seconds,
            ),
            pitch_degrees: (self.orientation.pitch_degrees
                + pitch_input * self.movement.turn_rate_degrees * seconds)
                .clamp(-89.0, 89.0),
        };
        if next == self.orientation {
            return Ok(false);
        }
        self.orientation = next;
        Ok(true)
    }

    pub(crate) fn brake(&mut self) -> bool {
        let changed = !self.braking || self.acceleration_input != Vec3::ZERO;
        self.braking = true;
        self.acceleration_input = Vec3::ZERO;
        changed
    }

    pub(crate) fn advance_manual(
        &mut self,
        delta: Duration,
        now: Duration,
    ) -> Option<(Vec3, Vec3, bool, Option<(DestinationId, bool)>)> {
        if delta.0 == 0 || self.has_active_travel() || self.planet_flight_state.is_terminal() {
            return None;
        }
        if let PlanetFlightState::Landing { planet } = self.planet_flight_state {
            self.velocity = Vec3::ZERO;
            self.planet_flight_state = PlanetFlightState::Landed { planet };
            return Some((Vec3::ZERO, Vec3::ZERO, false, Some((planet, false))));
        }
        if !self.control_active {
            return None;
        }
        let seconds = delta.0 as f32 / 1000.0;
        if !seconds.is_finite() || seconds <= 0.0 {
            return None;
        }
        let previous_velocity = self.velocity;
        let previous_position = self.current_position;
        if self.braking {
            self.velocity = move_towards_zero(self.velocity, self.movement.deceleration * seconds);
            if self.is_at_rest() {
                self.braking = false;
            }
        } else {
            self.velocity = add_velocity(
                self.velocity,
                self.acceleration_input,
                self.movement.acceleration * if self.boost_active { 1.5 } else { 1.0 } * seconds,
            );
            self.velocity = clamp_speed(self.velocity, self.speed_limit());
        }
        let mut atmospheric = None;
        let solar_physics = self.solar_physics;
        for (destination, physics) in &self.planet_physics {
            let planet = physics.position_at(now);
            let offset = sub_vec(planet, self.current_position);
            let distance = dot(offset, offset).sqrt();
            if distance > 0.0 && distance < physics.gravity_influence_radius {
                let direction = Vec3 {
                    x: offset.x / distance,
                    y: offset.y / distance,
                    z: offset.z / distance,
                };
                let denominator = distance.max(physics.collision_radius()).powi(2);
                let gravity =
                    (physics.gravitational_parameter / denominator).min(MAX_GRAVITY_ACCELERATION);
                self.velocity = add_vec(self.velocity, scale_vec(direction, gravity * seconds));
                self.velocity = clamp_speed(self.velocity, self.speed_limit());
            }
            let atmosphere_radius = physics.collision_radius() + physics.atmosphere_height;
            if distance <= atmosphere_radius && distance > physics.collision_radius() {
                let intensity = if physics.atmosphere_height > 0.0 {
                    (1.0 - (distance - physics.collision_radius()) / physics.atmosphere_height)
                        .clamp(0.0, 1.0)
                } else {
                    1.0
                };
                atmospheric = Some((*destination, intensity));
            }
        }
        if let Some(physics) = solar_physics {
            let solar_position = physics.position_at(now);
            let offset = sub_vec(solar_position, self.current_position);
            let distance = dot(offset, offset).sqrt();
            if distance > 0.0 && distance < physics.gravity_influence_radius {
                let direction = Vec3 {
                    x: offset.x / distance,
                    y: offset.y / distance,
                    z: offset.z / distance,
                };
                let denominator = distance.max(physics.collision_radius()).powi(2);
                let gravity =
                    (physics.gravitational_parameter / denominator).min(MAX_GRAVITY_ACCELERATION);
                self.velocity = add_vec(self.velocity, scale_vec(direction, gravity * seconds));
                self.velocity = clamp_speed(self.velocity, self.speed_limit());
            }
        }
        apply_collision_assist(
            &mut self.velocity,
            previous_position,
            self.free_navigation_destination
                .or(self.selected_destination),
            &self.planet_physics,
            solar_physics,
            self.movement.deceleration,
            seconds,
            now,
        );
        let displacement = Vec3 {
            x: self.velocity.x * seconds,
            y: self.velocity.y * seconds,
            z: self.velocity.z * seconds,
        };
        let next_position = Vec3 {
            x: self.current_position.x + displacement.x,
            y: self.current_position.y + displacement.y,
            z: self.current_position.z + displacement.z,
        };
        let mut contact = None;
        for (destination, physics) in &self.planet_physics {
            let planet = physics.position_at(now);
            if segment_intersects_sphere(
                previous_position,
                next_position,
                planet,
                physics.collision_radius(),
            ) {
                self.velocity = Vec3::ZERO;
                self.planet_flight_state = PlanetFlightState::Impacted {
                    planet: *destination,
                };
                self.impact_started_at = Some(now);
                contact = Some((*destination, true));
                break;
            }
        }
        if contact.is_none() {
            if let Some(physics) = solar_physics {
                let solar_position = physics.position_at(now);
                if segment_intersects_sphere(
                    previous_position,
                    next_position,
                    solar_position,
                    physics.collision_radius(),
                ) {
                    self.velocity = Vec3::ZERO;
                    self.current_position = impact_position(
                        previous_position,
                        solar_position,
                        physics.collision_radius(),
                    );
                    self.planet_flight_state = PlanetFlightState::Impacted {
                        planet: SUN_DESTINATION_ID,
                    };
                    self.impact_started_at = Some(now);
                    contact = Some((SUN_DESTINATION_ID, true));
                }
            }
        }
        if contact.is_none() && finite_vec3(next_position) {
            self.current_position = next_position;
            self.planet_flight_state = atmospheric
                .map_or(PlanetFlightState::Free, |(planet, intensity)| {
                    PlanetFlightState::Atmospheric { planet, intensity }
                });
        }
        Some((
            previous_velocity,
            self.velocity,
            self.current_position != previous_position,
            contact.or(atmospheric.map(|(planet, _)| (planet, false))),
        ))
    }

    pub(crate) fn advance_travel_at(
        &mut self,
        now: Duration,
    ) -> Option<(DestinationId, Vec3, bool)> {
        let TravelState::InProgress {
            destination,
            started_at,
            duration,
            ..
        } = self.travel_state
        else {
            return None;
        };
        let Some(physics) = self.planet_physics.get(&destination).copied() else {
            return None;
        };
        let elapsed = now.0.saturating_sub(started_at.0);
        let progress = (elapsed as f32 / duration.0.max(1) as f32).clamp(0.0, 1.0);
        let target = physics.position_at(now);
        let proposed = add_vec(
            self.travel_origin,
            scale_vec(sub_vec(target, self.travel_origin), progress),
        );
        if let Some(solar) = self.solar_physics {
            let solar_position = solar.position_at(now);
            if segment_intersects_sphere(
                self.current_position,
                proposed,
                solar_position,
                solar.collision_radius(),
            ) {
                self.current_position = impact_position(
                    self.current_position,
                    solar_position,
                    solar.collision_radius(),
                );
                self.velocity = Vec3::ZERO;
                self.planet_flight_state = PlanetFlightState::Impacted {
                    planet: SUN_DESTINATION_ID,
                };
                self.impact_started_at = Some(now);
                self.travel_state = TravelState::Cancelled { destination };
                return Some((SUN_DESTINATION_ID, self.current_position, true));
            }
        }
        for (planet_id, physics) in &self.planet_physics {
            if *planet_id == destination {
                continue;
            }
            let planet_position = physics.position_at(now);
            if segment_intersects_sphere(
                self.current_position,
                proposed,
                planet_position,
                physics.collision_radius(),
            ) {
                self.current_position = impact_position(
                    self.current_position,
                    planet_position,
                    physics.collision_radius(),
                );
                self.velocity = Vec3::ZERO;
                self.planet_flight_state = PlanetFlightState::Impacted { planet: *planet_id };
                self.impact_started_at = Some(now);
                self.travel_state = TravelState::Cancelled { destination };
                return Some((*planet_id, self.current_position, true));
            }
        }
        let planet = target;
        let radius = physics.collision_radius();
        if matches!(
            self.planet_flight_state,
            PlanetFlightState::Landing { planet: active } if active == destination
        ) && distance(proposed, planet) <= radius + physics.atmosphere_height
        {
            let direction = sub_vec(planet, self.travel_origin);
            let length = distance(self.travel_origin, planet).max(radius);
            self.current_position = sub_vec(planet, scale_vec(direction, radius / length));
            self.velocity = Vec3::ZERO;
            self.planet_flight_state = PlanetFlightState::Landed {
                planet: destination,
            };
            self.travel_state = TravelState::Completed { destination };
            if matches!(
                self.autopilot,
                AutopilotState::InTransit { ref route, next_leg: 1, .. }
                    if route.len() == 2
            ) {
                self.autopilot = AutopilotState::Completed { destination };
            }
            return Some((destination, self.current_position, false));
        }
        if segment_intersects_sphere(self.current_position, proposed, planet, radius) {
            let direction = sub_vec(planet, self.current_position);
            let length = distance(self.current_position, planet).max(radius);
            self.current_position = sub_vec(planet, scale_vec(direction, radius / length));
            self.velocity = Vec3::ZERO;
            self.planet_flight_state = PlanetFlightState::Impacted {
                planet: destination,
            };
            self.impact_started_at = Some(now);
            self.travel_state = TravelState::Cancelled { destination };
            return Some((destination, self.current_position, true));
        }
        self.current_position = proposed;
        let distance_to_planet = distance(self.current_position, planet);
        if distance_to_planet <= radius + physics.atmosphere_height && distance_to_planet > radius {
            let intensity = if physics.atmosphere_height > 0.0 {
                (1.0 - (distance_to_planet - radius) / physics.atmosphere_height).clamp(0.0, 1.0)
            } else {
                1.0
            };
            self.planet_flight_state = PlanetFlightState::Atmospheric {
                planet: destination,
                intensity,
            };
        }
        None
    }

    pub fn destinations(&self) -> impl Iterator<Item = &Destination> {
        self.destinations.values()
    }

    pub fn routes(&self) -> impl Iterator<Item = &NavigationRoute> {
        self.routes.values()
    }

    pub fn route(&self, from: DestinationId, to: DestinationId) -> Option<&NavigationRoute> {
        self.routes.get(&(from, to))
    }

    pub fn available_routes_from(
        &self,
        from: DestinationId,
    ) -> impl Iterator<Item = &NavigationRoute> {
        self.routes.values().filter(move |route| {
            route.from == from && route.state == NavigationRouteState::Available
        })
    }

    pub fn autopilot_state(&self) -> &AutopilotState {
        &self.autopilot
    }

    pub(crate) const fn autopilot_continuation_authorized(&self) -> bool {
        self.autopilot_continuation_authorized
    }

    pub(crate) fn set_autopilot_continuation_authorized(&mut self, authorized: bool) {
        self.autopilot_continuation_authorized = authorized;
    }

    pub fn destination(&self, id: DestinationId) -> Option<&Destination> {
        self.destinations.get(&id)
    }

    pub fn selected_destination(&self) -> Option<DestinationId> {
        self.selected_destination
    }

    pub fn free_navigation_destination(&self) -> Option<DestinationId> {
        self.free_navigation_destination
    }

    pub fn free_navigation_proposal(&self) -> Option<&FreeNavigationProposal> {
        self.free_navigation_proposal.as_ref()
    }

    pub(crate) fn set_free_navigation_proposal(&mut self, proposal: FreeNavigationProposal) {
        self.free_navigation_proposal = Some(proposal);
    }

    pub const fn travel_state(&self) -> TravelState {
        self.travel_state
    }

    pub const fn has_active_travel(&self) -> bool {
        matches!(self.travel_state, TravelState::InProgress { .. })
    }

    pub const fn scanner(&self) -> SignalScanner {
        self.scanner
    }
    pub fn last_scan_results(&self) -> &[SignalDetection] {
        &self.last_scan_results
    }
    pub fn signal_knowledge(&self, destination: DestinationId) -> SignalKnowledge {
        if self
            .destinations
            .get(&destination)
            .is_some_and(|value| value.discovery == DiscoveryState::Discovered)
        {
            return SignalKnowledge::Discovered;
        }
        if let Some(value) = self.signal_knowledge.get(&destination) {
            return *value;
        }
        match self
            .destinations
            .get(&destination)
            .map(|value| value.discovery)
        {
            Some(DiscoveryState::Known) => SignalKnowledge::Identified,
            _ => SignalKnowledge::Unknown,
        }
    }
    pub fn identified_destinations(&self) -> impl Iterator<Item = DestinationId> + '_ {
        self.destinations.keys().copied().filter(|destination| {
            matches!(
                self.signal_knowledge(*destination),
                SignalKnowledge::Identified | SignalKnowledge::Discovered
            )
        })
    }

    pub(crate) fn configure_scanner(
        &mut self,
        range: f32,
        cooldown: Duration,
    ) -> Result<(), ScannerError> {
        let scanner = SignalScanner {
            enabled: self.scanner.enabled,
            range,
            cooldown,
            last_scan: self.scanner.last_scan,
        };
        scanner.validate()?;
        self.scanner = scanner;
        Ok(())
    }

    pub(crate) fn set_scanner_enabled(&mut self, enabled: bool) -> bool {
        if self.scanner.enabled == enabled {
            return false;
        }
        self.scanner.enabled = enabled;
        true
    }

    pub(crate) fn scan_signals(
        &mut self,
        now: Duration,
    ) -> Result<Vec<(DestinationId, SignalKnowledge, SignalKnowledge, f32)>, ScannerError> {
        self.scanner.validate()?;
        if !self.scanner.enabled {
            return Err(ScannerError::Disabled);
        }
        if self.scanner.cooldown_remaining(now).0 > 0 {
            return Err(ScannerError::Cooldown);
        }
        if !finite_position(self.current_position) {
            return Err(ScannerError::InvalidPosition);
        }
        let range_squared = self.scanner.range as f64 * self.scanner.range as f64;
        let mut candidates = Vec::new();
        for destination in self.destinations.values() {
            let squared = distance_squared(self.current_position, destination.position);
            if !squared.is_finite() {
                continue;
            }
            if squared <= range_squared {
                candidates.push((destination.id, squared.sqrt().min(f32::MAX as f64) as f32));
            }
        }
        candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_id.cmp(right_id))
        });
        let mut transitions = Vec::new();
        self.last_scan_results.clear();
        for (destination, distance) in candidates {
            let previous = self.signal_knowledge(destination);
            let next = match previous {
                SignalKnowledge::Unknown => SignalKnowledge::Detected,
                SignalKnowledge::Detected => SignalKnowledge::Identified,
                SignalKnowledge::Identified | SignalKnowledge::Discovered => previous,
            };
            if next != previous {
                self.signal_knowledge.insert(destination, next);
                transitions.push((destination, previous, next, distance));
            }
            self.last_scan_results.push(SignalDetection {
                destination,
                distance,
                knowledge: next,
            });
        }
        self.scanner.last_scan = Some(now);
        Ok(transitions)
    }

    pub fn travel_progress(&self, now: Duration) -> Option<TravelProgress> {
        let TravelState::InProgress {
            destination,
            started_at,
            duration,
            fuel_cost,
        } = self.travel_state
        else {
            return None;
        };
        let elapsed = Duration(now.0.saturating_sub(started_at.0));
        let progress = ((elapsed.0 as f64) / (duration.0 as f64)).clamp(0.0, 1.0) as f32;
        let distance_remaining = self
            .destinations
            .get(&destination)
            .map(|destination| {
                (distance(self.current_position, destination.position) * (1.0 - progress))
                    .min(f32::MAX)
            })
            .unwrap_or(0.0);
        Some(TravelProgress {
            elapsed,
            duration,
            fuel_cost,
            progress,
            distance_remaining,
        })
    }

    pub(crate) fn initialize_ship(
        &mut self,
        ship_id: EntityId,
    ) -> Result<(), NavigationInitializationError> {
        if self.ship_id.is_some() {
            return Err(NavigationInitializationError::AlreadyInitialized);
        }
        self.ship_id = Some(ship_id);
        Ok(())
    }

    pub(crate) fn select_destination(
        &mut self,
        destination: DestinationId,
    ) -> Result<(), NavigationSelectionError> {
        if self.has_active_travel() {
            return Err(NavigationSelectionError::TravelActive);
        }
        if matches!(
            self.autopilot,
            AutopilotState::Planned { .. } | AutopilotState::InTransit { .. }
        ) {
            return Err(NavigationSelectionError::AutopilotActive);
        }
        let Some(destination_data) = self.destinations.get(&destination) else {
            return Err(NavigationSelectionError::DestinationNotFound);
        };
        if destination_data.discovery == DiscoveryState::Unknown {
            return Err(NavigationSelectionError::DestinationUnknown);
        }
        self.selected_destination = Some(destination);
        Ok(())
    }

    pub(crate) fn confirm_free_navigation_destination(
        &mut self,
        destination: DestinationId,
        now: Duration,
    ) -> Result<(), FreeNavigationSelectionError> {
        if self.has_active_travel() {
            return Err(FreeNavigationSelectionError::TravelActive);
        }
        if matches!(
            self.autopilot,
            AutopilotState::Planned { .. } | AutopilotState::InTransit { .. }
        ) {
            return Err(FreeNavigationSelectionError::AutopilotActive);
        }
        let Some(destination_data) = self.destinations.get(&destination) else {
            return Err(FreeNavigationSelectionError::DestinationNotFound);
        };
        if destination_data.destination_type != DestinationType::Planet {
            return Err(FreeNavigationSelectionError::DestinationNotPlanet);
        }
        if !is_free_navigation_destination(destination) {
            return Err(FreeNavigationSelectionError::DestinationNotAllowed);
        }
        if destination_data.discovery == DiscoveryState::Unknown {
            return Err(FreeNavigationSelectionError::DestinationUnknown);
        }
        let target_position = self
            .planet_position_at(destination, now)
            .unwrap_or(destination_data.position);
        if target_position == self.current_position {
            return Err(FreeNavigationSelectionError::DestinationIsCurrentPosition);
        }
        self.free_navigation_destination = Some(destination);
        self.free_navigation_proposal = None;
        Ok(())
    }

    pub(crate) fn evaluate_free_navigation(
        &self,
        now: Duration,
        fuel_available: u32,
    ) -> Result<FreeNavigationProposal, FreeNavigationEvaluationError> {
        let Some(destination) = self.free_navigation_destination else {
            return Err(FreeNavigationEvaluationError::DestinationNotSelected);
        };
        let Some(destination_data) = self.destinations.get(&destination) else {
            return Err(FreeNavigationEvaluationError::DestinationNotFound);
        };
        if destination_data.destination_type != DestinationType::Planet
            || !is_free_navigation_destination(destination)
        {
            return Err(FreeNavigationEvaluationError::DestinationNotAllowed);
        }
        if destination_data.discovery == DiscoveryState::Unknown {
            return Err(FreeNavigationEvaluationError::DestinationUnknown);
        }
        let origin_position = self.current_position;
        let destination_position = self
            .planet_position_at(destination, now)
            .unwrap_or(destination_data.position);
        let distance = distance(origin_position, destination_position);
        if !distance.is_finite() || distance < 0.0 {
            return Err(FreeNavigationEvaluationError::InvalidDistance);
        }
        let duration = destination_data.travel_duration;
        if duration.0 == 0 || destination_data.fuel_cost == 0 {
            return Err(FreeNavigationEvaluationError::InvalidTravelConfiguration);
        }

        let mut risks = Vec::new();
        if let Some(solar) = self.solar_physics {
            if segment_intersects_sphere(
                origin_position,
                destination_position,
                solar.position_at(now),
                solar.collision_radius(),
            ) {
                risks.push(FreeNavigationRisk::SolarCollision);
            }
        }
        for (planet_id, physics) in &self.planet_physics {
            if *planet_id == destination {
                continue;
            }
            if segment_intersects_sphere(
                origin_position,
                destination_position,
                physics.position_at(now),
                physics.collision_radius(),
            ) {
                risks.push(FreeNavigationRisk::PlanetCollision(*planet_id));
            }
        }
        if !self.planet_physics.contains_key(&destination) {
            risks.push(FreeNavigationRisk::IncompleteBodyData(destination));
        }
        for registered in self.destinations.values() {
            if registered.destination_type == DestinationType::Planet
                && registered.id != destination
                && !self.planet_physics.contains_key(&registered.id)
            {
                risks.push(FreeNavigationRisk::IncompleteBodyData(registered.id));
            }
        }
        if fuel_available < destination_data.fuel_cost {
            risks.push(FreeNavigationRisk::InsufficientFuel {
                required: destination_data.fuel_cost,
                available: fuel_available,
            });
        }
        let hard_risk = risks.iter().any(|risk| {
            matches!(
                risk,
                FreeNavigationRisk::SolarCollision
                    | FreeNavigationRisk::PlanetCollision(_)
                    | FreeNavigationRisk::InsufficientFuel { .. }
            )
        });
        let incomplete = risks
            .iter()
            .any(|risk| matches!(risk, FreeNavigationRisk::IncompleteBodyData(_)));
        let evaluation = if hard_risk {
            FreeNavigationEvaluation::NotViable
        } else if incomplete {
            FreeNavigationEvaluation::Incomplete
        } else {
            FreeNavigationEvaluation::Viable
        };
        Ok(FreeNavigationProposal {
            destination,
            origin_position,
            destination_position,
            distance,
            duration,
            fuel_required: destination_data.fuel_cost,
            fuel_available,
            evaluation,
            risks,
            evaluated_at: now,
        })
    }

    pub(crate) fn register_route(
        &mut self,
        route: NavigationRoute,
    ) -> Result<(), NavigationRouteError> {
        route.validate()?;
        if self.destination(route.from).is_none() || self.destination(route.to).is_none() {
            return Err(NavigationRouteError::DestinationNotFound);
        }
        if self
            .destination(route.from)
            .is_none_or(|destination| destination.discovery != DiscoveryState::Discovered)
            || self
                .destination(route.to)
                .is_none_or(|destination| destination.discovery != DiscoveryState::Discovered)
        {
            return Err(NavigationRouteError::DestinationNotDiscovered);
        }
        if self.routes.contains_key(&(route.from, route.to)) {
            return Err(NavigationRouteError::DuplicateRoute);
        }
        self.routes.insert((route.from, route.to), route);
        Ok(())
    }

    pub(crate) fn plan_autopilot(
        &mut self,
        origin: DestinationId,
        destination: DestinationId,
    ) -> Result<Duration, AutopilotError> {
        if origin == destination {
            return Err(AutopilotError::SameDestination);
        }
        if !self.destinations.contains_key(&origin) || !self.destinations.contains_key(&destination)
        {
            return Err(AutopilotError::DestinationNotFound);
        }
        if self.has_active_travel() {
            return Err(AutopilotError::TravelActive);
        }
        let Some(origin_position) = self.destination(origin).map(|value| value.position) else {
            return Err(AutopilotError::DestinationNotFound);
        };
        if self.current_position != origin_position {
            return Err(AutopilotError::OriginMismatch);
        }
        if !matches!(
            self.autopilot,
            AutopilotState::Inactive
                | AutopilotState::Completed { .. }
                | AutopilotState::Interrupted { .. }
                | AutopilotState::Failed { .. }
        ) {
            return Err(AutopilotError::AlreadyPlanned);
        }
        let Some((route, distance, duration)) = self.find_route(origin, destination) else {
            return Err(AutopilotError::NoRoute);
        };
        let eta = Duration(duration);
        self.autopilot = AutopilotState::Planned {
            origin,
            destination,
            route,
            eta,
        };
        let _ = distance;
        Ok(eta)
    }

    pub(crate) fn start_autopilot(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
        player: EntityId,
        pilot_station: EntityId,
        discount_percent: u32,
    ) -> Result<(DestinationId, Duration, Duration, u32, u32), AutopilotError> {
        let AutopilotState::Planned {
            origin,
            destination,
            route,
            ..
        } = self.autopilot.clone()
        else {
            return Err(AutopilotError::NotPlanned);
        };
        if self.has_active_travel() {
            return Err(AutopilotError::TravelActive);
        }
        if route.len() < 2 || route[0] != origin {
            return Err(AutopilotError::OriginMismatch);
        }
        let total_fuel = route
            .windows(2)
            .try_fold(0_u32, |total, pair| {
                total.checked_add(effective_fuel_cost(
                    self.route(pair[0], pair[1])?.fuel_cost,
                    discount_percent,
                )?)
            })
            .ok_or(AutopilotError::InsufficientFuel)?;
        if !fuel.can_consume(total_fuel) {
            return Err(AutopilotError::InsufficientFuel);
        }
        let leg = self
            .route(route[0], route[1])
            .copied()
            .ok_or(AutopilotError::NoRoute)?;
        let leg_fuel = effective_fuel_cost(leg.fuel_cost, discount_percent)
            .ok_or(AutopilotError::InsufficientFuel)?;
        if leg.state != NavigationRouteState::Available || !fuel.can_consume(leg_fuel) {
            return Err(AutopilotError::InsufficientFuel);
        }
        fuel.consume(leg_fuel)
            .map_err(|_| AutopilotError::InsufficientFuel)?;
        self.autopilot_continuation_authorized = false;
        self.travel_origin = self.current_position;
        self.travel_state = TravelState::InProgress {
            destination: leg.to,
            started_at: now,
            duration: leg.duration,
            fuel_cost: leg_fuel,
        };
        let eta = self.route_duration(&route, 0).unwrap_or(leg.duration);
        self.autopilot = AutopilotState::InTransit {
            origin,
            destination,
            route,
            next_leg: 1,
            eta,
            player,
            pilot_station,
        };
        Ok((leg.to, now, leg.duration, leg_fuel, fuel.current()))
    }

    pub(crate) fn advance_autopilot_leg(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
        discount_percent: u32,
    ) -> Result<Option<(EntityId, EntityId, DestinationId, Duration, u32, u32)>, AutopilotError>
    {
        let AutopilotState::InTransit {
            origin,
            destination,
            route,
            next_leg,
            player,
            pilot_station,
            ..
        } = self.autopilot.clone()
        else {
            return Ok(None);
        };
        if next_leg + 1 >= route.len() {
            self.autopilot_continuation_authorized = false;
            self.autopilot = AutopilotState::Completed { destination };
            return Ok(None);
        }
        let leg = self
            .route(route[next_leg], route[next_leg + 1])
            .copied()
            .ok_or(AutopilotError::NoRoute)?;
        let leg_fuel = effective_fuel_cost(leg.fuel_cost, discount_percent)
            .ok_or(AutopilotError::InsufficientFuel)?;
        if leg.state != NavigationRouteState::Available || !fuel.can_consume(leg_fuel) {
            self.autopilot = AutopilotState::Failed { destination };
            return Err(AutopilotError::InsufficientFuel);
        }
        fuel.consume(leg_fuel)
            .map_err(|_| AutopilotError::InsufficientFuel)?;
        self.autopilot_continuation_authorized = false;
        self.travel_origin = self.current_position;
        self.travel_state = TravelState::InProgress {
            destination: leg.to,
            started_at: now,
            duration: leg.duration,
            fuel_cost: leg_fuel,
        };
        let remaining = self
            .route_duration(&route, next_leg)
            .unwrap_or(leg.duration);
        self.autopilot = AutopilotState::InTransit {
            origin,
            destination,
            route,
            next_leg: next_leg + 1,
            eta: remaining,
            player,
            pilot_station,
        };
        Ok(Some((
            player,
            pilot_station,
            leg.to,
            leg.duration,
            leg_fuel,
            fuel.current(),
        )))
    }

    pub(crate) fn cancel_autopilot(&mut self) -> Option<DestinationId> {
        let free_navigation = matches!(
            self.autopilot,
            AutopilotState::InTransit { ref route, next_leg: 1, .. }
                if route.len() == 2 && route[0] == route[1]
        );
        let destination = match self.autopilot {
            AutopilotState::Planned { destination, .. }
            | AutopilotState::InTransit { destination, .. } => destination,
            _ => return None,
        };
        if matches!(self.autopilot, AutopilotState::InTransit { .. }) {
            self.travel_state = TravelState::Cancelled { destination };
        }
        if free_navigation {
            self.planet_flight_state = PlanetFlightState::Free;
        }
        self.autopilot_continuation_authorized = false;
        self.autopilot = AutopilotState::Interrupted { destination };
        Some(destination)
    }

    fn route_duration(&self, route: &[DestinationId], start: usize) -> Option<Duration> {
        let total = route
            .windows(2)
            .skip(start)
            .try_fold(0_u64, |total, pair| {
                total.checked_add(self.route(pair[0], pair[1])?.duration.0)
            })?;
        Some(Duration(total))
    }

    fn find_route(
        &self,
        origin: DestinationId,
        destination: DestinationId,
    ) -> Option<(Vec<DestinationId>, f64, u64)> {
        let mut best: BTreeMap<DestinationId, (f64, u64, Vec<DestinationId>)> = BTreeMap::new();
        let mut pending = vec![(0.0_f64, 0_u64, vec![origin])];
        best.insert(origin, (0.0, 0, vec![origin]));
        while !pending.is_empty() {
            pending.sort_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            });
            let (distance, duration, path) = pending.remove(0);
            let current = *path.last()?;
            if current == destination {
                return Some((path, distance, duration));
            }
            for route in self.available_routes_from(current) {
                if path.contains(&route.to) {
                    continue;
                }
                let next_distance = distance + f64::from(route.distance);
                let next_duration = duration.checked_add(route.duration.0)?;
                let mut next_path = path.clone();
                next_path.push(route.to);
                let replace = best.get(&route.to).is_none_or(|existing| {
                    (next_distance, next_duration, &next_path)
                        < (existing.0, existing.1, &existing.2)
                });
                if replace {
                    best.insert(route.to, (next_distance, next_duration, next_path.clone()));
                    pending.push((next_distance, next_duration, next_path));
                }
            }
        }
        None
    }

    pub(crate) fn start_travel(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
        discount_percent: u32,
    ) -> Result<(DestinationId, Duration, Duration, u32, u32), NavigationStartError> {
        if self.has_active_travel() {
            return Err(NavigationStartError::TravelActive);
        }
        if matches!(
            self.autopilot,
            AutopilotState::Planned { .. } | AutopilotState::InTransit { .. }
        ) {
            return Err(NavigationStartError::AutopilotActive);
        }
        let Some(destination) = self.selected_destination else {
            return Err(NavigationStartError::DestinationNotSelected);
        };
        let Some(destination_data) = self.destinations.get(&destination) else {
            return Err(NavigationStartError::DestinationNotFound);
        };
        if destination_data.discovery == DiscoveryState::Unknown {
            return Err(NavigationStartError::DestinationUnknown);
        }
        if destination_data.travel_duration.0 == 0 {
            return Err(NavigationStartError::InvalidDuration);
        }
        if now
            .0
            .checked_add(destination_data.travel_duration.0)
            .is_none()
        {
            return Err(NavigationStartError::InvalidDuration);
        }
        let fuel_cost = effective_fuel_cost(destination_data.fuel_cost, discount_percent)
            .ok_or(NavigationStartError::InsufficientFuel)?;
        if !fuel.can_consume(fuel_cost) {
            return Err(NavigationStartError::InsufficientFuel);
        }
        fuel.consume(fuel_cost)
            .map_err(|_| NavigationStartError::InsufficientFuel)?;
        self.travel_origin = self.current_position;
        self.travel_state = TravelState::InProgress {
            destination,
            started_at: now,
            duration: destination_data.travel_duration,
            fuel_cost,
        };
        Ok((
            destination,
            now,
            destination_data.travel_duration,
            fuel_cost,
            fuel.current(),
        ))
    }

    pub(crate) fn start_free_navigation(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
        player: EntityId,
        pilot_station: EntityId,
        discount_percent: u32,
    ) -> Result<(DestinationId, Duration, Duration, u32, u32), FreeNavigationStartError> {
        if self.has_active_travel() {
            return Err(FreeNavigationStartError::TravelActive);
        }
        if matches!(
            self.autopilot,
            AutopilotState::Planned { .. } | AutopilotState::InTransit { .. }
        ) {
            return Err(FreeNavigationStartError::AutopilotActive);
        }
        let Some(destination) = self.free_navigation_destination else {
            return Err(FreeNavigationStartError::DestinationNotSelected);
        };
        let Some(proposal) = self.free_navigation_proposal.as_ref() else {
            return Err(FreeNavigationStartError::ProposalMissing);
        };
        if proposal.destination != destination {
            return Err(FreeNavigationStartError::ProposalDestinationMismatch);
        }
        if proposal.evaluation != FreeNavigationEvaluation::Viable {
            return Err(FreeNavigationStartError::ProposalNotViable);
        }
        let Some(destination_data) = self.destinations.get(&destination) else {
            return Err(FreeNavigationStartError::DestinationNotFound);
        };
        if destination_data.destination_type != DestinationType::Planet
            || !is_free_navigation_destination(destination)
        {
            return Err(FreeNavigationStartError::DestinationNotAllowed);
        }
        if destination_data.discovery == DiscoveryState::Unknown {
            return Err(FreeNavigationStartError::DestinationUnknown);
        }
        let destination_position = self
            .planet_position_at(destination, now)
            .unwrap_or(destination_data.position);
        let destination_moves = self
            .planet_physics
            .get(&destination)
            .is_some_and(|physics| physics.orbit_angular_speed != 0.0);
        if proposal.is_stale(
            now,
            self.current_position,
            destination_position,
            fuel.current(),
            destination_moves,
        ) {
            return Err(FreeNavigationStartError::ProposalStale);
        }
        let fuel_cost = effective_fuel_cost(destination_data.fuel_cost, discount_percent)
            .ok_or(FreeNavigationStartError::InvalidConfiguration)?;
        if !fuel.can_consume(fuel_cost) {
            return Err(FreeNavigationStartError::InsufficientFuel);
        }
        if now
            .0
            .checked_add(destination_data.travel_duration.0)
            .is_none()
        {
            return Err(FreeNavigationStartError::InvalidConfiguration);
        }
        fuel.consume(fuel_cost)
            .map_err(|_| FreeNavigationStartError::InsufficientFuel)?;
        self.travel_origin = self.current_position;
        self.planet_flight_state = PlanetFlightState::Landing {
            planet: destination,
        };
        self.travel_state = TravelState::InProgress {
            destination,
            started_at: now,
            duration: destination_data.travel_duration,
            fuel_cost,
        };
        self.free_navigation_proposal = None;
        self.autopilot_continuation_authorized = false;
        self.autopilot = AutopilotState::InTransit {
            origin: destination,
            destination,
            route: vec![destination, destination],
            next_leg: 1,
            eta: destination_data.travel_duration,
            player,
            pilot_station,
        };
        Ok((
            destination,
            now,
            destination_data.travel_duration,
            fuel_cost,
            fuel.current(),
        ))
    }

    pub(crate) fn complete_travel(&mut self, destination: DestinationId) -> bool {
        if !matches!(
            self.travel_state,
            TravelState::InProgress {
                destination: active,
                ..
            } if active == destination
        ) {
            return false;
        }
        if let Some(destination_data) = self.destinations.get(&destination) {
            self.current_position = destination_data.position;
        }
        self.travel_state = TravelState::Completed { destination };
        true
    }

    pub(crate) fn complete_travel_at(
        &mut self,
        destination: DestinationId,
        now: Duration,
    ) -> Result<Vec3, (DestinationId, Vec3)> {
        if !matches!(
            self.travel_state,
            TravelState::InProgress {
                destination: active,
                ..
            } if active == destination
        ) {
            return Err((destination, self.current_position));
        }
        let Some(physics) = self.planet_physics.get(&destination).copied() else {
            let _ = self.complete_travel(destination);
            return Ok(self.current_position);
        };
        let planet = physics.position_at(now);
        let direction = sub_vec(planet, self.current_position);
        let length = distance(self.current_position, planet);
        if length <= physics.collision_radius() {
            self.planet_flight_state = PlanetFlightState::Impacted {
                planet: destination,
            };
            self.travel_state = TravelState::Cancelled { destination };
            return Err((destination, self.current_position));
        }
        if matches!(self.planet_flight_state, PlanetFlightState::Landing { planet: active } if active == destination)
        {
            let landing_position = sub_vec(
                planet,
                scale_vec(direction, physics.collision_radius() / length),
            );
            self.current_position = landing_position;
            self.velocity = Vec3::ZERO;
            self.planet_flight_state = PlanetFlightState::Landed {
                planet: destination,
            };
            self.travel_state = TravelState::Completed { destination };
            return Ok(landing_position);
        }
        let impact_position = add_vec(
            self.current_position,
            scale_vec(direction, (length - physics.collision_radius()) / length),
        );
        self.current_position = impact_position;
        self.velocity = Vec3::ZERO;
        self.planet_flight_state = PlanetFlightState::Impacted {
            planet: destination,
        };
        self.impact_started_at = Some(now);
        self.travel_state = TravelState::Cancelled { destination };
        Err((destination, impact_position))
    }

    pub(crate) fn cancel_travel(&mut self) -> Option<DestinationId> {
        let TravelState::InProgress { destination, .. } = self.travel_state else {
            return None;
        };
        self.travel_state = TravelState::Cancelled { destination };
        let _ = self.cancel_autopilot();
        Some(destination)
    }

    pub(crate) fn discover_destination(&mut self, destination: DestinationId) -> bool {
        let Some(destination_data) = self.destinations.get_mut(&destination) else {
            return false;
        };
        if destination_data.destination_type != DestinationType::Planet
            || destination_data.discovery == DiscoveryState::Discovered
        {
            return false;
        }
        destination_data.discovery = DiscoveryState::Discovered;
        true
    }
}

fn effective_fuel_cost(base: u32, discount_percent: u32) -> Option<u32> {
    if discount_percent > 100 {
        return None;
    }
    let remaining = u64::from(100 - discount_percent);
    let scaled = u64::from(base).checked_mul(remaining)?;
    u32::try_from(scaled.checked_add(99)? / 100).ok()
}

pub(crate) fn discounted_fuel_cost(base: u32, discount_percent: u32) -> Option<u32> {
    effective_fuel_cost(base, discount_percent)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotError {
    SameDestination,
    DestinationNotFound,
    TravelActive,
    AlreadyPlanned,
    NoRoute,
    NotPlanned,
    OriginMismatch,
    InsufficientFuel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationSelectionError {
    TravelActive,
    AutopilotActive,
    DestinationNotFound,
    DestinationUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreeNavigationSelectionError {
    TravelActive,
    AutopilotActive,
    DestinationNotFound,
    DestinationNotPlanet,
    DestinationNotAllowed,
    DestinationUnknown,
    DestinationIsCurrentPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreeNavigationEvaluationError {
    DestinationNotSelected,
    DestinationNotFound,
    DestinationNotAllowed,
    DestinationUnknown,
    InvalidDistance,
    InvalidTravelConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreeNavigationStartError {
    TravelActive,
    AutopilotActive,
    DestinationNotSelected,
    ProposalMissing,
    ProposalDestinationMismatch,
    ProposalStale,
    ProposalNotViable,
    DestinationNotFound,
    DestinationNotAllowed,
    DestinationUnknown,
    InsufficientFuel,
    InvalidConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationStartError {
    TravelActive,
    AutopilotActive,
    DestinationNotSelected,
    DestinationNotFound,
    DestinationUnknown,
    InvalidDuration,
    ShipNotFound,
    ResourcesNotInitialized,
    InsufficientFuel,
}

fn finite_position(position: Vec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}

fn distance(first: Vec3, second: Vec3) -> f32 {
    distance_squared(first, second).sqrt().min(f32::MAX as f64) as f32
}

fn distance_squared(first: Vec3, second: Vec3) -> f64 {
    let x = first.x as f64 - second.x as f64;
    let y = first.y as f64 - second.y as f64;
    let z = first.z as f64 - second.z as f64;
    x * x + y * y + z * z
}

fn segment_intersects_sphere(start: Vec3, end: Vec3, center: Vec3, radius: f32) -> bool {
    let direction = sub_vec(end, start);
    let offset = sub_vec(start, center);
    let length_squared = dot(direction, direction);
    if length_squared <= 0.0 {
        return dot(offset, offset) <= radius * radius;
    }
    let projection = (-dot(offset, direction) / length_squared).clamp(0.0, 1.0);
    let closest = add_vec(start, scale_vec(direction, projection));
    let distance = sub_vec(closest, center);
    dot(distance, distance) <= radius * radius
}

fn apply_collision_assist(
    velocity: &mut Vec3,
    position: Vec3,
    target: Option<DestinationId>,
    planets: &BTreeMap<DestinationId, PlanetPhysics>,
    solar: Option<PlanetPhysics>,
    deceleration: f32,
    seconds: f32,
    now: Duration,
) {
    // Assistance is intentional only while the player has selected a
    // destination. Free flight without a target keeps the original physics
    // and collision feedback intact.
    let Some(target) = target else {
        return;
    };
    if !finite_vec3(position) || !finite_vec3(*velocity) {
        return;
    }

    let mut assist_body = |body_id: DestinationId, body: PlanetPhysics| {
        let body_position = body.position_at(now);
        let offset = sub_vec(position, body_position);
        let distance = magnitude(offset);
        if !distance.is_finite() || distance <= f32::EPSILON {
            return;
        }
        let speed = magnitude(*velocity);
        let stopping_distance = speed * speed / (2.0 * deceleration.max(0.1));
        let braking_radius =
            body.collision_radius() + COLLISION_ASSIST_BUFFER + stopping_distance.min(12.0);
        let projected = add_vec(position, scale_vec(*velocity, seconds));
        let target_body = target == body_id;

        if target_body && distance <= braking_radius {
            *velocity = move_towards_zero(*velocity, deceleration * seconds * 1.35);
            return;
        }
        if !target_body
            && segment_intersects_sphere(
                position,
                projected,
                body_position,
                body.collision_radius() + COLLISION_ASSIST_BUFFER,
            )
        {
            // Remove the component aimed at the obstacle. The remaining
            // tangential component lets the player naturally pass around it.
            let outward = scale_vec(offset, 1.0 / distance);
            let inward_speed = dot(*velocity, outward);
            if inward_speed < 0.0 {
                *velocity = sub_vec(*velocity, scale_vec(outward, inward_speed));
            }
            *velocity = scale_vec(*velocity, 0.65);
        }
    };

    for (id, body) in planets {
        assist_body(*id, *body);
    }
    if let Some(body) = solar {
        assist_body(SUN_DESTINATION_ID, body);
    }
}

fn impact_position(start: Vec3, center: Vec3, radius: f32) -> Vec3 {
    let direction = sub_vec(start, center);
    let length = distance(start, center).max(radius);
    add_vec(center, scale_vec(direction, radius / length))
}

fn dot(left: Vec3, right: Vec3) -> f32 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

fn add_vec(left: Vec3, right: Vec3) -> Vec3 {
    Vec3 {
        x: left.x + right.x,
        y: left.y + right.y,
        z: left.z + right.z,
    }
}

fn sub_vec(left: Vec3, right: Vec3) -> Vec3 {
    Vec3 {
        x: left.x - right.x,
        y: left.y - right.y,
        z: left.z - right.z,
    }
}

fn scale_vec(value: Vec3, scale: f32) -> Vec3 {
    Vec3 {
        x: value.x * scale,
        y: value.y * scale,
        z: value.z * scale,
    }
}

fn length_squared(value: Vec3) -> f32 {
    dot(value, value)
}

fn cross_vec(left: Vec3, right: Vec3) -> Vec3 {
    Vec3 {
        x: left.y * right.z - left.z * right.y,
        y: left.z * right.x - left.x * right.z,
        z: left.x * right.y - left.y * right.x,
    }
}

fn normalize_vec(value: Vec3) -> Vec3 {
    let length = magnitude(value);
    if !length.is_finite() || length <= f32::EPSILON {
        return Vec3::ZERO;
    }
    scale_vec(value, 1.0 / length)
}

fn finite_vec3(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn magnitude(value: Vec3) -> f32 {
    (value.x * value.x + value.y * value.y + value.z * value.z).sqrt()
}

fn clamp_speed(value: Vec3, maximum: f32) -> Vec3 {
    let current = magnitude(value);
    if !current.is_finite() || current <= maximum || current == 0.0 {
        return value;
    }
    Vec3 {
        x: value.x / current * maximum,
        y: value.y / current * maximum,
        z: value.z / current * maximum,
    }
}

fn add_velocity(current: Vec3, input: Vec3, delta: f32) -> Vec3 {
    Vec3 {
        x: current.x + input.x * delta,
        y: current.y + input.y * delta,
        z: current.z + input.z * delta,
    }
}

fn move_towards_zero(value: Vec3, delta: f32) -> Vec3 {
    let current = magnitude(value);
    if current <= delta || current == 0.0 {
        return Vec3::ZERO;
    }
    Vec3 {
        x: value.x / current * (current - delta),
        y: value.y / current * (current - delta),
        z: value.z / current * (current - delta),
    }
}

fn normalize_yaw(mut value: f32) -> f32 {
    while value >= 180.0 {
        value -= 360.0;
    }
    while value < -180.0 {
        value += 360.0;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn physics(destination: DestinationId) -> PlanetPhysics {
        PlanetPhysics {
            destination,
            orbit_center: Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            orbit_radius: 1.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 2.0,
            ship_radius: 0.0,
            gravity_influence_radius: 20.0,
            gravitational_parameter: 4.0,
            atmosphere_height: 2.0,
        }
    }

    fn solar_physics() -> PlanetPhysics {
        PlanetPhysics {
            destination: SUN_DESTINATION_ID,
            orbit_center: Vec3::ZERO,
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 2.0,
            ship_radius: 0.0,
            gravity_influence_radius: 20.0,
            gravitational_parameter: 4.0,
            atmosphere_height: 0.0,
        }
    }

    fn navigation(position: Vec3) -> NavigationState {
        let mut navigation = NavigationState::default();
        let destination = DestinationId::new(7);
        navigation
            .register_destination(Destination {
                id: destination,
                destination_type: DestinationType::Planet,
                position: Vec3::ZERO,
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .expect("destination should be valid");
        assert!(navigation.configure_planet_physics(physics(destination)));
        navigation
            .restore_position(position)
            .expect("position is finite");
        navigation
            .initialize_ship(EntityId::new(1))
            .expect("ship should initialize");
        navigation.set_control_active(true);
        navigation
    }

    #[test]
    fn orbital_position_is_deterministic_and_time_dependent() {
        let physics = physics(DestinationId::new(7));
        let start = physics.position_at(Duration(0));
        let later = physics.position_at(Duration(1_000));
        assert_eq!(start, later);

        let mut moving = physics;
        moving.orbit_angular_speed = 1.0;
        assert_ne!(
            moving.position_at(Duration(0)),
            moving.position_at(Duration(1_000))
        );
    }

    #[test]
    fn collision_assist_brakes_for_selected_planet_before_arrival() {
        let destination = DestinationId::new(7);
        let mut bodies = BTreeMap::new();
        bodies.insert(
            destination,
            PlanetPhysics {
                destination,
                orbit_center: Vec3::ZERO,
                orbit_radius: 0.0,
                orbit_start: Vec3::ZERO,
                orbit_axis: Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                orbit_phase: 0.0,
                orbit_angular_speed: 0.0,
                surface_radius: 2.0,
                ship_radius: 1.0,
                gravity_influence_radius: 20.0,
                gravitational_parameter: 1.0,
                atmosphere_height: 2.0,
            },
        );
        let mut velocity = Vec3 {
            x: 8.0,
            y: 0.0,
            z: 0.0,
        };
        apply_collision_assist(
            &mut velocity,
            Vec3 {
                x: -5.0,
                y: 0.0,
                z: 0.0,
            },
            Some(destination),
            &bodies,
            None,
            6.5,
            0.1,
            Duration(0),
        );
        assert!(velocity.x < 8.0);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn collision_assist_removes_course_towards_non_target_body() {
        let destination = DestinationId::new(7);
        let obstacle = DestinationId::new(8);
        let mut bodies = BTreeMap::new();
        bodies.insert(
            obstacle,
            PlanetPhysics {
                destination: obstacle,
                orbit_center: Vec3::ZERO,
                orbit_radius: 0.0,
                orbit_start: Vec3::ZERO,
                orbit_axis: Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                orbit_phase: 0.0,
                orbit_angular_speed: 0.0,
                surface_radius: 2.0,
                ship_radius: 1.0,
                gravity_influence_radius: 20.0,
                gravitational_parameter: 1.0,
                atmosphere_height: 2.0,
            },
        );
        let mut velocity = Vec3 {
            x: 20.0,
            y: 0.0,
            z: 0.0,
        };
        apply_collision_assist(
            &mut velocity,
            Vec3 {
                x: -5.0,
                y: 0.0,
                z: 0.0,
            },
            Some(destination),
            &bodies,
            None,
            6.5,
            0.5,
            Duration(0),
        );
        assert!(velocity.x.abs() < 0.001);
    }

    #[test]
    fn orbital_phase_preserves_initial_angle_and_orbit_radius() {
        let physics = PlanetPhysics {
            destination: DestinationId::new(9_602),
            orbit_center: Vec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: 100.0004,
            orbit_start: Vec3 {
                x: -70.2,
                y: -12.0,
                z: -88.2,
            },
            orbit_axis: Vec3 {
                x: 70.2,
                y: 0.0,
                z: -70.2,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.08,
            surface_radius: 7.0,
            ship_radius: 1.0,
            gravity_influence_radius: 28.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        };
        let start = physics.position_at(Duration(0));
        let later = physics.position_at(Duration(1_000));
        let radius = |position: Vec3| {
            let dx = position.x - physics.orbit_center.x;
            let dy = position.y - physics.orbit_center.y;
            let dz = position.z - physics.orbit_center.z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        assert!((start.x + 70.2).abs() < 1e-3);
        assert!((start.z + 88.2).abs() < 1e-3);
        assert!((radius(start) - physics.orbit_radius).abs() < 1e-2);
        assert!((radius(later) - physics.orbit_radius).abs() < 1e-2);
        assert_ne!(start, later);
        assert_eq!(physics.position_at(Duration(1_000)), later);
    }

    #[test]
    fn tessara_orbit_preserves_initial_position_and_radius() {
        let physics = PlanetPhysics {
            destination: DestinationId::new(9_601),
            orbit_center: Vec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: 69.99556,
            orbit_start: Vec3 {
                x: -49.17,
                y: 8.0,
                z: 31.17,
            },
            orbit_axis: Vec3 {
                x: 49.17,
                y: 0.0,
                z: 49.17,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.12,
            surface_radius: 5.0,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        };
        let start = physics.position_at(Duration(0));
        let later = physics.position_at(Duration(10_000));
        let radius = |position: Vec3| distance(position, physics.orbit_center);

        assert!(distance(start, physics.orbit_start) < 1e-2);
        assert!((radius(start) - physics.orbit_radius).abs() < 1e-2);
        assert!((radius(later) - physics.orbit_radius).abs() < 1e-2);
        assert_ne!(start, later);
        assert_eq!(physics.position_at(Duration(10_000)), later);
    }

    #[test]
    fn static_planet_physics_can_anchor_a_destination_without_orbit_animation() {
        let mut navigation = NavigationState::default();
        let destination = DestinationId::new(9_601);
        let position = Vec3 {
            x: -49.17,
            y: 8.0,
            z: 31.17,
        };
        navigation
            .register_destination(Destination {
                id: destination,
                destination_type: DestinationType::Planet,
                position,
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .expect("static planet destination should be valid");
        assert!(navigation.configure_planet_physics(PlanetPhysics {
            destination,
            orbit_center: position,
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 5.0,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        }));
        assert_eq!(
            navigation.planet_position_at(destination, Duration(0)),
            Some(position)
        );
        assert_eq!(
            navigation.planet_position_at(destination, Duration(10_000)),
            Some(position)
        );
    }

    #[test]
    fn gravity_changes_velocity_towards_the_planet() {
        let mut navigation = navigation(Vec3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        });
        navigation
            .set_acceleration_input(Vec3::ZERO)
            .expect("zero input is valid");
        let (_, velocity, _, _) = navigation
            .advance_manual(Duration(100), Duration(0))
            .expect("active control advances");
        assert!(velocity.x < 0.0);
        assert!(velocity.x.is_finite());
    }

    #[test]
    fn solar_gravity_is_fixed_directional_and_outside_influence_is_inert() {
        let mut outside = NavigationState::default();
        assert!(outside.configure_solar_physics(solar_physics()));
        outside
            .restore_position(Vec3 {
                x: 30.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("position is finite");
        outside
            .initialize_ship(EntityId::new(1))
            .expect("ship should initialize");
        outside.set_control_active(true);
        outside
            .set_acceleration_input(Vec3::ZERO)
            .expect("zero input is valid");
        outside
            .advance_manual(Duration(100), Duration(0))
            .expect("outside flight advances");
        assert_eq!(outside.velocity(), Vec3::ZERO);

        let mut inside = NavigationState::default();
        assert!(inside.configure_solar_physics(solar_physics()));
        inside
            .restore_position(Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("position is finite");
        inside
            .initialize_ship(EntityId::new(1))
            .expect("ship should initialize");
        inside.set_control_active(true);
        inside
            .set_acceleration_input(Vec3::ZERO)
            .expect("zero input is valid");
        let (_, velocity, _, _) = inside
            .advance_manual(Duration(100), Duration(0))
            .expect("inside flight advances");
        assert!(velocity.x < 0.0);
    }

    #[test]
    fn solar_impact_is_swept_terminal_and_not_landable() {
        let mut navigation = NavigationState::default();
        assert!(navigation.configure_solar_physics(solar_physics()));
        navigation
            .restore_position(Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("position is finite");
        navigation
            .initialize_ship(EntityId::new(1))
            .expect("ship should initialize");
        navigation.set_control_active(true);
        navigation
            .set_acceleration_input(Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("input is valid");

        let contact = navigation
            .advance_manual(Duration(10_000), Duration(0))
            .expect("flight advances to the sun");
        assert_eq!(contact.3, Some((SUN_DESTINATION_ID, true)));
        assert_eq!(
            navigation.planet_flight_state(),
            PlanetFlightState::Impacted {
                planet: SUN_DESTINATION_ID
            }
        );
        assert!(!navigation.landing_available(SUN_DESTINATION_ID, Duration(10_000)));
        assert!(navigation.current_position().x.abs() >= solar_physics().collision_radius());
        assert!(navigation
            .advance_manual(Duration(100), Duration(100))
            .is_none());
    }

    #[test]
    fn boost_raises_the_speed_limit_only_while_active() {
        let mut navigation = NavigationState::default();
        navigation
            .initialize_ship(EntityId::new(1))
            .expect("ship should initialize");
        navigation.set_control_active(true);
        navigation
            .set_acceleration_input(Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("input is valid");
        for _ in 0..20 {
            navigation.advance_manual(Duration(1_000), Duration(0));
        }
        assert!(navigation.velocity().x <= navigation.movement_parameters().max_speed);
        assert!(navigation.set_boost_active(true));
        for _ in 0..20 {
            navigation.advance_manual(Duration(1_000), Duration(0));
        }
        assert!(navigation.velocity().x > navigation.movement_parameters().max_speed);
        assert!(navigation.velocity().x <= navigation.speed_limit());
        navigation.set_boost_active(false);
        assert!(navigation.velocity().x <= navigation.movement_parameters().max_speed);
    }

    #[test]
    fn manual_braking_reverses_velocity_without_erasing_inertia() {
        let mut navigation = navigation(Vec3 {
            x: 100.0,
            y: 0.0,
            z: 0.0,
        });
        navigation
            .set_acceleration_input(Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            })
            .expect("forward input is valid");
        navigation
            .advance_manual(Duration(1_000), Duration(0))
            .expect("forward flight advances");
        let forward_velocity = navigation.velocity();
        assert!(forward_velocity.z < 0.0);

        navigation
            .set_acceleration_input(Vec3::ZERO)
            .expect("releasing input is valid");
        let position_before_coast = navigation.current_position();
        navigation
            .advance_manual(Duration(500), Duration(500))
            .expect("coasting advances");
        assert_eq!(navigation.velocity(), forward_velocity);
        assert!(navigation.current_position().z < position_before_coast.z);

        navigation
            .set_acceleration_input(Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            })
            .expect("backward input is valid");
        for step in 0..12 {
            navigation
                .advance_manual(Duration(100), Duration(1_000 + step * 100))
                .expect("reverse flight advances");
        }
        assert!(navigation.velocity().z > 0.0);
        assert!(navigation.current_position().z.is_finite());
    }

    #[test]
    fn gravity_is_planetary_and_stops_outside_its_influence_radius() {
        let mut near = navigation(Vec3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        });
        let mut far = navigation(Vec3 {
            x: 100.0,
            y: 0.0,
            z: 0.0,
        });
        near.set_acceleration_input(Vec3::ZERO)
            .expect("zero input is valid");
        far.set_acceleration_input(Vec3::ZERO)
            .expect("zero input is valid");

        near.advance_manual(Duration(100), Duration(0))
            .expect("near trajectory advances");
        far.advance_manual(Duration(100), Duration(0))
            .expect("far trajectory advances");

        assert!(near.velocity().x < 0.0);
        assert_eq!(far.velocity(), Vec3::ZERO);
        assert!(near.current_position().x < 10.0);
        assert_eq!(
            far.current_position(),
            Vec3 {
                x: 100.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert!(near.velocity().x.is_finite());
        assert!(near.current_position().x.is_finite());
    }

    #[test]
    fn swept_collision_keeps_the_ship_outside_the_planet_and_emits_once() {
        let mut navigation = navigation(Vec3 {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        });
        navigation
            .set_acceleration_input(Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("input is valid");
        let (_, _, moved, contact) = navigation
            .advance_manual(Duration(1_000), Duration(0))
            .expect("active control advances");
        assert!(!moved);
        assert_eq!(contact, Some((DestinationId::new(7), true)));
        assert_eq!(
            navigation.planet_flight_state(),
            PlanetFlightState::Impacted {
                planet: DestinationId::new(7)
            }
        );
        assert!(distance(navigation.current_position(), Vec3::ZERO) >= 2.0);
        assert!(navigation
            .advance_manual(Duration(100), Duration(0))
            .is_none());
    }

    #[test]
    fn impact_flash_is_bounded_and_does_not_retrigger_terminal_motion() {
        let mut navigation = navigation(Vec3 {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        });
        navigation
            .set_acceleration_input(Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("input is valid");
        navigation
            .advance_manual(Duration(1_000), Duration(100))
            .expect("active control advances");

        assert_eq!(navigation.impact_flash_intensity(Duration(100)), Some(1.0));
        assert!(navigation.impact_flash_intensity(Duration(249)).unwrap() > 0.0);
        assert_eq!(navigation.impact_flash_intensity(Duration(350)), None);
        assert_eq!(navigation.impact_fade_to_black(Duration(100)), Some(0.0));
        assert_eq!(navigation.impact_fade_to_black(Duration(1_100)), Some(1.0));
        assert!(navigation
            .advance_manual(Duration(100), Duration(400))
            .is_none());
    }

    #[test]
    fn landing_is_confirmed_before_the_next_simulation_step() {
        let mut navigation = navigation(Vec3 {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        });
        let planet = DestinationId::new(7);
        assert!(navigation.landing_available(planet, Duration(0)));
        assert!(navigation.request_landing(planet, Duration(0)));
        let (_, _, _, contact) = navigation
            .advance_manual(Duration(100), Duration(0))
            .expect("landing advances without flight controls");
        assert_eq!(contact, Some((planet, false)));
        assert_eq!(
            navigation.planet_flight_state(),
            PlanetFlightState::Landed { planet }
        );
    }

    #[test]
    fn free_navigation_selection_is_independent_and_planet_only() {
        let mut navigation = NavigationState::default();
        navigation
            .register_destination(Destination {
                id: VEYRA_DESTINATION_ID,
                destination_type: DestinationType::Planet,
                position: Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .expect("Veyra should register");
        navigation
            .register_destination(Destination {
                id: TESSARA_DESTINATION_ID,
                destination_type: DestinationType::Planet,
                position: Vec3 {
                    x: 20.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .expect("Tessara should register");
        navigation
            .register_destination(Destination {
                id: DestinationId::new(9_003),
                destination_type: DestinationType::SpacePoint,
                position: Vec3 {
                    x: 30.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .expect("signal should register");
        navigation
            .restore_position(Vec3::ZERO)
            .expect("ship position should be finite");

        navigation
            .confirm_free_navigation_destination(VEYRA_DESTINATION_ID, Duration(0))
            .expect("Veyra should be accepted");
        assert_eq!(
            navigation.free_navigation_destination(),
            Some(VEYRA_DESTINATION_ID)
        );
        assert_eq!(navigation.selected_destination(), None);
        assert_eq!(navigation.travel_state(), TravelState::Idle);
        assert_eq!(navigation.autopilot_state(), &AutopilotState::Inactive);

        navigation
            .confirm_free_navigation_destination(TESSARA_DESTINATION_ID, Duration(0))
            .expect("Tessara should replace the free destination");
        assert_eq!(
            navigation.free_navigation_destination(),
            Some(TESSARA_DESTINATION_ID)
        );

        assert_eq!(
            navigation.confirm_free_navigation_destination(DestinationId::new(9_003), Duration(0)),
            Err(FreeNavigationSelectionError::DestinationNotPlanet)
        );
        assert_eq!(
            navigation.free_navigation_destination(),
            Some(TESSARA_DESTINATION_ID)
        );
    }

    #[test]
    fn free_navigation_evaluation_uses_current_position_without_mutating_travel() {
        let mut navigation = NavigationState::default();
        navigation
            .register_destination(Destination {
                id: VEYRA_DESTINATION_ID,
                destination_type: DestinationType::Planet,
                position: Vec3 {
                    x: 30.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(2_000),
                fuel_cost: 3,
                discovery: DiscoveryState::Discovered,
            })
            .expect("Veyra should register");
        navigation
            .restore_position(Vec3 {
                x: 10.0,
                y: 4.0,
                z: -2.0,
            })
            .expect("arbitrary ship position should be accepted");
        navigation
            .confirm_free_navigation_destination(VEYRA_DESTINATION_ID, Duration(0))
            .expect("Veyra should be confirmed");
        assert!(navigation.configure_planet_physics(PlanetPhysics {
            destination: VEYRA_DESTINATION_ID,
            orbit_center: Vec3 {
                x: 30.0,
                y: 0.0,
                z: 0.0
            },
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 5.0,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        }));

        let proposal = navigation
            .evaluate_free_navigation(Duration(0), 7)
            .expect("proposal should be evaluable");
        assert_eq!(
            proposal.origin_position,
            Vec3 {
                x: 10.0,
                y: 4.0,
                z: -2.0
            }
        );
        assert!((proposal.distance - (420.0_f32).sqrt()).abs() < 0.001);
        assert_eq!(proposal.duration, Duration(2_000));
        assert_eq!(proposal.fuel_required, 3);
        assert_eq!(proposal.fuel_available, 7);
        assert_eq!(proposal.evaluation, FreeNavigationEvaluation::Viable);
        assert_eq!(navigation.travel_state(), TravelState::Idle);
        assert_eq!(navigation.autopilot_state(), &AutopilotState::Inactive);
        assert_eq!(navigation.free_navigation_proposal(), None);
    }

    #[test]
    fn free_navigation_evaluation_reports_fuel_and_solar_risks_without_starting_travel() {
        let mut navigation = NavigationState::default();
        navigation
            .register_destination(Destination {
                id: VEYRA_DESTINATION_ID,
                destination_type: DestinationType::Planet,
                position: Vec3 {
                    x: 30.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(2_000),
                fuel_cost: 3,
                discovery: DiscoveryState::Discovered,
            })
            .expect("Veyra should register");
        assert!(navigation.configure_planet_physics(PlanetPhysics {
            destination: VEYRA_DESTINATION_ID,
            orbit_center: Vec3 {
                x: 30.0,
                y: 0.0,
                z: 0.0
            },
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 5.0,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        }));
        assert!(navigation.configure_solar_physics(PlanetPhysics {
            destination: SUN_DESTINATION_ID,
            orbit_center: Vec3 {
                x: 20.0,
                y: 0.0,
                z: 0.0
            },
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 2.0,
            ship_radius: 0.0,
            gravity_influence_radius: 20.0,
            gravitational_parameter: 4.0,
            atmosphere_height: 0.0,
        }));
        navigation
            .restore_position(Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("arbitrary ship position should be accepted");
        navigation
            .confirm_free_navigation_destination(VEYRA_DESTINATION_ID, Duration(0))
            .expect("Veyra should be confirmed");

        let proposal = navigation
            .evaluate_free_navigation(Duration(0), 2)
            .expect("proposal should be evaluable");
        assert_eq!(proposal.evaluation, FreeNavigationEvaluation::NotViable);
        assert!(proposal.risks.contains(&FreeNavigationRisk::SolarCollision));
        assert!(proposal
            .risks
            .contains(&FreeNavigationRisk::InsufficientFuel {
                required: 3,
                available: 2,
            }));
        assert_eq!(navigation.travel_state(), TravelState::Idle);
        assert_eq!(navigation.autopilot_state(), &AutopilotState::Inactive);
    }

    #[test]
    fn free_navigation_starts_once_from_current_position_and_completes_at_destination() {
        let mut navigation = NavigationState::default();
        let destination = VEYRA_DESTINATION_ID;
        let target = Vec3 {
            x: 30.0,
            y: 0.0,
            z: 0.0,
        };
        navigation
            .register_destination(Destination {
                id: destination,
                destination_type: DestinationType::Planet,
                position: target,
                travel_duration: Duration(1_000),
                fuel_cost: 3,
                discovery: DiscoveryState::Discovered,
            })
            .expect("destination should register");
        assert!(navigation.configure_planet_physics(PlanetPhysics {
            destination,
            orbit_center: target,
            orbit_radius: 0.0,
            orbit_start: Vec3::ZERO,
            orbit_axis: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: 2.0,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        }));
        navigation
            .restore_position(Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            })
            .expect("origin should be valid");
        navigation
            .confirm_free_navigation_destination(destination, Duration(0))
            .expect("destination should confirm");
        let proposal = navigation
            .evaluate_free_navigation(Duration(0), 7)
            .expect("proposal should evaluate");
        navigation.set_free_navigation_proposal(proposal);
        let mut fuel = FuelTank::new(10, 7).expect("fuel should initialize");
        let started = navigation
            .start_free_navigation(
                Duration(0),
                &mut fuel,
                EntityId::new(1),
                EntityId::new(2),
                0,
            )
            .expect("free navigation should start");
        assert_eq!(started.3, 3);
        assert_eq!(fuel.current(), 4);
        assert_eq!(
            navigation.current_position(),
            Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert!(matches!(
            navigation.travel_state(),
            TravelState::InProgress { .. }
        ));
        assert!(matches!(
            navigation.autopilot_state(),
            AutopilotState::InTransit { .. }
        ));
        assert!(navigation.free_navigation_proposal().is_none());
        assert_eq!(
            navigation.start_free_navigation(
                Duration(0),
                &mut fuel,
                EntityId::new(1),
                EntityId::new(2),
                0,
            ),
            Err(FreeNavigationStartError::TravelActive)
        );
        assert_eq!(fuel.current(), 4);

        assert_eq!(navigation.advance_travel_at(Duration(500)), None);
        assert!(navigation.current_position().x > 10.0);
        let arrival = navigation.advance_travel_at(Duration(1_000));
        assert_eq!(arrival.map(|value| value.0), Some(destination));
        assert_eq!(
            navigation.travel_state(),
            TravelState::Completed { destination }
        );
        assert_eq!(navigation.current_position().x, 27.0);
        assert_eq!(fuel.current(), 4);
    }
}

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
    Completed { destination: DestinationId },
    Interrupted { destination: DestinationId },
    Failed { destination: DestinationId },
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

impl Default for ShipMovementParameters {
    fn default() -> Self {
        Self {
            acceleration: 4.0,
            deceleration: 6.0,
            max_speed: 20.0,
            turn_rate_degrees: 45.0,
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
    travel_state: TravelState,
    destinations: BTreeMap<DestinationId, Destination>,
    orientation: ShipOrientation,
    velocity: Vec3,
    movement: ShipMovementParameters,
    control_active: bool,
    acceleration_input: Vec3,
    braking: bool,
    scanner: SignalScanner,
    signal_knowledge: BTreeMap<DestinationId, SignalKnowledge>,
    last_scan_results: Vec<SignalDetection>,
    routes: BTreeMap<(DestinationId, DestinationId), NavigationRoute>,
    autopilot: AutopilotState,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            ship_id: None,
            current_position: Vec3::ZERO,
            selected_destination: None,
            travel_state: TravelState::Idle,
            destinations: BTreeMap::new(),
            orientation: ShipOrientation::default(),
            velocity: Vec3::ZERO,
            movement: ShipMovementParameters::default(),
            control_active: false,
            acceleration_input: Vec3::ZERO,
            braking: false,
            scanner: SignalScanner::default(),
            signal_knowledge: BTreeMap::new(),
            last_scan_results: Vec::new(),
            routes: BTreeMap::new(),
            autopilot: AutopilotState::Inactive,
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
                || next_destinations.insert(destination.id, destination).is_some()
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
        self.travel_state = TravelState::Idle;
        self.autopilot = AutopilotState::Inactive;
        Ok(())
    }

    pub(crate) fn restore_position(&mut self, position: Vec3) -> Result<(), NavigationMovementError> {
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
        }
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

    pub(crate) fn advance_manual(&mut self, delta: Duration) -> Option<(Vec3, Vec3, bool)> {
        if !self.control_active || delta.0 == 0 || self.has_active_travel() {
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
                self.movement.acceleration * seconds,
            );
            self.velocity = clamp_speed(self.velocity, self.movement.max_speed);
        }
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
        if finite_vec3(next_position) {
            self.current_position = next_position;
        }
        Some((
            previous_velocity,
            self.velocity,
            self.current_position != previous_position,
        ))
    }

    pub fn destinations(&self) -> impl Iterator<Item = &Destination> {
        self.destinations.values()
    }

    pub fn routes(&self) -> impl Iterator<Item = &NavigationRoute> {
        self.routes.values()
    }

    pub fn route(
        &self,
        from: DestinationId,
        to: DestinationId,
    ) -> Option<&NavigationRoute> {
        self.routes.get(&(from, to))
    }

    pub fn available_routes_from(
        &self,
        from: DestinationId,
    ) -> impl Iterator<Item = &NavigationRoute> {
        self.routes
            .values()
            .filter(move |route| route.from == from && route.state == NavigationRouteState::Available)
    }

    pub fn autopilot_state(&self) -> &AutopilotState {
        &self.autopilot
    }

    pub fn destination(&self, id: DestinationId) -> Option<&Destination> {
        self.destinations.get(&id)
    }

    pub fn selected_destination(&self) -> Option<DestinationId> {
        self.selected_destination
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
        if !self.destinations.contains_key(&origin) || !self.destinations.contains_key(&destination) {
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
        if !matches!(self.autopilot, AutopilotState::Inactive | AutopilotState::Completed { .. } | AutopilotState::Interrupted { .. } | AutopilotState::Failed { .. }) {
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
                total.checked_add(self.route(pair[0], pair[1])?.fuel_cost)
            })
            .ok_or(AutopilotError::InsufficientFuel)?;
        if !fuel.can_consume(total_fuel) {
            return Err(AutopilotError::InsufficientFuel);
        }
        let leg = self
            .route(route[0], route[1])
            .copied()
            .ok_or(AutopilotError::NoRoute)?;
        if leg.state != NavigationRouteState::Available || !fuel.can_consume(leg.fuel_cost) {
            return Err(AutopilotError::InsufficientFuel);
        }
        fuel.consume(leg.fuel_cost).map_err(|_| AutopilotError::InsufficientFuel)?;
        self.travel_state = TravelState::InProgress {
            destination: leg.to,
            started_at: now,
            duration: leg.duration,
            fuel_cost: leg.fuel_cost,
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
        Ok((leg.to, now, leg.duration, leg.fuel_cost, fuel.current()))
    }

    pub(crate) fn advance_autopilot_leg(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
    ) -> Result<Option<(EntityId, EntityId, DestinationId, Duration, u32, u32)>, AutopilotError> {
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
            self.autopilot = AutopilotState::Completed { destination };
            return Ok(None);
        }
        let leg = self
            .route(route[next_leg], route[next_leg + 1])
            .copied()
            .ok_or(AutopilotError::NoRoute)?;
        if leg.state != NavigationRouteState::Available || !fuel.can_consume(leg.fuel_cost) {
            self.autopilot = AutopilotState::Failed { destination };
            return Err(AutopilotError::InsufficientFuel);
        }
        fuel.consume(leg.fuel_cost).map_err(|_| AutopilotError::InsufficientFuel)?;
        self.travel_state = TravelState::InProgress {
            destination: leg.to,
            started_at: now,
            duration: leg.duration,
            fuel_cost: leg.fuel_cost,
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
            leg.fuel_cost,
            fuel.current(),
        )))
    }

    pub(crate) fn cancel_autopilot(&mut self) -> Option<DestinationId> {
        let destination = match self.autopilot {
            AutopilotState::Planned { destination, .. }
            | AutopilotState::InTransit { destination, .. } => destination,
            _ => return None,
        };
        if matches!(self.autopilot, AutopilotState::InTransit { .. }) {
            self.travel_state = TravelState::Cancelled { destination };
        }
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
        if !fuel.can_consume(destination_data.fuel_cost) {
            return Err(NavigationStartError::InsufficientFuel);
        }
        fuel.consume(destination_data.fuel_cost)
            .map_err(|_| NavigationStartError::InsufficientFuel)?;
        self.travel_state = TravelState::InProgress {
            destination,
            started_at: now,
            duration: destination_data.travel_duration,
            fuel_cost: destination_data.fuel_cost,
        };
        Ok((
            destination,
            now,
            destination_data.travel_duration,
            destination_data.fuel_cost,
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
use crate::resources::FuelTank;
use engine::{Duration, EntityId, Vec3};
use std::collections::BTreeMap;

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
        Duration(last.0.saturating_add(self.cooldown.0).saturating_sub(now.0))
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
        }
    }
}

impl NavigationState {
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
        let range_squared = self.scanner.range * self.scanner.range;
        let mut candidates = Vec::new();
        for destination in self.destinations.values() {
            let squared = distance_squared(self.current_position, destination.position);
            if !squared.is_finite() {
                continue;
            }
            if squared <= range_squared {
                candidates.push((destination.id, squared.sqrt()));
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

    pub(crate) fn initialize_ship(&mut self, ship_id: EntityId) {
        self.ship_id = Some(ship_id);
    }

    pub(crate) fn select_destination(
        &mut self,
        destination: DestinationId,
    ) -> Result<(), NavigationSelectionError> {
        if self.has_active_travel() {
            return Err(NavigationSelectionError::TravelActive);
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

    pub(crate) fn start_travel(
        &mut self,
        now: Duration,
        fuel: &mut FuelTank,
    ) -> Result<(DestinationId, Duration, Duration, u32, u32), NavigationStartError> {
        if self.has_active_travel() {
            return Err(NavigationStartError::TravelActive);
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
pub enum NavigationSelectionError {
    TravelActive,
    DestinationNotFound,
    DestinationUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationStartError {
    TravelActive,
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
    distance_squared(first, second).sqrt()
}

fn distance_squared(first: Vec3, second: Vec3) -> f32 {
    let x = first.x - second.x;
    let y = first.y - second.y;
    let z = first.z - second.z;
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
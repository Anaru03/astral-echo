use engine::{Duration, Vec3};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HazardId(u32);

impl HazardId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentalHazardType {
    Radiation,
    SpaceStorm,
    GravitationalAnomaly,
    DebrisField,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentalHazard {
    pub id: HazardId,
    pub hazard_type: EnvironmentalHazardType,
    pub position: Vec3,
    pub radius: f32,
    pub damage: u32,
    pub interval: Duration,
    pub active: bool,
    accumulated: u64,
}

impl EnvironmentalHazard {
    pub const fn new(
        id: HazardId,
        hazard_type: EnvironmentalHazardType,
        position: Vec3,
        radius: f32,
        damage: u32,
        interval: Duration,
        active: bool,
    ) -> Self {
        Self {
            id,
            hazard_type,
            position,
            radius,
            damage,
            interval,
            active,
            accumulated: 0,
        }
    }

    pub fn validate(self) -> Result<(), EnvironmentalHazardError> {
        if self.id.value() == 0 {
            return Err(EnvironmentalHazardError::InvalidId);
        }
        if !finite_position(self.position) {
            return Err(EnvironmentalHazardError::InvalidPosition);
        }
        if !self.radius.is_finite() || self.radius <= 0.0 {
            return Err(EnvironmentalHazardError::InvalidRadius);
        }
        if self.damage == 0 || self.interval.0 == 0 {
            return Err(EnvironmentalHazardError::InvalidDamageConfiguration);
        }
        Ok(())
    }

    pub const fn accumulated(&self) -> Duration {
        Duration(self.accumulated)
    }

    pub(crate) fn restore_accumulated(&mut self, value: Duration) {
        self.accumulated = value.0;
    }

    pub(crate) fn reset_accumulated(&mut self) {
        self.accumulated = 0;
    }

    pub(crate) fn advance_accumulated(&mut self, delta: Duration) -> u64 {
        let interval = self.interval.0;
        let whole_intervals = delta.0 / interval;
        let remainder = delta.0 % interval;
        let remaining_capacity = interval - remainder;
        if self.accumulated >= remaining_capacity {
            self.accumulated -= remaining_capacity;
            whole_intervals.saturating_add(1)
        } else {
            self.accumulated += remainder;
            whole_intervals
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentalHazardError {
    InvalidId,
    DuplicateId,
    InvalidPosition,
    InvalidRadius,
    InvalidDamageConfiguration,
}

fn finite_position(position: Vec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}

pub(crate) fn distance_squared(first: Vec3, second: Vec3) -> Option<f64> {
    let dx = first.x as f64 - second.x as f64;
    let dy = first.y as f64 - second.y as f64;
    let dz = first.z as f64 - second.z as f64;
    if !dx.is_finite() || !dy.is_finite() || !dz.is_finite() {
        return None;
    }
    let squared = dx * dx + dy * dy + dz * dz;
    squared.is_finite().then_some(squared)
}
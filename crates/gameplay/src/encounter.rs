use crate::{combat::EnemyConfiguration, hazard::HazardId, navigation::DestinationId, types::ItemId};
use engine::{EntityId, Vec3};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EncounterId(u32);

impl EncounterId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialEncounterState {
    Pending,
    Active,
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialEncounterKind {
    Signal { destination: DestinationId },
    Hazard { hazard: HazardId },
    Hostile { configuration: EnemyConfiguration },
    Resource { item: ItemId, amount: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialEncounter {
    pub id: EncounterId,
    pub kind: SpatialEncounterKind,
    pub position: Vec3,
    pub activation_radius: f32,
    pub state: SpatialEncounterState,
    pub spawned_enemy: Option<EntityId>,
}

impl SpatialEncounter {
    pub const fn new(
        id: EncounterId,
        kind: SpatialEncounterKind,
        position: Vec3,
        activation_radius: f32,
    ) -> Self {
        Self {
            id,
            kind,
            position,
            activation_radius,
            state: SpatialEncounterState::Pending,
            spawned_enemy: None,
        }
    }

    pub fn validate(self) -> Result<(), SpatialEncounterError> {
        if self.id.value() == 0 {
            return Err(SpatialEncounterError::InvalidId);
        }
        if !finite_position(self.position) {
            return Err(SpatialEncounterError::InvalidPosition);
        }
        if !self.activation_radius.is_finite() || self.activation_radius <= 0.0 {
            return Err(SpatialEncounterError::InvalidRadius);
        }
        match self.kind {
            SpatialEncounterKind::Hostile { configuration } => configuration
                .validate()
                .map_err(|_| SpatialEncounterError::InvalidHostileConfiguration)?,
            SpatialEncounterKind::Resource { amount, .. } if amount == 0 => {
                return Err(SpatialEncounterError::InvalidResourceAmount)
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialEncounterError {
    InvalidId,
    DuplicateId,
    InvalidPosition,
    InvalidRadius,
    InvalidResourceAmount,
    InvalidHostileConfiguration,
    DestinationNotFound,
    HazardNotFound,
    ItemDefinitionNotFound,
}

pub(crate) fn distance_squared(first: Vec3, second: Vec3) -> Option<f64> {
    let dx = f64::from(first.x) - f64::from(second.x);
    let dy = f64::from(first.y) - f64::from(second.y);
    let dz = f64::from(first.z) - f64::from(second.z);
    let value = dx * dx + dy * dy + dz * dz;
    value.is_finite().then_some(value)
}

fn finite_position(position: Vec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}
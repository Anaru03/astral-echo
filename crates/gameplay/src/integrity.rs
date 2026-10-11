use engine::EntityId;
use crate::types::ItemId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralState {
    Operational,
    Critical,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShipIntegrity {
    pub ship_id: EntityId,
    current: u32,
    maximum: u32,
    critical_threshold: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityError {
    ShipNotFound,
    InvalidConfiguration,
    AlreadyInitialized,
    NotInitialized,
    InvalidAmount,
    Destroyed,
    ExceedsMaximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShipRepairConfiguration {
    pub material: ItemId,
    pub material_cost: u32,
    pub integrity_restored: u32,
}

impl Default for ShipRepairConfiguration {
    fn default() -> Self {
        Self {
            material: ItemId(1),
            material_cost: 1,
            integrity_restored: 20,
        }
    }
}

impl ShipRepairConfiguration {
    pub const fn new(material: ItemId, material_cost: u32, integrity_restored: u32) -> Self {
        Self {
            material,
            material_cost,
            integrity_restored,
        }
    }

    pub const fn validate(self) -> bool {
        self.material_cost > 0 && self.integrity_restored > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShipRepairError {
    PlayerNotFound,
    PlayerInactive,
    ShipNotFound,
    ShipIntegrityNotInitialized,
    ShipDestroyed,
    ShipAtMaximum,
    StationNotFound,
    StationDisabled,
    WrongStationType,
    StationNotOccupiedByPlayer,
    InventoryNotInitialized,
    RepairMaterialNotFound,
    InsufficientMaterial,
    InvalidConfiguration,
}

impl ShipIntegrity {
    pub fn new(
        ship_id: EntityId,
        maximum: u32,
        initial: u32,
        critical_threshold: u32,
    ) -> Result<Self, IntegrityError> {
        if maximum == 0
            || critical_threshold == 0
            || critical_threshold > maximum
            || initial > maximum
        {
            return Err(IntegrityError::InvalidConfiguration);
        }
        Ok(Self {
            ship_id,
            current: initial,
            maximum,
            critical_threshold,
        })
    }

    pub const fn current(self) -> u32 {
        self.current
    }
    pub const fn maximum(self) -> u32 {
        self.maximum
    }
    pub const fn critical_threshold(self) -> u32 {
        self.critical_threshold
    }
    pub const fn state(self) -> StructuralState {
        if self.current == 0 {
            StructuralState::Destroyed
        } else if self.current <= self.critical_threshold {
            StructuralState::Critical
        } else {
            StructuralState::Operational
        }
    }
    pub const fn is_operational(self) -> bool {
        self.current > 0
    }
    pub const fn is_critical(self) -> bool {
        matches!(self.state(), StructuralState::Critical)
    }
    pub const fn is_destroyed(self) -> bool {
        self.current == 0
    }

    pub fn apply_damage(
        &mut self,
        amount: u32,
    ) -> Result<(u32, StructuralState, StructuralState), IntegrityError> {
        if amount == 0 {
            return Err(IntegrityError::InvalidAmount);
        }
        if self.is_destroyed() {
            return Err(IntegrityError::Destroyed);
        }
        let before = self.state();
        self.current = self.current.saturating_sub(amount);
        Ok((self.current, before, self.state()))
    }

    pub fn repair(
        &mut self,
        amount: u32,
    ) -> Result<(u32, StructuralState, StructuralState), IntegrityError> {
        if amount == 0 {
            return Err(IntegrityError::InvalidAmount);
        }
        if self.is_destroyed() {
            return Err(IntegrityError::Destroyed);
        }
        if amount > self.maximum.saturating_sub(self.current) {
            return Err(IntegrityError::ExceedsMaximum);
        }
        let before = self.state();
        self.current += amount;
        Ok((self.current, before, self.state()))
    }
}
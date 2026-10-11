use crate::types::ItemId;
use engine::EntityId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealingEffect {
    pub health_per_unit: u32,
}

impl HealingEffect {
    pub const fn new(health_per_unit: u32) -> Self {
        Self { health_per_unit }
    }

    pub const fn is_valid(self) -> bool {
        self.health_per_unit > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealingResult {
    pub player: EntityId,
    pub item: ItemId,
    pub consumed: u32,
    pub health_before: u32,
    pub restored: u32,
    pub health_after: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealingError {
    PlayerNotFound,
    PlayerInactive,
    CombatantInvalid,
    CombatantDefeated,
    InventoryNotInitialized,
    ItemDefinitionNotFound,
    NotHealingConsumable,
    InvalidAmount,
    InsufficientQuantity,
    HealthAtMaximum,
    InvalidHealth,
    ArithmeticOverflow,
}
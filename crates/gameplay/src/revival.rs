use crate::types::ItemId;
use engine::EntityId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviveEffect {
    pub item: ItemId,
    pub units_per_revival: u32,
    pub health_restored: u32,
    pub range: u32,
}

impl ReviveEffect {
    pub const fn new(
        item: ItemId,
        units_per_revival: u32,
        health_restored: u32,
        range: u32,
    ) -> Self {
        Self {
            item,
            units_per_revival,
            health_restored,
            range,
        }
    }

    pub fn is_valid(self) -> bool {
        self.item.0 != 0 && self.units_per_revival > 0 && self.health_restored > 0 && self.range > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviveResult {
    pub helper: EntityId,
    pub target: EntityId,
    pub item: ItemId,
    pub consumed: u32,
    pub health_before: u32,
    pub restored: u32,
    pub health_after: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevivalError {
    HelperNotFound,
    HelperInactive,
    HelperInvalid,
    TargetNotFound,
    TargetInvalid,
    TargetNotDefeated,
    TargetAlreadyActive,
    InvalidPosition,
    OutOfRange,
    InventoryNotInitialized,
    ItemDefinitionNotFound,
    NotRevivalConsumable,
    InvalidConfiguration,
    InsufficientQuantity,
    ArithmeticOverflow,
}
use engine::{EntityId, Vec3};

use crate::types::ItemId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemCategory {
    RepairMaterial,
    MineralSample,
    ElectronicComponent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemId,
    pub category: ItemCategory,
    pub max_stack: u32,
}

impl ItemDefinition {
    pub const fn new(id: ItemId, category: ItemCategory, max_stack: u32) -> Self {
        Self {
            id,
            category,
            max_stack,
        }
    }

    pub fn validate(self) -> Result<(), ItemDefinitionError> {
        if self.max_stack == 0 {
            return Err(ItemDefinitionError::InvalidStackSize);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemDefinitionError {
    InvalidStackSize,
    DuplicateItem,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collectible {
    pub entity: EntityId,
    pub item: ItemId,
    pub amount: u32,
    pub position: Vec3,
    pub interaction_range: f32,
    pub active: bool,
}

impl Collectible {
    pub fn validate(self) -> Result<(), CollectibleError> {
        if self.amount == 0 {
            return Err(CollectibleError::InvalidAmount);
        }
        if !self.position.x.is_finite()
            || !self.position.y.is_finite()
            || !self.position.z.is_finite()
        {
            return Err(CollectibleError::InvalidPosition);
        }
        if !self.interaction_range.is_finite() || self.interaction_range <= 0.0 {
            return Err(CollectibleError::InvalidRange);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectibleError {
    InvalidAmount,
    InvalidPosition,
    InvalidRange,
    EntityNotFound,
    DuplicateEntity,
    ItemNotFound,
}
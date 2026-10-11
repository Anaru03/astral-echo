use crate::healing::HealingEffect;
use crate::revival::ReviveEffect;
use crate::types::ItemId;
use engine::{EntityId, Vec3};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemCategory {
    RepairMaterial,
    CombatAmmunition,
    MineralSample,
    ElectronicComponent,
    HealingConsumable,
    RevivalConsumable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemId,
    pub category: ItemCategory,
    pub max_stack: u32,
    pub healing: Option<HealingEffect>,
    pub revival: Option<ReviveEffect>,
}

impl ItemDefinition {
    pub const fn new(id: ItemId, category: ItemCategory, max_stack: u32) -> Self {
        Self {
            id,
            category,
            max_stack,
            healing: None,
            revival: None,
        }
    }

    pub const fn with_healing(mut self, effect: HealingEffect) -> Self {
        self.healing = Some(effect);
        self
    }

    pub const fn with_revival(mut self, effect: ReviveEffect) -> Self {
        self.revival = Some(effect);
        self
    }

    pub fn validate(self) -> Result<(), ItemDefinitionError> {
        if self.max_stack == 0 {
            return Err(ItemDefinitionError::InvalidStackSize);
        }
        if self.category == ItemCategory::HealingConsumable
            && !self.healing.is_some_and(|effect| effect.is_valid())
        {
            return Err(ItemDefinitionError::InvalidHealingConfiguration);
        }
        if self.category != ItemCategory::HealingConsumable && self.healing.is_some() {
            return Err(ItemDefinitionError::InvalidHealingConfiguration);
        }
        if self.category == ItemCategory::RevivalConsumable
            && !self.revival.is_some_and(|effect| effect.is_valid())
        {
            return Err(ItemDefinitionError::InvalidRevivalConfiguration);
        }
        if self.category != ItemCategory::RevivalConsumable && self.revival.is_some() {
            return Err(ItemDefinitionError::InvalidRevivalConfiguration);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemDefinitionError {
    InvalidStackSize,
    InvalidHealingConfiguration,
    InvalidRevivalConfiguration,
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
    pub(crate) fn validate_configuration(
        amount: u32,
        position: Vec3,
        interaction_range: f32,
    ) -> Result<(), CollectibleError> {
        if amount == 0 {
            return Err(CollectibleError::InvalidAmount);
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(CollectibleError::InvalidPosition);
        }
        if !interaction_range.is_finite() || interaction_range <= 0.0 {
            return Err(CollectibleError::InvalidRange);
        }
        Ok(())
    }

    pub fn validate(self) -> Result<(), CollectibleError> {
        Self::validate_configuration(self.amount, self.position, self.interaction_range)
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
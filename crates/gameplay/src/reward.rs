use crate::{combat::EnemyArchetype, types::ItemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RewardEntry {
    pub item: ItemId,
    pub amount: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnemyReward {
    pub archetype: EnemyArchetype,
    pub entries: Vec<RewardEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewardConfigurationError {
    Empty,
    InvalidAmount,
    DuplicateItem,
}

impl EnemyReward {
    pub fn new(archetype: EnemyArchetype, entries: Vec<RewardEntry>) -> Self {
        Self { archetype, entries }
    }

    pub fn validate(&self) -> Result<(), RewardConfigurationError> {
        if self.entries.is_empty() {
            return Err(RewardConfigurationError::Empty);
        }
        let mut items = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            if entry.item.0 == 0 || entry.amount == 0 {
                return Err(RewardConfigurationError::InvalidAmount);
            }
            if items.contains(&entry.item) {
                return Err(RewardConfigurationError::DuplicateItem);
            }
            items.push(entry.item);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewardError {
    InvalidConfiguration,
    DuplicateReward,
    ItemDefinitionNotFound,
    AmountExceedsStack,
    InvalidPosition,
    EnemyNotFound,
    RewardAlreadyProcessed,
}
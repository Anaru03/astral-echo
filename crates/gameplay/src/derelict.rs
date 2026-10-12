use crate::{
    combat::EnemyConfiguration,
    encounter::EncounterId,
    reward::RewardDistributionPolicy,
    types::ItemId,
};
use engine::{EntityId, Vec3};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DerelictId(u32);

impl DerelictId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerelictState {
    Undiscovered,
    Discovered,
    Exploring,
    Resolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerelictError {
    InvalidId,
    DuplicateId,
    InvalidPosition,
    InvalidRange,
    InvalidReward,
    ItemDefinitionNotFound,
    EncounterNotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerelictCommandError {
    NotFound,
    NotDiscovered,
    AlreadyDiscovered,
    AlreadyExploring,
    AlreadyResolved,
    PlayerNotFound,
    PlayerInactive,
    ShipNotOperational,
    OutOfRange,
    EncounterUnavailable,
    EnemyRegistrationFailed,
    EnemyStillActive,
    RewardNotCollected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerelictReward {
    pub item: ItemId,
    pub amount: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Derelict {
    pub id: DerelictId,
    pub position: Vec3,
    pub interaction_range: f32,
    pub state: DerelictState,
    pub associated_encounter: Option<EncounterId>,
    pub enemy_configuration: Option<EnemyConfiguration>,
    pub rewards: Vec<DerelictReward>,
    pub distribution: RewardDistributionPolicy,
    pub spawned_enemy: Option<EntityId>,
    pub enemy_defeated: bool,
    pub reward_collectibles: Vec<EntityId>,
    pub reward_generated: bool,
}

impl Derelict {
    pub fn new(id: DerelictId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            id,
            position,
            interaction_range,
            state: DerelictState::Undiscovered,
            associated_encounter: None,
            enemy_configuration: None,
            rewards: Vec::new(),
            distribution: RewardDistributionPolicy::Shared,
            spawned_enemy: None,
            enemy_defeated: false,
            reward_collectibles: Vec::new(),
            reward_generated: false,
        }
    }

    pub fn with_enemy(mut self, configuration: EnemyConfiguration) -> Self {
        self.enemy_configuration = Some(configuration);
        self
    }

    pub fn with_rewards(
        mut self,
        rewards: Vec<DerelictReward>,
        distribution: RewardDistributionPolicy,
    ) -> Self {
        self.rewards = rewards;
        self.distribution = distribution;
        self
    }

    pub fn with_encounter(mut self, encounter: EncounterId) -> Self {
        self.associated_encounter = Some(encounter);
        self
    }

    pub fn validate(&self) -> Result<(), DerelictError> {
        if self.id.value() == 0 {
            return Err(DerelictError::InvalidId);
        }
        if !finite_position(self.position) {
            return Err(DerelictError::InvalidPosition);
        }
        if !self.interaction_range.is_finite() || self.interaction_range <= 0.0 {
            return Err(DerelictError::InvalidRange);
        }
        let mut items = Vec::with_capacity(self.rewards.len());
        for reward in &self.rewards {
            if reward.item.0 == 0 || reward.amount == 0 || items.contains(&reward.item) {
                return Err(DerelictError::InvalidReward);
            }
            items.push(reward.item);
        }
        if let Some(configuration) = self.enemy_configuration {
            configuration
                .validate()
                .map_err(|_| DerelictError::InvalidReward)?;
        }
        Ok(())
    }
}

fn finite_position(position: Vec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}
use crate::combat::{EnemyConfiguration, Shield};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupSize {
    Empty,
    Solo,
    Duo,
    Trio,
}

impl GroupSize {
    pub const fn from_active_players(count: usize) -> Self {
        match count {
            0 => Self::Empty,
            1 => Self::Solo,
            2 => Self::Duo,
            _ => Self::Trio,
        }
    }

    pub const fn player_count(self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Solo => 1,
            Self::Duo => 2,
            Self::Trio => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameplayScaling {
    pub group_size: GroupSize,
    pub enemy_health_percent: u32,
    pub enemy_damage_percent: u32,
    pub resource_yield_percent: u32,
    pub reward_yield_percent: u32,
    pub maximum_enemy_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalingError {
    NoActivePlayers,
    PlayerNotFound,
    PlayerInactive,
    ArithmeticOverflow,
    InvalidResult,
}

impl GameplayScaling {
    pub const fn for_group_size(group_size: GroupSize) -> Self {
        let (health, damage, resource) = match group_size {
            GroupSize::Empty | GroupSize::Solo => (100, 100, 100),
            GroupSize::Duo => (125, 115, 110),
            GroupSize::Trio => (150, 130, 120),
        };
        Self {
            group_size,
            enemy_health_percent: health,
            enemy_damage_percent: damage,
            resource_yield_percent: resource,
            reward_yield_percent: 100,
            maximum_enemy_count: 1,
        }
    }

    pub const fn is_playable(self) -> bool {
        self.group_size.player_count() > 0
    }

    pub fn scale_amount(self, amount: u32, percent: u32) -> Result<u32, ScalingError> {
        if amount == 0 || percent == 0 {
            return Err(ScalingError::InvalidResult);
        }
        let product = amount
            .checked_mul(percent)
            .ok_or(ScalingError::ArithmeticOverflow)?;
        let scaled = product / 100;
        (scaled > 0)
            .then_some(scaled)
            .ok_or(ScalingError::InvalidResult)
    }

    pub fn scale_enemy_configuration(
        self,
        mut configuration: EnemyConfiguration,
    ) -> Result<EnemyConfiguration, ScalingError> {
        if !self.is_playable() {
            return Err(ScalingError::NoActivePlayers);
        }
        configuration.maximum_health = self.scale_amount(
            configuration.maximum_health,
            self.enemy_health_percent,
        )?;
        configuration.weapon.damage = self.scale_amount(
            configuration.weapon.damage,
            self.enemy_damage_percent,
        )?;
        if let Some(shield) = configuration.shield {
            configuration.shield = Some(Self::scale_shield(self, shield)?);
        }
        configuration
            .validate()
            .map_err(|_| ScalingError::InvalidResult)
            .map(|_| configuration)
    }

    fn scale_shield(self, shield: Shield) -> Result<Shield, ScalingError> {
        let maximum = self.scale_amount(shield.maximum, self.enemy_health_percent)?;
        let current = self.scale_amount(shield.current, self.enemy_health_percent)?.min(maximum);
        Ok(Shield {
            current,
            maximum,
            weakness: shield.weakness,
        })
    }

}
use crate::{
    commands::{CommandInvalidity, CommandRejection, CommandResult, GameplayCommand},
    events::GameplayEvent,
    reward::{EnemyReward, RewardError},
    systems::GameplaySystem,
    world::GameplayWorld,
};

pub struct RewardSystem {
    event_cursor: usize,
}

impl RewardSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn register(&self, world: &mut GameplayWorld, reward: EnemyReward) -> CommandResult {
        match world.register_enemy_reward(reward) {
            Ok(()) => CommandResult::success(true),
            Err(RewardError::DuplicateReward) => {
                CommandResult::rejected(CommandRejection::RewardAlreadyRegistered)
            }
            Err(RewardError::ItemDefinitionNotFound) => {
                CommandResult::rejected(CommandRejection::ItemDefinitionNotFound)
            }
            Err(_) => CommandResult::invalid(CommandInvalidity::InvalidRewardConfiguration),
        }
    }

    fn process_defeat(
        &self,
        world: &mut GameplayWorld,
        enemy: engine::EntityId,
        archetype: crate::combat::EnemyArchetype,
    ) {
        let Some(position) = world.combatant(enemy).map(|combatant| combatant.position) else {
            return;
        };
        let Ok(collectibles) = world.create_enemy_reward_collectibles(enemy, archetype, position)
        else {
            return;
        };
        for collectible in &collectibles {
            world.publish(GameplayEvent::RewardCollectibleCreated {
                enemy,
                collectible: collectible.entity,
                item: collectible.item,
                amount: collectible.amount,
                position: collectible.position,
            });
        }
        world.publish(GameplayEvent::EnemyRewardProcessed {
            enemy,
            archetype,
            collectibles: collectibles.len().try_into().unwrap_or(u32::MAX),
        });
    }

    fn observe_new_events(&mut self, world: &mut GameplayWorld) {
        let length = world.events().len();
        if self.event_cursor > length {
            self.event_cursor = 0;
        }
        let events: Vec<_> = world
            .events()
            .iter()
            .skip(self.event_cursor)
            .cloned()
            .collect();
        self.event_cursor = length;
        for event in events {
            if let GameplayEvent::EnemyDefeated { enemy, archetype } = event {
                self.process_defeat(world, enemy, archetype);
            }
        }
    }
}

impl Default for RewardSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for RewardSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::RegisterEnemyReward { reward } => Some(self.register(world, reward)),
            _ => None,
        }
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn reset_event_cursor(&mut self) {
        self.event_cursor = 0;
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        self.observe_new_events(world);
    }
}
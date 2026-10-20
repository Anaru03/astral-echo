use crate::{
    commands::{CommandInvalidity, CommandRejection, CommandResult, GameplayCommand},
    systems::GameplaySystem,
    world::GameplayWorld,
};

pub struct HealingSystem;

impl GameplaySystem for HealingSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let GameplayCommand::UseHealingConsumable {
            player,
            item,
            amount,
        } = command
        else {
            return None;
        };
        if world.phase() != crate::types::SimulationPhase::Running {
            return Some(CommandResult::rejected(
                CommandRejection::SimulationNotRunning,
            ));
        }
        match world.use_healing_consumable(player, item, amount) {
            Ok(_) => Some(CommandResult::success(true)),
            Err(crate::healing::HealingError::PlayerNotFound) => {
                Some(CommandResult::rejected(CommandRejection::PlayerNotFound))
            }
            Err(crate::healing::HealingError::PlayerInactive) => {
                Some(CommandResult::rejected(CommandRejection::PlayerInactive))
            }
            Err(crate::healing::HealingError::CombatantDefeated) => {
                Some(CommandResult::rejected(CommandRejection::CombatantDefeated))
            }
            Err(crate::healing::HealingError::InventoryNotInitialized) => Some(
                CommandResult::rejected(CommandRejection::InventoryNotInitialized),
            ),
            Err(crate::healing::HealingError::ItemDefinitionNotFound) => Some(
                CommandResult::rejected(CommandRejection::ItemDefinitionNotFound),
            ),
            Err(crate::healing::HealingError::NotHealingConsumable) => Some(
                CommandResult::rejected(CommandRejection::HealingNotConfigured),
            ),
            Err(crate::healing::HealingError::InsufficientQuantity) => Some(
                CommandResult::rejected(CommandRejection::InventoryInsufficientQuantity),
            ),
            Err(crate::healing::HealingError::HealthAtMaximum) => Some(CommandResult::rejected(
                CommandRejection::PlayerHealthAtMaximum,
            )),
            Err(crate::healing::HealingError::InvalidAmount) => Some(CommandResult::invalid(
                CommandInvalidity::InvalidHealingConfiguration,
            )),
            Err(
                crate::healing::HealingError::CombatantInvalid
                | crate::healing::HealingError::InvalidHealth
                | crate::healing::HealingError::ArithmeticOverflow,
            ) => Some(CommandResult::invalid(
                CommandInvalidity::InvalidHealingConfiguration,
            )),
        }
    }
}
use crate::{
    commands::{CommandInvalidity, CommandRejection, CommandResult, GameplayCommand},
    systems::GameplaySystem,
    world::GameplayWorld,
};

pub struct RevivalSystem;

impl GameplaySystem for RevivalSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let GameplayCommand::RevivePlayer {
            helper,
            target,
            item,
        } = command
        else {
            return None;
        };
        if world.phase() != crate::types::SimulationPhase::Running {
            return Some(CommandResult::rejected(
                CommandRejection::SimulationNotRunning,
            ));
        }
        match world.revive_player(helper, target, item) {
            Ok(_) => Some(CommandResult::success(true)),
            Err(crate::revival::RevivalError::HelperNotFound) => {
                Some(CommandResult::rejected(CommandRejection::PlayerNotFound))
            }
            Err(
                crate::revival::RevivalError::HelperInactive
                | crate::revival::RevivalError::HelperInvalid,
            ) => Some(CommandResult::rejected(CommandRejection::PlayerInactive)),
            Err(crate::revival::RevivalError::TargetNotFound) => {
                Some(CommandResult::rejected(CommandRejection::PlayerNotFound))
            }
            Err(crate::revival::RevivalError::TargetNotDefeated) => Some(CommandResult::rejected(
                CommandRejection::ReviveTargetNotDefeated,
            )),
            Err(crate::revival::RevivalError::TargetAlreadyActive) => Some(
                CommandResult::rejected(CommandRejection::ReviveTargetNotDefeated),
            ),
            Err(crate::revival::RevivalError::TargetInvalid) => Some(CommandResult::rejected(
                CommandRejection::ReviveTargetInvalid,
            )),
            Err(crate::revival::RevivalError::InvalidPosition) => Some(CommandResult::invalid(
                CommandInvalidity::InvalidRevivalConfiguration,
            )),
            Err(crate::revival::RevivalError::OutOfRange) => {
                Some(CommandResult::rejected(CommandRejection::ReviveOutOfRange))
            }
            Err(crate::revival::RevivalError::InventoryNotInitialized) => Some(
                CommandResult::rejected(CommandRejection::InventoryNotInitialized),
            ),
            Err(crate::revival::RevivalError::ItemDefinitionNotFound) => Some(
                CommandResult::rejected(CommandRejection::ItemDefinitionNotFound),
            ),
            Err(crate::revival::RevivalError::NotRevivalConsumable) => Some(
                CommandResult::rejected(CommandRejection::RevivalNotConfigured),
            ),
            Err(crate::revival::RevivalError::InsufficientQuantity) => Some(
                CommandResult::rejected(CommandRejection::InventoryInsufficientQuantity),
            ),
            Err(
                crate::revival::RevivalError::InvalidConfiguration
                | crate::revival::RevivalError::ArithmeticOverflow,
            ) => Some(CommandResult::invalid(
                CommandInvalidity::InvalidRevivalConfiguration,
            )),
        }
    }
}
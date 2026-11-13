use std::collections::{BTreeMap, BTreeSet};

use crate::{
    commands::{CommandResult, GameplayCommand},
    world::GameplayWorld,
};

pub trait GameplaySystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult>;

    fn observes_events(&self) -> bool {
        false
    }

    fn observe_events(&mut self, _world: &mut GameplayWorld) {}

    fn reset_event_cursor(&mut self) {}
}

pub struct TimeSystem;

impl GameplaySystem for TimeSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::AdvanceTime { delta } => {
                Some(crate::pipeline::advance_time(world, delta))
            }
            _ => None,
        }
    }
}

pub struct PlayerSystem;

pub struct CompanionSystem {
    event_cursor: usize,
}

impl CompanionSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }
}

impl GameplaySystem for CompanionSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::CreateCompanion {
                followed_player,
                position,
            } => Some(match world.create_companion(followed_player, position) {
                Ok(_) => CommandResult::success(true),
                Err(crate::companion::CompanionError::PlayerNotFound) => {
                    CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound)
                }
                Err(crate::companion::CompanionError::PlayerInactive) => CommandResult::rejected(
                    crate::commands::CommandRejection::CompanionPlayerInvalid,
                ),
                Err(crate::companion::CompanionError::InvalidPosition) => CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidInteractionData,
                ),
                Err(_) => CommandResult::rejected(
                    crate::commands::CommandRejection::CompanionInvalidTarget,
                ),
            }),
            GameplayCommand::SetCompanionBehavior {
                companion,
                behavior,
            } => Some(match world.set_companion_behavior(companion, behavior) {
                Ok(()) => CommandResult::success(true),
                Err(crate::companion::CompanionError::CompanionNotFound) => {
                    CommandResult::rejected(crate::commands::CommandRejection::CompanionNotFound)
                }
                Err(crate::companion::CompanionError::CompanionInactive) => {
                    CommandResult::rejected(crate::commands::CommandRejection::CompanionInactive)
                }
                Err(crate::companion::CompanionError::PlayerInactive) => CommandResult::rejected(
                    crate::commands::CommandRejection::CompanionPlayerInvalid,
                ),
                Err(crate::companion::CompanionError::KnowledgeTargetUnavailable) => {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::CompanionKnowledgeUnavailable,
                    )
                }
                Err(_) => CommandResult::rejected(
                    crate::commands::CommandRejection::CompanionInvalidTarget,
                ),
            }),
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
            if let crate::events::GameplayEvent::TimeAdvanced { from, to } = event {
                self.observe_time(
                    world,
                    engine::Duration(to.elapsed.0.saturating_sub(from.elapsed.0)),
                );
            }
        }
    }
}

impl CompanionSystem {
    fn observe_time(&mut self, world: &mut GameplayWorld, delta: engine::Duration) {
        world.update_companions(delta);
    }
}

impl GameplaySystem for PlayerSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::CreatePlayer => match world.create_player() {
                Ok(_) => CommandResult::success(true),
                Err(crate::world::PlayerCreationError::MaximumPlayersReached) => {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::MaximumPlayersReached,
                    )
                }
            },
            GameplayCommand::MovePlayer {
                player,
                direction,
                delta,
            } => self.move_player(world, player, direction, delta),
            GameplayCommand::RotatePlayer {
                player,
                yaw_delta_degrees,
                pitch_delta_degrees,
            } => self.rotate_player(world, player, yaw_delta_degrees, pitch_delta_degrees),
            GameplayCommand::SetPlayerActive { player, active } => {
                self.set_active(world, player, active)
            }
            _ => return None,
        };
        Some(result)
    }
}

impl PlayerSystem {
    fn player_or_rejection(
        &self,
        world: &GameplayWorld,
        entity: engine::EntityId,
    ) -> Result<crate::player::Player, CommandResult> {
        let Some(player) = world.player(entity).copied() else {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::PlayerNotFound,
            ));
        };
        if !world.entities().contains(entity) {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::PlayerInactive,
            ));
        }
        if !player.is_active {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::PlayerInactive,
            ));
        }
        Ok(player)
    }

    fn move_player(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        direction: engine::Vec3,
        delta: engine::Duration,
    ) -> CommandResult {
        let player = match self.player_or_rejection(world, entity) {
            Ok(player) => player,
            Err(result) => return result,
        };
        if delta.0 == 0 {
            return CommandResult::rejected(crate::commands::CommandRejection::InvalidDuration);
        }
        if !crate::player::is_finite_vec3(direction)
            || crate::player::magnitude_squared(direction) > 1.0001
        {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidMovementDirection,
            );
        }

        let seconds = crate::player::duration_seconds(delta);
        let input = crate::player::normalize_or_zero(direction);
        let target_velocity = engine::Vec3 {
            x: input.x * player.movement.max_speed,
            y: input.y * player.movement.max_speed,
            z: input.z * player.movement.max_speed,
        };
        let rate = if crate::player::magnitude_squared(direction) > 0.0 {
            player.movement.acceleration
        } else {
            player.movement.deceleration
        };
        let velocity =
            crate::player::move_towards(player.velocity, target_velocity, rate * seconds);
        let position = engine::Vec3 {
            x: player.position.x + velocity.x * seconds,
            y: player.position.y + velocity.y * seconds,
            z: player.position.z + velocity.z * seconds,
        };
        let changed = velocity != player.velocity || position != player.position;
        if changed {
            if let Some(updated) = world.player_mut(entity) {
                updated.velocity = velocity;
                updated.position = position;
            }
            world.publish(crate::events::GameplayEvent::PlayerMoved {
                player: player.id(),
                position,
                velocity,
            });
        }
        CommandResult::success(changed)
    }

    fn rotate_player(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        yaw_delta: f32,
        pitch_delta: f32,
    ) -> CommandResult {
        let player = match self.player_or_rejection(world, entity) {
            Ok(player) => player,
            Err(result) => return result,
        };
        if !yaw_delta.is_finite()
            || !pitch_delta.is_finite()
            || yaw_delta.abs() > 360.0
            || pitch_delta.abs() > 360.0
        {
            return CommandResult::invalid(crate::commands::CommandInvalidity::InvalidOrientation);
        }
        let yaw = normalize_yaw(player.orientation.yaw_degrees + yaw_delta);
        let pitch = (player.orientation.pitch_degrees + pitch_delta).clamp(-89.0, 89.0);
        if yaw == player.orientation.yaw_degrees && pitch == player.orientation.pitch_degrees {
            return CommandResult::success(false);
        }
        if let Some(updated) = world.player_mut(entity) {
            updated.orientation.yaw_degrees = yaw;
            updated.orientation.pitch_degrees = pitch;
        }
        world.publish(crate::events::GameplayEvent::PlayerOrientationChanged {
            player: player.id(),
            yaw_degrees: yaw,
            pitch_degrees: pitch,
        });
        CommandResult::success(true)
    }

    fn set_active(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        active: bool,
    ) -> CommandResult {
        let Some(player) = world.player(entity).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if active {
            if player.is_active && world.entities().contains(entity) {
                return CommandResult::success(false);
            }
            let Some(combatant) = world.combatant(entity).copied() else {
                return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
            };
            if !combatant.is_player || combatant.is_defeated() || combatant.maximum_health == 0 {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::CombatantDefeated,
                );
            }
            if combatant.current_health > combatant.maximum_health {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidCombatConfiguration,
                );
            }
            if !world.activate_entity(entity) {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidCombatConfiguration,
                );
            }
            if let Some(updated) = world.player_mut(entity) {
                updated.is_active = true;
            }
            if let Some(updated) = world.combatant_mut(entity) {
                updated.active = true;
            }
            world.publish(crate::events::GameplayEvent::PlayerActivated {
                player: player.id(),
            });
            CommandResult::success(true)
        } else {
            if !world.entities().contains(entity) || !player.is_active {
                return CommandResult::success(false);
            }
            world.deactivate_entity(entity);
            CommandResult::success(true)
        }
    }
}

fn normalize_yaw(mut yaw: f32) -> f32 {
    while yaw >= 180.0 {
        yaw -= 360.0;
    }
    while yaw < -180.0 {
        yaw += 360.0;
    }
    yaw
}

pub struct InteractionSystem;

impl GameplaySystem for InteractionSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::Interact { player, target } => {
                Some(self.interact(world, player, target))
            }
            _ => None,
        }
    }
}

impl InteractionSystem {
    fn interact(
        &self,
        world: &mut GameplayWorld,
        player_id: engine::EntityId,
        target: engine::EntityId,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player) = world.player(player_id).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player_id) || !player.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        let Some(interactable) = world.interactable(target).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        if !interactable.enabled {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetDisabled);
        }
        if !finite_position(player.position)
            || !finite_position(interactable.position)
            || !interactable.interaction_range.is_finite()
            || interactable.interaction_range <= 0.0
        {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInteractionData,
            );
        }
        let distance_squared = squared_distance(player.position, interactable.position);
        let range_squared = interactable.interaction_range * interactable.interaction_range;
        if !distance_squared.is_finite() || distance_squared > range_squared {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OutOfInteractionRange,
            );
        }

        let mut updated = interactable;
        if let crate::interaction::InteractionBehavior::Door { door } = interactable.behavior {
            if !world.access_requirement_satisfied(door.access_requirement) {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::DependenciesIncomplete,
                );
            }
        }
        let effect = match updated.execute(player_id) {
            Ok(effect) => effect,
            Err(crate::interaction::InteractionFailure::Door(
                crate::door::DoorTransitionError::Locked,
            )) => return CommandResult::rejected(crate::commands::CommandRejection::DoorLocked),
            Err(crate::interaction::InteractionFailure::Door(
                crate::door::DoorTransitionError::InvalidTransition,
            )) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidDoorTransition,
                )
            }
            Err(crate::interaction::InteractionFailure::Station(
                crate::station::StationError::Occupied,
            )) => {
                return CommandResult::rejected(crate::commands::CommandRejection::StationOccupied)
            }
            Err(crate::interaction::InteractionFailure::Station(
                crate::station::StationError::NotOccupiedByPlayer,
            )) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::StationNotOccupiedByPlayer,
                )
            }
        };
        if let Some(stored) = world.interactable_mut(target) {
            *stored = updated;
        } else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        }
        world.publish(crate::events::GameplayEvent::InteractionSucceeded {
            player: player.id(),
            target,
        });
        match effect {
            crate::interaction::InteractionEffect::ConsoleStateChanged { state, role } => {
                world.publish(crate::events::GameplayEvent::ConsoleStateChanged {
                    console: target,
                    state,
                    role,
                });
                if role == crate::interaction::ConsoleRole::ShipStatus {
                    world.publish(crate::events::GameplayEvent::ShipStatusEvaluated {
                        console: target,
                        role,
                    });
                }
            }
            crate::interaction::InteractionEffect::DoorStateChanged { state, role } => {
                let event = match state {
                    crate::domains::door::DoorState::Open => {
                        crate::events::GameplayEvent::DoorOpened {
                            door: target,
                            state,
                            role,
                        }
                    }
                    crate::domains::door::DoorState::Closed => {
                        crate::events::GameplayEvent::DoorClosed {
                            door: target,
                            state,
                            role,
                        }
                    }
                    _ => {
                        return CommandResult::rejected(
                            crate::commands::CommandRejection::InvalidDoorTransition,
                        )
                    }
                };
                world.publish(event);
            }
            crate::interaction::InteractionEffect::StationOccupied {
                player: occupant,
                station_type,
            } => {
                world.publish(crate::events::GameplayEvent::StationOccupied {
                    station: target,
                    player: occupant,
                    state: crate::station::StationState::Occupied,
                    station_type,
                });
            }
            crate::interaction::InteractionEffect::ElectricalPanelInteractionStarted => {
                world.publish(
                    crate::events::GameplayEvent::ElectricalPanelInteractionStarted {
                        player: player.entity_id,
                        panel: target,
                    },
                );
            }
            crate::interaction::InteractionEffect::LifeSupportPanelInteractionStarted => {
                world.publish(
                    crate::events::GameplayEvent::LifeSupportPanelInteractionStarted {
                        player: player.entity_id,
                        panel: target,
                    },
                );
            }
        }
        CommandResult::success(true)
    }
}

pub struct ElectricalSystem {
    authorized_interactions: BTreeSet<(engine::EntityId, engine::EntityId)>,
}

impl ElectricalSystem {
    pub fn new() -> Self {
        Self {
            authorized_interactions: BTreeSet::new(),
        }
    }

    fn set_breaker_state(
        &self,
        world: &mut GameplayWorld,
        player_id: engine::EntityId,
        panel_id: engine::EntityId,
        breaker_id: crate::electrical::BreakerId,
        state: crate::electrical::BreakerState,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player) = world.player(player_id).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player_id) || !player.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        let Some(interactable) = world.interactable(panel_id).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelNotFound,
            );
        };
        if !matches!(
            interactable.behavior,
            crate::interaction::InteractionBehavior::ElectricalPanel
        ) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::IncompatibleInteraction,
            );
        }
        if !interactable.enabled {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelDisabled,
            );
        }
        let Some(panel) = world.electrical_panel(panel_id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelNotFound,
            );
        };
        if panel.is_repaired() {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelRepaired,
            );
        }
        if !self
            .authorized_interactions
            .contains(&(player_id, panel_id))
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalInteractionRequired,
            );
        }
        if !finite_position(player.position)
            || !finite_position(interactable.position)
            || !interactable.interaction_range.is_finite()
            || interactable.interaction_range <= 0.0
        {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInteractionData,
            );
        }
        let distance_squared = squared_distance(player.position, interactable.position);
        let range_squared = interactable.interaction_range * interactable.interaction_range;
        if !distance_squared.is_finite() || distance_squared > range_squared {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OutOfInteractionRange,
            );
        }
        if !panel
            .breakers()
            .iter()
            .any(|breaker| breaker.id == breaker_id)
        {
            return CommandResult::rejected(crate::commands::CommandRejection::BreakerNotFound);
        }

        let Some(panel) = world.electrical_panel_mut(panel_id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelNotFound,
            );
        };
        let Some(changed) = panel.set_breaker_state(breaker_id, state) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ElectricalPanelRepaired,
            );
        };
        if !changed {
            return CommandResult::success(false);
        }
        let repaired = panel.is_repaired();
        world.publish(
            crate::events::GameplayEvent::ElectricalBreakerStateChanged {
                panel: panel_id,
                breaker: breaker_id,
                state,
            },
        );
        if repaired {
            world.publish(crate::events::GameplayEvent::ElectricalSystemRepaired {
                panel: panel_id,
            });
        }
        CommandResult::success(true)
    }
}

impl Default for ElectricalSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for ElectricalSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::CreateElectricalPanel {
                position,
                interaction_range,
                configuration,
            } => match world.create_electrical_panel(position, interaction_range, configuration) {
                Ok(panel) => {
                    world.publish(crate::events::GameplayEvent::ElectricalPanelCreated { panel });
                    CommandResult::success(true)
                }
                Err(_) => CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidElectricalConfiguration,
                ),
            },
            GameplayCommand::SetElectricalBreakerState {
                player,
                panel,
                breaker,
                state,
            } => self.set_breaker_state(world, player, panel, breaker, state),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        for event in world.events().iter() {
            if let crate::events::GameplayEvent::ElectricalPanelInteractionStarted {
                player,
                panel,
            } = event
            {
                self.authorized_interactions.insert((*player, *panel));
            }
        }
    }
}

pub struct LifeSupportSystem {
    authorized_interactions: BTreeSet<(engine::EntityId, engine::EntityId)>,
}

impl LifeSupportSystem {
    pub fn new() -> Self {
        Self {
            authorized_interactions: BTreeSet::new(),
        }
    }

    fn set_allocation(
        &self,
        world: &mut GameplayWorld,
        player_id: engine::EntityId,
        panel_id: engine::EntityId,
        compartment_id: crate::life_support::CompartmentId,
        allocation: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player) = world.player(player_id).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player_id) || !player.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        let Some(interactable) = world.interactable(panel_id).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelNotFound,
            );
        };
        if !matches!(
            interactable.behavior,
            crate::interaction::InteractionBehavior::LifeSupportPanel
        ) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::IncompatibleInteraction,
            );
        }
        if !interactable.enabled {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelDisabled,
            );
        }
        let Some(panel) = world.life_support_panel(panel_id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelNotFound,
            );
        };
        if panel.is_repaired() {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelRepaired,
            );
        }
        if !self
            .authorized_interactions
            .contains(&(player_id, panel_id))
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportInteractionRequired,
            );
        }
        if !finite_position(player.position)
            || !finite_position(interactable.position)
            || !interactable.interaction_range.is_finite()
            || interactable.interaction_range <= 0.0
        {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInteractionData,
            );
        }
        let distance_squared = squared_distance(player.position, interactable.position);
        let range_squared = interactable.interaction_range * interactable.interaction_range;
        if !distance_squared.is_finite() || distance_squared > range_squared {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OutOfInteractionRange,
            );
        }
        if !panel
            .compartments()
            .iter()
            .any(|compartment| compartment.id == compartment_id)
        {
            return CommandResult::rejected(crate::commands::CommandRejection::CompartmentNotFound);
        }

        let Some(panel) = world.life_support_panel_mut(panel_id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelNotFound,
            );
        };
        let Some(result) = panel.set_allocation(compartment_id, allocation) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelRepaired,
            );
        };
        let changed = match result {
            Ok(changed) => changed,
            Err(
                crate::life_support::LifeSupportRegistrationError::InitialAllocationOutOfBounds,
            ) => {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidLifeSupportConfiguration,
                )
            }
            Err(crate::life_support::LifeSupportRegistrationError::InitialCapacityExceeded) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::LifeSupportCapacityExceeded,
                )
            }
            Err(_) => {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidLifeSupportConfiguration,
                )
            }
        };
        if !changed {
            return CommandResult::success(false);
        }
        let repaired = panel.is_repaired();
        world.publish(crate::events::GameplayEvent::OxygenAllocationChanged {
            panel: panel_id,
            compartment: compartment_id,
            allocation,
        });
        if repaired {
            world.publish(crate::events::GameplayEvent::OxygenSystemRepaired { panel: panel_id });
        }
        CommandResult::success(true)
    }

    fn use_oxygen_resource(
        &self,
        world: &mut GameplayWorld,
        player_id: engine::EntityId,
        panel_id: engine::EntityId,
        compartment_id: crate::life_support::CompartmentId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if amount == 0 {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            );
        }
        if world.item_definition(item).is_none_or(|definition| {
            definition.category != crate::collectible::ItemCategory::Oxygen
        }) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            );
        }
        let Some(inventory) = world.inventory(player_id).cloned() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::InventoryNotInitialized,
            );
        };
        if inventory.quantity(item) < amount {
            return CommandResult::rejected(crate::commands::CommandRejection::ResourceNotOwned);
        }
        let Some(interactable) = world.interactable(panel_id).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelNotFound,
            );
        };
        let Some(player) = world.player(player_id).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        let Some(panel) = world.life_support_panel(panel_id).cloned() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportPanelNotFound,
            );
        };
        if panel.is_repaired()
            || !self
                .authorized_interactions
                .contains(&(player_id, panel_id))
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::LifeSupportInteractionRequired,
            );
        }
        if !matches!(
            interactable.behavior,
            crate::interaction::InteractionBehavior::LifeSupportPanel
        ) || !interactable.enabled
            || squared_distance(player.position, interactable.position)
                > interactable.interaction_range * interactable.interaction_range
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OutOfInteractionRange,
            );
        }
        let Some(compartment) = panel
            .compartments()
            .iter()
            .find(|value| value.id == compartment_id)
            .copied()
        else {
            return CommandResult::rejected(crate::commands::CommandRejection::CompartmentNotFound);
        };
        let available = compartment
            .maximum_allocation
            .saturating_sub(compartment.allocation);
        if available < amount {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OxygenCapacityExceeded,
            );
        }
        let new_allocation = compartment.allocation + amount;
        let mut panel_after = panel;
        if panel_after
            .set_allocation(compartment_id, new_allocation)
            .is_none_or(|result| result.is_err())
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::OxygenCapacityExceeded,
            );
        }
        let mut inventory_after = inventory;
        if inventory_after.try_remove(item, amount).is_err() {
            return CommandResult::rejected(crate::commands::CommandRejection::ResourceNotOwned);
        }
        *world.inventory_mut(player_id).expect("inventory validated") = inventory_after;
        *world
            .life_support_panel_mut(panel_id)
            .expect("panel validated") = panel_after;
        world.publish(crate::events::GameplayEvent::ItemRemoved {
            player: player_id,
            item,
            amount,
        });
        world.publish(crate::events::GameplayEvent::OxygenAllocationChanged {
            panel: panel_id,
            compartment: compartment_id,
            allocation: new_allocation,
        });
        CommandResult::success(true)
    }
}

impl Default for LifeSupportSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for LifeSupportSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::CreateLifeSupportPanel {
                position,
                interaction_range,
                configuration,
            } => {
                match world.create_life_support_panel(position, interaction_range, configuration) {
                    Ok(panel) => {
                        world.publish(crate::events::GameplayEvent::LifeSupportPanelCreated {
                            panel,
                        });
                        CommandResult::success(true)
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidLifeSupportConfiguration,
                    ),
                }
            }
            GameplayCommand::SetOxygenAllocation {
                player,
                panel,
                compartment,
                allocation,
            } => self.set_allocation(world, player, panel, compartment, allocation),
            GameplayCommand::UseOxygenResource {
                player,
                panel,
                compartment,
                item,
                amount,
            } => self.use_oxygen_resource(world, player, panel, compartment, item, amount),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        for event in world.events().iter() {
            if let crate::events::GameplayEvent::LifeSupportPanelInteractionStarted {
                player,
                panel,
            } = event
            {
                self.authorized_interactions.insert((*player, *panel));
            }
        }
    }
}

fn finite_position(position: engine::Vec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}

fn squared_distance(first: engine::Vec3, second: engine::Vec3) -> f32 {
    let delta = engine::Vec3 {
        x: first.x - second.x,
        y: first.y - second.y,
        z: first.z - second.z,
    };
    delta.x * delta.x + delta.y * delta.y + delta.z * delta.z
}

pub struct NavigationSystem {
    event_cursor: usize,
}

pub struct PilotSystem {
    event_cursor: usize,
}

impl PilotSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn configure(
        &self,
        world: &mut GameplayWorld,
        ship: engine::EntityId,
        parameters: crate::navigation::ShipMovementParameters,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if world.navigation().ship_id() != Some(ship) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        }
        if !world.entities().contains(ship) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        }
        match world.navigation_mut().configure_movement(parameters) {
            Ok(()) => CommandResult::success(true),
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipMovementConfiguration,
            ),
        }
    }

    fn control_access(
        &self,
        world: &GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
    ) -> Result<(), CommandResult> {
        validate_pilot_access(world, player, station)
    }

    fn set_active(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
        active: bool,
    ) -> CommandResult {
        if let Err(result) = self.control_access(world, player, station) {
            return result;
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if world.navigation_mut().set_control_active(active) {
            if !active {
                world.set_ship_shield_active(ship, false);
            }
            world.publish(crate::events::GameplayEvent::ShipControlChanged { ship, active });
            CommandResult::success(true)
        } else {
            CommandResult::success(false)
        }
    }

    fn accelerate(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
        input: engine::Vec3,
    ) -> CommandResult {
        if let Err(result) = self.control_access(world, player, station) {
            return result;
        }
        if !world.navigation().control_active() {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipControlInactive);
        }
        match world.navigation_mut().set_acceleration_input(input) {
            Ok(changed) => CommandResult::success(changed),
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipMovementConfiguration,
            ),
        }
    }

    fn rotate(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
        yaw_input: f32,
        pitch_input: f32,
        delta: engine::Duration,
    ) -> CommandResult {
        if let Err(result) = self.control_access(world, player, station) {
            return result;
        }
        if !world.navigation().control_active() {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipControlInactive);
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        match world.navigation_mut().rotate(yaw_input, pitch_input, delta) {
            Ok(false) => CommandResult::success(false),
            Ok(true) => {
                let orientation = world.navigation().orientation();
                world.publish(crate::events::GameplayEvent::ShipOrientationChanged {
                    ship,
                    yaw_degrees: orientation.yaw_degrees,
                    pitch_degrees: orientation.pitch_degrees,
                });
                CommandResult::success(true)
            }
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipMovementConfiguration,
            ),
        }
    }

    fn brake(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.control_access(world, player, station) {
            return result;
        }
        if !world.navigation().control_active() {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipControlInactive);
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if world.navigation_mut().brake() {
            world.publish(crate::events::GameplayEvent::ShipBraked { ship });
            CommandResult::success(true)
        } else {
            CommandResult::success(false)
        }
    }

    fn boost(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        station: engine::EntityId,
        active: bool,
    ) -> CommandResult {
        if let Err(result) = self.control_access(world, player, station) {
            return result;
        }
        if !world.navigation().control_active() {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipControlInactive);
        }
        CommandResult::success(world.set_ship_boost_active(active))
    }
}

impl Default for PilotSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for PilotSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::ConfigureShipMovement { ship, parameters } => {
                self.configure(world, ship, parameters)
            }
            GameplayCommand::AccelerateShip {
                player,
                pilot_station,
                input,
            } => self.accelerate(world, player, pilot_station, input),
            GameplayCommand::RotateShip {
                player,
                pilot_station,
                yaw_input,
                pitch_input,
                delta,
            } => self.rotate(world, player, pilot_station, yaw_input, pitch_input, delta),
            GameplayCommand::BrakeShip {
                player,
                pilot_station,
            } => self.brake(world, player, pilot_station),
            GameplayCommand::SetShipBoost {
                player,
                pilot_station,
                active,
            } => self.boost(world, player, pilot_station, active),
            GameplayCommand::SetShipControlActive {
                player,
                pilot_station,
                active,
            } => self.set_active(world, player, pilot_station, active),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn reset_event_cursor(&mut self) {
        self.event_cursor = 0;
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
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
            let crate::events::GameplayEvent::TimeAdvanced { from, to } = event else {
                continue;
            };
            let Some(ship) = world.navigation().ship_id() else {
                continue;
            };
            if world.ship_is_destroyed(ship) == Some(true)
                || world.navigation().planet_flight_state().is_terminal()
            {
                continue;
            }
            if world.navigation().has_active_travel() {
                continue;
            }
            let delta = engine::Duration(to.elapsed.0.saturating_sub(from.elapsed.0));
            let Some((before, after, position_changed, contact)) =
                world.navigation_mut().advance_manual(delta, to.elapsed)
            else {
                continue;
            };
            if before != after {
                world.publish(crate::events::GameplayEvent::ShipVelocityChanged {
                    ship,
                    velocity: after,
                });
            }
            if position_changed {
                world.publish(crate::events::GameplayEvent::ShipPositionChanged {
                    ship,
                    position: world.navigation().current_position(),
                });
            }
            if let Some((planet, impact)) = contact {
                if impact {
                    let position = world.navigation().current_position();
                    process_navigation_impact(world, ship, planet, position);
                } else if matches!(
                    world.navigation().planet_flight_state(),
                    crate::navigation::PlanetFlightState::Landed { .. }
                ) {
                    world.publish(crate::events::GameplayEvent::PlanetLanded {
                        ship,
                        planet,
                        position: world.navigation().current_position(),
                    });
                }
            }
        }
    }
}

pub struct ScannerSystem;

impl GameplaySystem for ScannerSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::ConfigureSignalScanner {
                ship,
                range,
                cooldown,
            } => self.configure(world, ship, range, cooldown),
            GameplayCommand::SetSignalScannerEnabled { ship, enabled } => {
                self.set_enabled(world, ship, enabled)
            }
            GameplayCommand::ScanSignals { player, ship } => self.scan(world, player, ship),
            _ => return None,
        };
        Some(result)
    }
}

impl ScannerSystem {
    fn validate_ship(
        &self,
        world: &GameplayWorld,
        ship: engine::EntityId,
    ) -> Result<(), CommandResult> {
        if world.navigation().ship_id() != Some(ship) || !world.entities().contains(ship) {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::ScannerNotFound,
            ));
        }
        Ok(())
    }

    fn configure(
        &self,
        world: &mut GameplayWorld,
        ship: engine::EntityId,
        range: f32,
        cooldown: engine::Duration,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if let Err(result) = self.validate_ship(world, ship) {
            return result;
        }
        match world.navigation_mut().configure_scanner(range, cooldown) {
            Ok(()) => CommandResult::success(true),
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidScannerConfiguration,
            ),
        }
    }

    fn set_enabled(
        &self,
        world: &mut GameplayWorld,
        ship: engine::EntityId,
        enabled: bool,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if let Err(result) = self.validate_ship(world, ship) {
            return result;
        }
        CommandResult::success(world.navigation_mut().set_scanner_enabled(enabled))
    }

    fn scan(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        ship: engine::EntityId,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player) || !player_state.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        if let Err(result) = self.validate_ship(world, ship) {
            return result;
        }
        if world.ship_is_destroyed(ship) == Some(true) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed);
        }
        let now = world.time().elapsed;
        let transitions = match world.navigation_mut().scan_signals(now) {
            Ok(transitions) => transitions,
            Err(crate::navigation::ScannerError::Disabled) => {
                return CommandResult::rejected(crate::commands::CommandRejection::ScannerDisabled)
            }
            Err(crate::navigation::ScannerError::Cooldown) => {
                return CommandResult::rejected(crate::commands::CommandRejection::ScannerCooldown)
            }
            Err(_) => {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidScannerConfiguration,
                )
            }
        };
        let detected = world.navigation().last_scan_results().len() as u32;
        world.publish(crate::events::GameplayEvent::SignalScanCompleted {
            ship,
            detected,
            at: now,
        });
        for (destination, previous, next, distance) in transitions {
            if previous == crate::navigation::SignalKnowledge::Unknown
                && next >= crate::navigation::SignalKnowledge::Detected
            {
                world.publish(crate::events::GameplayEvent::SignalDetected {
                    ship,
                    destination,
                    distance,
                });
            }
            if previous < crate::navigation::SignalKnowledge::Identified
                && next >= crate::navigation::SignalKnowledge::Identified
            {
                world.publish(crate::events::GameplayEvent::DestinationIdentified {
                    ship,
                    destination,
                });
            }
        }
        CommandResult::success(true)
    }
}

pub struct EnvironmentalHazardSystem {
    event_cursor: usize,
}

impl EnvironmentalHazardSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn apply_time_advance(
        &mut self,
        world: &mut GameplayWorld,
        from: engine::GameTime,
        to: engine::GameTime,
    ) {
        let Some(ship) = world.navigation().ship_id() else {
            return;
        };
        if !world.entities().contains(ship) || world.ship_is_destroyed(ship) == Some(true) {
            return;
        }
        let delta = engine::Duration(to.elapsed.0.saturating_sub(from.elapsed.0));
        if delta.0 == 0 {
            return;
        }
        let hazard_ids: Vec<_> = world
            .environmental_hazards()
            .map(|hazard| hazard.id)
            .collect();
        let ship_position = world.navigation().current_position();
        for hazard_id in hazard_ids {
            if world.ship_is_destroyed(ship) == Some(true) {
                break;
            }
            let Some(hazard) = world.environmental_hazard(hazard_id) else {
                continue;
            };
            if !hazard.active {
                if let Some(hazard_mut) = world.environmental_hazard_mut(hazard_id) {
                    hazard_mut.reset_accumulated();
                }
                continue;
            }
            let inside = crate::hazard::distance_squared(ship_position, hazard.position)
                .is_some_and(|distance| distance <= hazard.radius as f64 * hazard.radius as f64);
            if !inside {
                if let Some(hazard_mut) = world.environmental_hazard_mut(hazard_id) {
                    hazard_mut.reset_accumulated();
                }
                continue;
            }
            let intervals = world
                .environmental_hazard_mut(hazard_id)
                .map_or(0, |hazard_mut| hazard_mut.advance_accumulated(delta));
            if intervals == 0 {
                continue;
            }
            let attempted_damage = hazard
                .damage
                .saturating_mul(intervals.min(u32::MAX as u64) as u32);
            let Some(before) = world.ship_integrity(ship) else {
                continue;
            };
            let Ok((remaining, _, _)) = world.apply_ship_damage(ship, attempted_damage) else {
                continue;
            };
            world.publish(crate::events::GameplayEvent::ShipDamagedByHazard {
                hazard: hazard.id,
                hazard_type: hazard.hazard_type,
                ship,
                attempted_damage,
                applied_damage: before.current().saturating_sub(remaining),
                remaining_integrity: remaining,
                intervals,
            });
        }
    }
}

impl Default for EnvironmentalHazardSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for EnvironmentalHazardSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::RegisterEnvironmentalHazard { hazard } => {
                if world.phase() != crate::types::SimulationPhase::Running {
                    return Some(CommandResult::rejected(
                        crate::commands::CommandRejection::SimulationNotRunning,
                    ));
                }
                Some(match world.register_environmental_hazard(hazard) {
                    Ok(()) => CommandResult::success(true),
                    Err(crate::hazard::EnvironmentalHazardError::DuplicateId) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::EnvironmentalHazardAlreadyRegistered,
                        )
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidEnvironmentalHazardConfiguration,
                    ),
                })
            }
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
            if let crate::events::GameplayEvent::TimeAdvanced { from, to } = event {
                self.apply_time_advance(world, from, to);
            }
        }
    }
}

pub struct HullIntegritySystem;

pub struct DerelictSystem {
    event_cursor: usize,
}

impl DerelictSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn player_can_access(
        world: &GameplayWorld,
        player: engine::EntityId,
        derelict: &crate::derelict::Derelict,
    ) -> Result<(), crate::derelict::DerelictCommandError> {
        let Some(player_state) = world.player(player) else {
            return Err(crate::derelict::DerelictCommandError::PlayerNotFound);
        };
        let Some(combatant) = world.combatant(player) else {
            return Err(crate::derelict::DerelictCommandError::PlayerInactive);
        };
        if !world.entities().contains(player)
            || !player_state.is_active
            || !combatant.is_player
            || combatant.is_defeated()
        {
            return Err(crate::derelict::DerelictCommandError::PlayerInactive);
        }
        let Some(ship) = world.navigation().ship_id() else {
            return Err(crate::derelict::DerelictCommandError::ShipNotOperational);
        };
        if !world.ship_is_operational(ship).unwrap_or(false) {
            return Err(crate::derelict::DerelictCommandError::ShipNotOperational);
        }
        let Some(position) = world.ship_position() else {
            return Err(crate::derelict::DerelictCommandError::OutOfRange);
        };
        if crate::encounter::distance_squared(position, derelict.position).is_none_or(|distance| {
            distance > f64::from(derelict.interaction_range) * f64::from(derelict.interaction_range)
        }) {
            return Err(crate::derelict::DerelictCommandError::OutOfRange);
        }
        Ok(())
    }

    fn result(error: crate::derelict::DerelictCommandError) -> CommandResult {
        use crate::commands::CommandRejection;
        let reason = match error {
            crate::derelict::DerelictCommandError::NotFound => CommandRejection::DerelictNotFound,
            crate::derelict::DerelictCommandError::NotDiscovered => {
                CommandRejection::DerelictNotDiscovered
            }
            crate::derelict::DerelictCommandError::AlreadyDiscovered => {
                CommandRejection::DerelictAlreadyExploring
            }
            crate::derelict::DerelictCommandError::AlreadyExploring => {
                CommandRejection::DerelictAlreadyExploring
            }
            crate::derelict::DerelictCommandError::AlreadyResolved => {
                CommandRejection::DerelictAlreadyResolved
            }
            crate::derelict::DerelictCommandError::PlayerNotFound => {
                CommandRejection::PlayerNotFound
            }
            crate::derelict::DerelictCommandError::PlayerInactive => {
                CommandRejection::DerelictRequiresActivePlayer
            }
            crate::derelict::DerelictCommandError::ShipNotOperational => {
                CommandRejection::DerelictShipNotOperational
            }
            crate::derelict::DerelictCommandError::OutOfRange => {
                CommandRejection::DerelictOutOfRange
            }
            crate::derelict::DerelictCommandError::EncounterUnavailable => {
                CommandRejection::DerelictEncounterUnavailable
            }
            crate::derelict::DerelictCommandError::EnemyRegistrationFailed => {
                CommandRejection::DerelictEnemyRegistrationFailed
            }
            crate::derelict::DerelictCommandError::EnemyStillActive => {
                CommandRejection::DerelictEnemyStillActive
            }
            crate::derelict::DerelictCommandError::RewardNotCollected => {
                CommandRejection::DerelictRewardNotCollected
            }
        };
        CommandResult::rejected(reason)
    }

    fn discover(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        id: crate::derelict::DerelictId,
    ) -> CommandResult {
        let Some(derelict) = world.derelict(id).cloned() else {
            return Self::result(crate::derelict::DerelictCommandError::NotFound);
        };
        if let Err(error) = Self::player_can_access(world, player, &derelict) {
            return Self::result(error);
        }
        if derelict.state != crate::derelict::DerelictState::Undiscovered {
            return CommandResult::success(false);
        }
        if let Some(updated) = world.derelict_mut(id) {
            updated.state = crate::derelict::DerelictState::Discovered;
        }
        world.publish(crate::events::GameplayEvent::DerelictDiscovered {
            derelict: id,
            player,
        });
        CommandResult::success(true)
    }

    fn generate_reward(
        &self,
        world: &mut GameplayWorld,
        id: crate::derelict::DerelictId,
    ) -> Result<(), crate::derelict::DerelictCommandError> {
        let Some(derelict) = world.derelict(id).cloned() else {
            return Err(crate::derelict::DerelictCommandError::NotFound);
        };
        if derelict.reward_generated {
            return Ok(());
        }
        let created = world
            .create_derelict_collectibles(id)
            .map_err(|_| crate::derelict::DerelictCommandError::RewardNotCollected)?;
        if let Some(updated) = world.derelict_mut(id) {
            updated.reward_collectibles = created.iter().map(|entry| entry.0).collect();
            updated.reward_generated = true;
        }
        for (collectible, item, amount, owner) in created {
            world.publish(crate::events::GameplayEvent::CollectibleCreated {
                collectible,
                item,
                amount,
                position: derelict.position,
            });
            world.publish(
                crate::events::GameplayEvent::DerelictRewardCollectibleCreated {
                    derelict: id,
                    collectible,
                    item,
                    amount,
                    owner,
                },
            );
        }
        Ok(())
    }

    fn explore(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        id: crate::derelict::DerelictId,
    ) -> CommandResult {
        let Some(derelict) = world.derelict(id).cloned() else {
            return Self::result(crate::derelict::DerelictCommandError::NotFound);
        };
        if let Err(error) = Self::player_can_access(world, player, &derelict) {
            return Self::result(error);
        }
        match derelict.state {
            crate::derelict::DerelictState::Undiscovered => {
                return Self::result(crate::derelict::DerelictCommandError::NotDiscovered)
            }
            crate::derelict::DerelictState::Exploring => {
                return Self::result(crate::derelict::DerelictCommandError::AlreadyExploring)
            }
            crate::derelict::DerelictState::Resolved => {
                return Self::result(crate::derelict::DerelictCommandError::AlreadyResolved)
            }
            crate::derelict::DerelictState::Discovered => {}
        }
        if let Some(encounter) = derelict.associated_encounter {
            let result = SpatialEncounterSystem::activate_for_gameplay(world, encounter);
            if !matches!(
                result.status,
                crate::commands::CommandStatus::Success { .. }
            ) {
                return Self::result(crate::derelict::DerelictCommandError::EncounterUnavailable);
            }
        }
        let spawned_enemy = if let Some(configuration) = derelict.enemy_configuration {
            let Ok(configuration) = world.scaled_enemy_configuration(configuration) else {
                return Self::result(
                    crate::derelict::DerelictCommandError::EnemyRegistrationFailed,
                );
            };
            let Ok(enemy) = world.register_enemy(configuration) else {
                return Self::result(
                    crate::derelict::DerelictCommandError::EnemyRegistrationFailed,
                );
            };
            world.publish(crate::events::GameplayEvent::EnemyCreated {
                enemy,
                archetype: configuration.archetype,
                position: configuration.position,
            });
            Some(enemy)
        } else {
            None
        };
        if let Some(updated) = world.derelict_mut(id) {
            updated.state = crate::derelict::DerelictState::Exploring;
            updated.spawned_enemy = spawned_enemy;
            updated.enemy_defeated = spawned_enemy.is_none();
        }
        world.publish(crate::events::GameplayEvent::DerelictExplorationStarted {
            derelict: id,
            player,
        });
        world.publish(crate::events::GameplayEvent::DerelictContentActivated { derelict: id });
        if spawned_enemy.is_none() && self.generate_reward(world, id).is_err() {
            return Self::result(crate::derelict::DerelictCommandError::RewardNotCollected);
        }
        CommandResult::success(true)
    }

    fn resolve(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        id: crate::derelict::DerelictId,
    ) -> CommandResult {
        let Some(derelict) = world.derelict(id).cloned() else {
            return Self::result(crate::derelict::DerelictCommandError::NotFound);
        };
        if let Err(error) = Self::player_can_access(world, player, &derelict) {
            return Self::result(error);
        }
        if derelict.state == crate::derelict::DerelictState::Resolved {
            return Self::result(crate::derelict::DerelictCommandError::AlreadyResolved);
        }
        if derelict.state != crate::derelict::DerelictState::Exploring {
            return Self::result(crate::derelict::DerelictCommandError::NotDiscovered);
        }
        if derelict.spawned_enemy.is_some() && !derelict.enemy_defeated {
            return Self::result(crate::derelict::DerelictCommandError::EnemyStillActive);
        }
        if !derelict.reward_generated {
            return Self::result(crate::derelict::DerelictCommandError::RewardNotCollected);
        }
        if derelict.reward_collectibles.iter().any(|collectible| {
            world
                .collectible(*collectible)
                .is_some_and(|value| value.active)
        }) {
            return Self::result(crate::derelict::DerelictCommandError::RewardNotCollected);
        }
        if let Some(updated) = world.derelict_mut(id) {
            updated.state = crate::derelict::DerelictState::Resolved;
        }
        world.publish(crate::events::GameplayEvent::DerelictResolved {
            derelict: id,
            player,
        });
        CommandResult::success(true)
    }
}

impl Default for DerelictSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for DerelictSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::RegisterDerelict { derelict } => {
                Some(match world.register_derelict(derelict) {
                    Ok(()) => CommandResult::success(true),
                    Err(crate::derelict::DerelictError::DuplicateId) => CommandResult::rejected(
                        crate::commands::CommandRejection::DerelictAlreadyRegistered,
                    ),
                    Err(crate::derelict::DerelictError::EncounterNotFound) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::DerelictEncounterUnavailable,
                        )
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidDerelictConfiguration,
                    ),
                })
            }
            GameplayCommand::DiscoverDerelict { player, derelict } => {
                Some(self.discover(world, player, derelict))
            }
            GameplayCommand::ExploreDerelict { player, derelict } => {
                Some(self.explore(world, player, derelict))
            }
            GameplayCommand::ResolveDerelict { player, derelict } => {
                Some(self.resolve(world, player, derelict))
            }
            _ => None,
        }
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
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
            if let crate::events::GameplayEvent::EnemyDefeated { enemy, .. } = event {
                let ids: Vec<_> = world
                    .derelicts()
                    .filter(|derelict| derelict.spawned_enemy == Some(enemy))
                    .map(|derelict| derelict.id)
                    .collect();
                for id in ids {
                    if let Some(updated) = world.derelict_mut(id) {
                        updated.enemy_defeated = true;
                    }
                    let _ = self.generate_reward(world, id);
                }
            }
        }
    }
}

pub struct FunctionalWorldSystem {
    event_cursor: usize,
}

impl FunctionalWorldSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn map_error(error: crate::world_activity::WorldActivityError) -> CommandResult {
        use crate::commands::CommandRejection;
        let reason = match error {
            crate::world_activity::WorldActivityError::WorldNotFound => {
                CommandRejection::FunctionalWorldNotFound
            }
            crate::world_activity::WorldActivityError::DestinationNotDiscovered => {
                CommandRejection::FunctionalWorldDestinationNotDiscovered
            }
            crate::world_activity::WorldActivityError::PlayerNotFound => {
                CommandRejection::PlayerNotFound
            }
            crate::world_activity::WorldActivityError::PlayerInactive => {
                CommandRejection::FunctionalWorldRequiresActivePlayer
            }
            crate::world_activity::WorldActivityError::ShipNotOperational => {
                CommandRejection::FunctionalWorldShipNotOperational
            }
            crate::world_activity::WorldActivityError::OutOfRange => {
                CommandRejection::FunctionalWorldOutOfRange
            }
            crate::world_activity::WorldActivityError::ActivityNotFound => {
                CommandRejection::WorldActivityNotFound
            }
            crate::world_activity::WorldActivityError::InvalidQuantity => {
                CommandRejection::WorldInvalidQuantity
            }
            crate::world_activity::WorldActivityError::InvalidItem => {
                CommandRejection::ItemDefinitionNotFound
            }
            crate::world_activity::WorldActivityError::InsufficientFunds => {
                CommandRejection::WorldInsufficientFunds
            }
            crate::world_activity::WorldActivityError::PriceOverflow => {
                CommandRejection::WorldPriceOverflow
            }
            crate::world_activity::WorldActivityError::InventoryCapacity => {
                CommandRejection::WorldInventoryCapacity
            }
            crate::world_activity::WorldActivityError::InventoryInsufficient => {
                CommandRejection::WorldInventoryInsufficient
            }
            crate::world_activity::WorldActivityError::ResourceUnavailable => {
                CommandRejection::WorldResourceUnavailable
            }
            crate::world_activity::WorldActivityError::CombatAlreadyActivated => {
                CommandRejection::WorldCombatAlreadyActivated
            }
            crate::world_activity::WorldActivityError::CombatResolved => {
                CommandRejection::WorldCombatResolved
            }
            crate::world_activity::WorldActivityError::StoryAlreadyInteracted => {
                CommandRejection::WorldStoryAlreadyInteracted
            }
            crate::world_activity::WorldActivityError::StoryKnowledgeUnavailable => {
                CommandRejection::WorldStoryKnowledgeUnavailable
            }
            crate::world_activity::WorldActivityError::EnemyCreationFailed => {
                CommandRejection::WorldEnemyCreationFailed
            }
            crate::world_activity::WorldActivityError::UpgradeAlreadyInstalled => {
                CommandRejection::ShipUpgradeAlreadyInstalled
            }
        };
        CommandResult::rejected(reason)
    }
}

impl Default for FunctionalWorldSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for FunctionalWorldSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::RegisterFunctionalWorld {
                world: functional_world,
            } => Some(match world.register_functional_world(functional_world) {
                Ok(()) => CommandResult::success(true),
                Err(crate::world_activity::FunctionalWorldError::DuplicateDestination) => {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::FunctionalWorldAlreadyRegistered,
                    )
                }
                Err(crate::world_activity::FunctionalWorldError::DestinationNotDiscovered) => {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::FunctionalWorldDestinationNotDiscovered,
                    )
                }
                Err(crate::world_activity::FunctionalWorldError::DestinationNotFound) => {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::FunctionalWorldNotFound,
                    )
                }
                Err(_) => CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidFunctionalWorldConfiguration,
                ),
            }),
            GameplayCommand::ExtractWorldResource {
                player,
                destination,
            } => Some(match world.extract_world_resource(player, destination) {
                Ok(_) => CommandResult::success(true),
                Err(error) => Self::map_error(error),
            }),
            GameplayCommand::BuyWorldItem {
                player,
                destination,
                item,
                amount,
            } => Some(
                match world.buy_world_item(player, destination, item, amount) {
                    Ok(_) => CommandResult::success(true),
                    Err(error) => Self::map_error(error),
                },
            ),
            GameplayCommand::SellWorldItem {
                player,
                destination,
                item,
                amount,
            } => Some(
                match world.sell_world_item(player, destination, item, amount) {
                    Ok(_) => CommandResult::success(true),
                    Err(error) => Self::map_error(error),
                },
            ),
            GameplayCommand::InteractStoryWorld {
                player,
                destination,
            } => Some(match world.interact_story_world(player, destination) {
                Ok(_) => CommandResult::success(true),
                Err(error) => Self::map_error(error),
            }),
            GameplayCommand::ActivateWorldCombat {
                player,
                destination,
            } => Some(match world.activate_world_combat(player, destination) {
                Ok(_) => CommandResult::success(true),
                Err(error) => Self::map_error(error),
            }),
            GameplayCommand::InstallShipUpgrade {
                player,
                ship,
                destination,
                upgrade,
            } => Some(
                match world.install_ship_upgrade(player, ship, destination, upgrade) {
                    Ok(()) => CommandResult::success(true),
                    Err(error) => Self::map_error(error),
                },
            ),
            _ => None,
        }
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
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
            if let crate::events::GameplayEvent::EnemyDefeated { enemy, .. } = event {
                let destinations: Vec<_> = world
                    .functional_worlds()
                    .filter(|functional_world| {
                        functional_world.activities.iter().any(|activity| {
                            matches!(activity, crate::world_activity::WorldActivity::Combat(zone) if zone.active_enemy == Some(enemy))
                        })
                    })
                    .map(|functional_world| functional_world.destination)
                    .collect();
                for destination in destinations {
                    if let Some(functional_world) = world.functional_world_mut(destination) {
                        for activity in &mut functional_world.activities {
                            if let crate::world_activity::WorldActivity::Combat(zone) = activity {
                                if zone.active_enemy == Some(enemy) {
                                    zone.active_enemy = None;
                                    zone.resolved = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct SpatialEncounterSystem {
    event_cursor: usize,
}

impl SpatialEncounterSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    pub(crate) fn activate_for_gameplay(
        world: &mut GameplayWorld,
        id: crate::encounter::EncounterId,
    ) -> CommandResult {
        Self::new().activate(world, id)
    }

    fn within_range(world: &GameplayWorld, encounter: &crate::encounter::SpatialEncounter) -> bool {
        let Some(position) = world.ship_position() else {
            return false;
        };
        crate::encounter::distance_squared(position, encounter.position).is_some_and(|distance| {
            distance
                <= f64::from(encounter.activation_radius) * f64::from(encounter.activation_radius)
        })
    }

    fn activate(
        &self,
        world: &mut GameplayWorld,
        id: crate::encounter::EncounterId,
    ) -> CommandResult {
        let Some(encounter) = world.spatial_encounter(id).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterNotFound,
            );
        };
        if encounter.state == crate::encounter::SpatialEncounterState::Active {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterAlreadyActive,
            );
        }
        if encounter.state == crate::encounter::SpatialEncounterState::Resolved {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterAlreadyResolved,
            );
        }
        if !Self::within_range(world, &encounter) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterOutOfRange,
            );
        }
        let spawned_enemy = match encounter.kind {
            crate::encounter::SpatialEncounterKind::Hostile { configuration } => {
                let Ok(configuration) = world.scaled_enemy_configuration(configuration) else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::SpatialEncounterResolutionFailed,
                    );
                };
                let Ok(enemy) = world.register_enemy(configuration) else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::SpatialEncounterResolutionFailed,
                    );
                };
                world.publish(crate::events::GameplayEvent::EnemyCreated {
                    enemy,
                    archetype: configuration.archetype,
                    position: configuration.position,
                });
                Some(enemy)
            }
            _ => None,
        };
        let Some(updated) = world.spatial_encounter_mut(id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterNotFound,
            );
        };
        updated.state = crate::encounter::SpatialEncounterState::Active;
        updated.spawned_enemy = spawned_enemy;
        if matches!(
            updated.kind,
            crate::encounter::SpatialEncounterKind::Hostile { .. }
        ) && matches!(
            world.autopilot_state(),
            crate::navigation::AutopilotState::InTransit { .. }
        ) {
            world
                .navigation_mut()
                .set_autopilot_continuation_authorized(false);
        }
        world.publish(crate::events::GameplayEvent::SpatialEncounterActivated { encounter: id });
        CommandResult::success(true)
    }

    fn resolve(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        id: crate::encounter::EncounterId,
    ) -> CommandResult {
        let Some(encounter) = world.spatial_encounter(id).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterNotFound,
            );
        };
        if encounter.state == crate::encounter::SpatialEncounterState::Resolved {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterAlreadyResolved,
            );
        }
        if encounter.state != crate::encounter::SpatialEncounterState::Active {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterRequiresScanner,
            );
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        let Some(combatant) = world.combatant(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player)
            || !player_state.is_active
            || !combatant.is_player
            || combatant.is_defeated()
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterRequiresActivePlayer,
            );
        }
        if !Self::within_range(world, &encounter) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterOutOfRange,
            );
        }
        match encounter.kind {
            crate::encounter::SpatialEncounterKind::Signal { destination } => {
                if !world
                    .knowledge(crate::knowledge::KnowledgeTarget::Destination(destination))
                    .is_identified()
                {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::SpatialEncounterRequiresScanner,
                    );
                }
            }
            crate::encounter::SpatialEncounterKind::Hazard { hazard } => {
                let Some(ship) = world.navigation().ship_id() else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::ShipNotFound,
                    );
                };
                let Some(hazard_data) = world.environmental_hazard(hazard) else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::EnvironmentalHazardNotFound,
                    );
                };
                if world.ship_is_destroyed(ship) == Some(true) || !hazard_data.active {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::ShipDestroyed,
                    );
                }
                if world.apply_ship_damage(ship, hazard_data.damage).is_err() {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::SpatialEncounterResolutionFailed,
                    );
                }
            }
            crate::encounter::SpatialEncounterKind::Resource { item, amount } => {
                let Some(position) = world.ship_position() else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::ShipNotFound,
                    );
                };
                let Ok(collectible) = world.create_collectible(item, amount, position, 2.0) else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::SpatialEncounterResolutionFailed,
                    );
                };
                world.publish(crate::events::GameplayEvent::CollectibleCreated {
                    collectible,
                    item,
                    amount,
                    position,
                });
            }
            crate::encounter::SpatialEncounterKind::Hostile { .. } => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::SpatialEncounterRequiresScanner,
                )
            }
        }
        let Some(updated) = world.spatial_encounter_mut(id) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SpatialEncounterNotFound,
            );
        };
        updated.state = crate::encounter::SpatialEncounterState::Resolved;
        world.publish(crate::events::GameplayEvent::SpatialEncounterResolved { encounter: id });
        CommandResult::success(true)
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
            match event {
                crate::events::GameplayEvent::TravelArrived { .. } => {
                    let ids: Vec<_> = world
                        .spatial_encounters()
                        .filter(|encounter| {
                            encounter.state == crate::encounter::SpatialEncounterState::Pending
                                && Self::within_range(world, encounter)
                        })
                        .map(|encounter| encounter.id)
                        .collect();
                    for id in ids {
                        let _ = self.activate(world, id);
                    }
                }
                crate::events::GameplayEvent::DestinationIdentified { destination, .. }
                | crate::events::GameplayEvent::KnowledgeIdentified {
                    target: crate::knowledge::KnowledgeTarget::Destination(destination),
                } => {
                    let ids: Vec<_> = world
                        .spatial_encounters()
                        .filter(|encounter| {
                            encounter.state == crate::encounter::SpatialEncounterState::Active
                                && matches!(
                                    encounter.kind,
                                    crate::encounter::SpatialEncounterKind::Signal {
                                        destination: expected
                                    } if expected == destination
                                )
                        })
                        .map(|encounter| encounter.id)
                        .collect();
                    for id in ids {
                        if let Some(updated) = world.spatial_encounter_mut(id) {
                            updated.state = crate::encounter::SpatialEncounterState::Resolved;
                            world.publish(crate::events::GameplayEvent::SpatialEncounterResolved {
                                encounter: id,
                            });
                        }
                    }
                }
                crate::events::GameplayEvent::EnemyDefeated { enemy, .. } => {
                    let ids: Vec<_> = world
                        .spatial_encounters()
                        .filter(|encounter| {
                            encounter.state == crate::encounter::SpatialEncounterState::Active
                                && encounter.spawned_enemy == Some(enemy)
                        })
                        .map(|encounter| encounter.id)
                        .collect();
                    for id in ids {
                        if let Some(updated) = world.spatial_encounter_mut(id) {
                            updated.state = crate::encounter::SpatialEncounterState::Resolved;
                            world.publish(crate::events::GameplayEvent::SpatialEncounterResolved {
                                encounter: id,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl Default for SpatialEncounterSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for SpatialEncounterSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::RegisterSpatialEncounter { encounter } => {
                if world.phase() != crate::types::SimulationPhase::Running {
                    return Some(CommandResult::rejected(
                        crate::commands::CommandRejection::SimulationNotRunning,
                    ));
                }
                Some(match world.register_spatial_encounter(encounter) {
                    Ok(()) => CommandResult::success(true),
                    Err(crate::encounter::SpatialEncounterError::DuplicateId) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::SpatialEncounterAlreadyRegistered,
                        )
                    }
                    Err(crate::encounter::SpatialEncounterError::DestinationNotFound)
                    | Err(crate::encounter::SpatialEncounterError::HazardNotFound)
                    | Err(crate::encounter::SpatialEncounterError::ItemDefinitionNotFound) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::SpatialEncounterNotFound,
                        )
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidSpatialEncounterConfiguration,
                    ),
                })
            }
            GameplayCommand::ActivateSpatialEncounter { encounter } => {
                Some(self.activate(world, encounter))
            }
            GameplayCommand::ResolveSpatialEncounter { player, encounter } => {
                Some(self.resolve(world, player, encounter))
            }
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

impl GameplaySystem for HullIntegritySystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::InitializeShipIntegrity {
                ship,
                maximum,
                initial,
                critical_threshold,
            } => self.initialize(world, ship, maximum, initial, critical_threshold),
            GameplayCommand::DamageShip { ship, amount } => self.damage(world, ship, amount),
            GameplayCommand::RepairShip { .. } => CommandResult::rejected(
                crate::commands::CommandRejection::RepairRequiresEngineeringStation,
            ),
            GameplayCommand::RepairShipAtStation {
                player,
                ship,
                station,
            } => self.repair_at_station(world, player, ship, station),
            _ => return None,
        };
        Some(result)
    }
}

impl HullIntegritySystem {
    fn validate_ship(
        &self,
        world: &GameplayWorld,
        ship: engine::EntityId,
    ) -> Result<(), CommandResult> {
        if world.navigation().ship_id() != Some(ship) || !world.entities().contains(ship) {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::ShipNotFound,
            ));
        }
        Ok(())
    }

    fn initialize(
        &self,
        world: &mut GameplayWorld,
        ship: engine::EntityId,
        maximum: u32,
        initial: u32,
        critical_threshold: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if let Err(result) = self.validate_ship(world, ship) {
            return result;
        }
        match world.initialize_ship_integrity(ship, maximum, initial, critical_threshold) {
            Ok(()) => CommandResult::success(true),
            Err(crate::integrity::IntegrityError::AlreadyInitialized) => CommandResult::rejected(
                crate::commands::CommandRejection::ShipIntegrityAlreadyInitialized,
            ),
            Err(crate::integrity::IntegrityError::ShipNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound)
            }
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipIntegrityConfiguration,
            ),
        }
    }

    fn damage(
        &self,
        world: &mut GameplayWorld,
        ship: engine::EntityId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if let Err(result) = self.validate_ship(world, ship) {
            return result;
        }
        match world.apply_ship_damage(ship, amount) {
            Ok(_) => CommandResult::success(true),
            Err(crate::integrity::IntegrityError::InvalidAmount) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipIntegrityConfiguration,
            ),
            Err(crate::integrity::IntegrityError::Destroyed) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed)
            }
            Err(_) => CommandResult::rejected(
                crate::commands::CommandRejection::ShipIntegrityNotInitialized,
            ),
        }
    }

    fn repair_at_station(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        ship: engine::EntityId,
        station: engine::EntityId,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        match world.repair_ship_at_station(player, ship, station) {
            Ok(_) => CommandResult::success(true),
            Err(crate::integrity::ShipRepairError::PlayerNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound)
            }
            Err(crate::integrity::ShipRepairError::PlayerInactive) => {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive)
            }
            Err(crate::integrity::ShipRepairError::ShipNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound)
            }
            Err(crate::integrity::ShipRepairError::ShipIntegrityNotInitialized) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::ShipIntegrityNotInitialized,
                )
            }
            Err(crate::integrity::ShipRepairError::ShipDestroyed) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed)
            }
            Err(crate::integrity::ShipRepairError::ShipAtMaximum) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipAtMaximumIntegrity)
            }
            Err(crate::integrity::ShipRepairError::StationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::RepairStationNotFound)
            }
            Err(crate::integrity::ShipRepairError::StationDisabled) => {
                CommandResult::rejected(crate::commands::CommandRejection::RepairStationDisabled)
            }
            Err(crate::integrity::ShipRepairError::WrongStationType) => {
                CommandResult::rejected(crate::commands::CommandRejection::RepairStationWrongType)
            }
            Err(crate::integrity::ShipRepairError::StationNotOccupiedByPlayer) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::RepairStationNotOccupiedByPlayer,
                )
            }
            Err(crate::integrity::ShipRepairError::InventoryNotInitialized) => {
                CommandResult::rejected(crate::commands::CommandRejection::InventoryNotInitialized)
            }
            Err(crate::integrity::ShipRepairError::RepairMaterialNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::RepairMaterialNotFound)
            }
            Err(crate::integrity::ShipRepairError::InsufficientMaterial) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryInsufficientQuantity,
                )
            }
            Err(crate::integrity::ShipRepairError::InvalidConfiguration) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidShipIntegrityConfiguration,
            ),
        }
    }
}

fn validate_pilot_access(
    world: &GameplayWorld,
    player_id: engine::EntityId,
    station_id: engine::EntityId,
) -> Result<(), CommandResult> {
    if world.phase() != crate::types::SimulationPhase::Running {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::SimulationNotRunning,
        ));
    }
    let Some(player) = world.player(player_id) else {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::PlayerNotFound,
        ));
    };
    if !world.entities().contains(player_id) || !player.is_active {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::PlayerInactive,
        ));
    }
    let Some(interactable) = world.interactable(station_id) else {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::PilotStationNotFound,
        ));
    };
    if !interactable.enabled {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::PilotStationDisabled,
        ));
    }
    let crate::interaction::InteractionBehavior::Station { station } = interactable.behavior else {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::IncompatibleInteraction,
        ));
    };
    if station.station_type != crate::station::StationType::Piloting {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::IncompatibleInteraction,
        ));
    }
    if station.occupant != Some(player_id) {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::PilotStationNotOccupiedByPlayer,
        ));
    }
    let Some(ship) = world.navigation().ship_id() else {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::ShipNotFound,
        ));
    };
    if !world.entities().contains(ship) {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::ShipNotFound,
        ));
    }
    if world.ship_is_destroyed(ship) == Some(true) {
        return Err(CommandResult::rejected(
            crate::commands::CommandRejection::ShipDestroyed,
        ));
    }
    Ok(())
}

impl NavigationSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn validate_pilot_access(
        &self,
        world: &GameplayWorld,
        player_id: engine::EntityId,
        station_id: engine::EntityId,
    ) -> Result<(), CommandResult> {
        validate_pilot_access(world, player_id, station_id)
    }

    fn select_destination(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
        destination: crate::navigation::DestinationId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        let result = world.navigation_mut().select_destination(destination);
        match result {
            Ok(()) => {
                world.publish(crate::events::GameplayEvent::DestinationSelected {
                    player,
                    pilot_station,
                    destination,
                });
                CommandResult::success(true)
            }
            Err(crate::navigation::NavigationSelectionError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::NavigationSelectionError::AutopilotActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotAlreadyPlanned)
            }
            Err(crate::navigation::NavigationSelectionError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotFound)
            }
            Err(crate::navigation::NavigationSelectionError::DestinationUnknown) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationUnknown)
            }
        }
    }

    fn confirm_free_navigation_destination(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
        destination: crate::navigation::DestinationId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        if !world
            .mission(crate::types::MissionId(1))
            .is_some_and(|mission| {
                mission.state() == crate::domains::mission::MissionState::Completed
            })
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::MissionOneIncomplete,
            );
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if world.ship_is_operational(ship) != Some(true) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed);
        }
        let now = world.time().elapsed;
        let result = world
            .navigation_mut()
            .confirm_free_navigation_destination(destination, now);
        match result {
            Ok(()) => {
                world.publish(
                    crate::events::GameplayEvent::FreeNavigationDestinationSelected {
                        player,
                        pilot_station,
                        destination,
                    },
                );
                CommandResult::success(true)
            }
            Err(crate::navigation::FreeNavigationSelectionError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::FreeNavigationSelectionError::AutopilotActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotAlreadyPlanned)
            }
            Err(crate::navigation::FreeNavigationSelectionError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotFound)
            }
            Err(crate::navigation::FreeNavigationSelectionError::DestinationNotPlanet) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotPlanet,
                )
            }
            Err(crate::navigation::FreeNavigationSelectionError::DestinationNotAllowed) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotAllowed,
                )
            }
            Err(crate::navigation::FreeNavigationSelectionError::DestinationUnknown) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationUnknown)
            }
            Err(crate::navigation::FreeNavigationSelectionError::DestinationIsCurrentPosition) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationIsCurrentPosition,
                )
            }
        }
    }

    fn evaluate_free_navigation(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        if !world
            .mission(crate::types::MissionId(1))
            .is_some_and(|mission| {
                mission.state() == crate::domains::mission::MissionState::Completed
            })
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::MissionOneIncomplete,
            );
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if world.ship_is_operational(ship) != Some(true) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed);
        }
        let Some(fuel) = world.ship_fuel(ship) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ResourcesNotInitialized,
            );
        };
        let now = world.time().elapsed;
        let proposal = match world
            .navigation()
            .evaluate_free_navigation(now, fuel.current())
        {
            Ok(proposal) => proposal,
            Err(crate::navigation::FreeNavigationEvaluationError::DestinationNotSelected) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotSelected,
                )
            }
            Err(crate::navigation::FreeNavigationEvaluationError::DestinationNotFound) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::DestinationNotFound,
                )
            }
            Err(crate::navigation::FreeNavigationEvaluationError::DestinationNotAllowed) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotAllowed,
                )
            }
            Err(crate::navigation::FreeNavigationEvaluationError::DestinationUnknown) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::DestinationUnknown,
                )
            }
            Err(_) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationProposalNotEvaluable,
                )
            }
        };
        world
            .navigation_mut()
            .set_free_navigation_proposal(proposal);
        CommandResult::success(true)
    }

    fn start_free_navigation(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        if !world
            .mission(crate::types::MissionId(1))
            .is_some_and(|mission| {
                mission.state() == crate::domains::mission::MissionState::Completed
            })
        {
            return CommandResult::rejected(
                crate::commands::CommandRejection::MissionOneIncomplete,
            );
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if world.ship_is_operational(ship) != Some(true) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed);
        }
        let now = world.time().elapsed;
        match world.start_free_navigation(now, player, pilot_station) {
            Ok((destination, started_at, duration, fuel_cost, remaining)) => {
                world.publish(crate::events::GameplayEvent::FuelConsumed {
                    ship,
                    destination,
                    amount: fuel_cost,
                    remaining,
                });
                world.publish(crate::events::GameplayEvent::TravelStarted {
                    player,
                    pilot_station,
                    destination,
                    started_at,
                    duration,
                    fuel_cost,
                });
                world.publish(crate::events::GameplayEvent::AutopilotStarted {
                    destination,
                    duration,
                    fuel_cost,
                });
                CommandResult::success(true)
            }
            Err(crate::navigation::FreeNavigationStartError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::FreeNavigationStartError::AutopilotActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotAlreadyPlanned)
            }
            Err(crate::navigation::FreeNavigationStartError::DestinationNotSelected) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotSelected,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::ProposalMissing) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationProposalNotEvaluable,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::ProposalDestinationMismatch) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationProposalStale,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::ProposalStale) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationProposalStale,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::ProposalNotViable) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationProposalNotViable,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotFound)
            }
            Err(crate::navigation::FreeNavigationStartError::DestinationNotAllowed) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::FreeNavigationDestinationNotAllowed,
                )
            }
            Err(crate::navigation::FreeNavigationStartError::DestinationUnknown) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationUnknown)
            }
            Err(crate::navigation::FreeNavigationStartError::InsufficientFuel) => {
                CommandResult::rejected(crate::commands::CommandRejection::InsufficientFuel)
            }
            Err(crate::navigation::FreeNavigationStartError::InvalidConfiguration) => {
                CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidNavigationConfiguration,
                )
            }
        }
    }

    fn start_travel(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        let now = world.time().elapsed;
        let result = world.start_navigation_travel(now);
        match result {
            Ok((destination, started_at, duration, fuel_cost, remaining)) => {
                world.publish(crate::events::GameplayEvent::FuelConsumed {
                    ship,
                    destination,
                    amount: fuel_cost,
                    remaining,
                });
                world.publish(crate::events::GameplayEvent::TravelStarted {
                    player,
                    pilot_station,
                    destination,
                    started_at,
                    duration,
                    fuel_cost,
                });
                CommandResult::success(true)
            }
            Err(crate::navigation::NavigationStartError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::NavigationStartError::AutopilotActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotAlreadyPlanned)
            }
            Err(crate::navigation::NavigationStartError::DestinationNotSelected) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotSelected)
            }
            Err(crate::navigation::NavigationStartError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotFound)
            }
            Err(crate::navigation::NavigationStartError::DestinationUnknown) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationUnknown)
            }
            Err(crate::navigation::NavigationStartError::InvalidDuration) => {
                CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidNavigationConfiguration,
                )
            }
            Err(crate::navigation::NavigationStartError::ResourcesNotInitialized) => {
                CommandResult::rejected(crate::commands::CommandRejection::ResourcesNotInitialized)
            }
            Err(crate::navigation::NavigationStartError::ShipNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound)
            }
            Err(crate::navigation::NavigationStartError::InsufficientFuel) => {
                CommandResult::rejected(crate::commands::CommandRejection::InsufficientFuel)
            }
        }
    }

    fn plan_autopilot(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
        origin: crate::navigation::DestinationId,
        destination: crate::navigation::DestinationId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        match world.plan_autopilot(origin, destination) {
            Ok(eta) => {
                world.publish(crate::events::GameplayEvent::AutopilotPlanned {
                    origin,
                    destination,
                    eta,
                });
                CommandResult::success(true)
            }
            Err(crate::navigation::AutopilotError::SameDestination) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotSameDestination)
            }
            Err(crate::navigation::AutopilotError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::NavigationRouteNotFound)
            }
            Err(crate::navigation::AutopilotError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::AutopilotError::AlreadyPlanned) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotAlreadyPlanned)
            }
            Err(crate::navigation::AutopilotError::NoRoute) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotNoRoute)
            }
            Err(crate::navigation::AutopilotError::OriginMismatch) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotOriginMismatch)
            }
            Err(_) => CommandResult::rejected(crate::commands::CommandRejection::AutopilotNoRoute),
        }
    }

    fn start_autopilot(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        let Some(ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        match world.start_autopilot(world.time().elapsed, player, pilot_station) {
            Ok((destination, started_at, duration, fuel_cost, remaining)) => {
                world.publish(crate::events::GameplayEvent::FuelConsumed {
                    ship,
                    destination,
                    amount: fuel_cost,
                    remaining,
                });
                world.publish(crate::events::GameplayEvent::TravelStarted {
                    player,
                    pilot_station,
                    destination,
                    started_at,
                    duration,
                    fuel_cost,
                });
                world.publish(crate::events::GameplayEvent::AutopilotStarted {
                    destination,
                    duration,
                    fuel_cost,
                });
                CommandResult::success(true)
            }
            Err(crate::navigation::AutopilotError::NotPlanned) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotNotPlanned)
            }
            Err(crate::navigation::AutopilotError::TravelActive) => {
                CommandResult::rejected(crate::commands::CommandRejection::TravelActive)
            }
            Err(crate::navigation::AutopilotError::InsufficientFuel) => {
                CommandResult::rejected(crate::commands::CommandRejection::InsufficientFuel)
            }
            Err(crate::navigation::AutopilotError::OriginMismatch) => {
                CommandResult::rejected(crate::commands::CommandRejection::AutopilotOriginMismatch)
            }
            Err(_) => CommandResult::rejected(crate::commands::CommandRejection::AutopilotNoRoute),
        }
    }

    fn continue_autopilot(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        if matches!(
            world.autopilot_state(),
            crate::navigation::AutopilotState::InTransit { .. }
        ) && matches!(
            world.navigation().travel_state(),
            crate::navigation::TravelState::InProgress { .. }
        ) {
            world
                .navigation_mut()
                .set_autopilot_continuation_authorized(true);
            CommandResult::success(true)
        } else {
            CommandResult::rejected(crate::commands::CommandRejection::AutopilotNotInTransit)
        }
    }

    fn cancel_autopilot(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        pilot_station: engine::EntityId,
    ) -> CommandResult {
        if let Err(result) = self.validate_pilot_access(world, player, pilot_station) {
            return result;
        }
        match world.navigation_mut().cancel_autopilot() {
            Some(destination) => {
                world.publish(crate::events::GameplayEvent::AutopilotInterrupted { destination });
                CommandResult::success(true)
            }
            None => CommandResult::rejected(crate::commands::CommandRejection::AutopilotNotPlanned),
        }
    }

    fn observe_time(&mut self, world: &mut GameplayWorld) {
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
            let crate::events::GameplayEvent::TimeAdvanced { to, .. } = event else {
                continue;
            };
            let crate::navigation::TravelState::InProgress {
                destination,
                started_at,
                duration,
                ..
            } = world.navigation().travel_state()
            else {
                continue;
            };
            let waiting_for_encounter_decision =
                !world.navigation().autopilot_continuation_authorized()
                    && world.spatial_encounters().any(|encounter| {
                        encounter.state == crate::encounter::SpatialEncounterState::Active
                            && matches!(
                                encounter.kind,
                                crate::encounter::SpatialEncounterKind::Hostile { .. }
                            )
                    });
            if waiting_for_encounter_decision {
                continue;
            }
            if let Some((planet, position, impact)) =
                world.navigation_mut().advance_travel_at(to.elapsed)
            {
                let Some(ship) = world.navigation().ship_id() else {
                    continue;
                };
                if impact {
                    process_navigation_impact(world, ship, planet, position);
                } else {
                    world.publish(crate::events::GameplayEvent::PlanetLanded {
                        ship,
                        planet,
                        position,
                    });
                    if let crate::navigation::AutopilotState::Completed { destination } =
                        world.autopilot_state()
                    {
                        world.publish(crate::events::GameplayEvent::AutopilotCompleted {
                            destination: *destination,
                        });
                    }
                }
                continue;
            }
            let deadline = started_at.0.saturating_add(duration.0);
            if to.elapsed.0 < deadline {
                continue;
            }
            let Some(destination_data) = world.destination(destination).cloned() else {
                continue;
            };
            let arrival_position = match world
                .navigation_mut()
                .complete_travel_at(destination, to.elapsed)
            {
                Ok(position) => position,
                Err((planet, position)) => {
                    let Some(ship) = world.navigation().ship_id() else {
                        continue;
                    };
                    process_navigation_impact(world, ship, planet, position);
                    continue;
                }
            };
            world.publish(crate::events::GameplayEvent::TravelArrived {
                destination,
                position: arrival_position,
            });
            if destination_data.destination_type == crate::navigation::DestinationType::Planet
                && world.navigation_mut().discover_destination(destination)
            {
                world.publish(crate::events::GameplayEvent::PlanetDiscovered { destination });
            }
            let was_autopilot = matches!(
                world.autopilot_state(),
                crate::navigation::AutopilotState::InTransit { .. }
            );
            if was_autopilot {
                match world.advance_autopilot_leg(to.elapsed) {
                    Ok(Some((player, pilot_station, next_destination, duration, fuel_cost, _))) => {
                        let Some(ship) = world.navigation().ship_id() else {
                            continue;
                        };
                        let remaining = world.ship_fuel(ship).map_or(0, |fuel| fuel.current());
                        world.publish(crate::events::GameplayEvent::FuelConsumed {
                            ship,
                            destination: next_destination,
                            amount: fuel_cost,
                            remaining,
                        });
                        world.publish(crate::events::GameplayEvent::TravelStarted {
                            player,
                            pilot_station,
                            destination: next_destination,
                            started_at: to.elapsed,
                            duration,
                            fuel_cost,
                        });
                        world.publish(crate::events::GameplayEvent::AutopilotStarted {
                            destination: next_destination,
                            duration,
                            fuel_cost,
                        });
                    }
                    Ok(None) => {
                        if let crate::navigation::AutopilotState::Completed { destination } =
                            world.autopilot_state()
                        {
                            world.publish(crate::events::GameplayEvent::AutopilotCompleted {
                                destination: *destination,
                            });
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}

impl Default for NavigationSystem {
    fn default() -> Self {
        Self::new()
    }
}

fn process_navigation_impact(
    world: &mut GameplayWorld,
    ship: engine::EntityId,
    planet: crate::navigation::DestinationId,
    position: engine::Vec3,
) {
    world.publish(crate::events::GameplayEvent::PlanetImpact {
        ship,
        planet,
        position,
    });
    if planet == crate::navigation::SUN_DESTINATION_ID {
        let _ = world.apply_ship_damage(ship, u32::MAX);
        world.defeat_active_players();
    }
}

impl GameplaySystem for NavigationSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::RegisterDestination { destination } => {
                match world.register_destination(destination.clone()) {
                    Ok(()) => {
                        world.publish(crate::events::GameplayEvent::DestinationRegistered {
                            destination: destination.id,
                        });
                        CommandResult::success(true)
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidNavigationConfiguration,
                    ),
                }
            }
            GameplayCommand::SelectDestination {
                player,
                pilot_station,
                destination,
            } => self.select_destination(world, player, pilot_station, destination),
            GameplayCommand::ConfirmFreeNavigationDestination {
                player,
                pilot_station,
                destination,
            } => {
                self.confirm_free_navigation_destination(world, player, pilot_station, destination)
            }
            GameplayCommand::EvaluateFreeNavigation {
                player,
                pilot_station,
            } => self.evaluate_free_navigation(world, player, pilot_station),
            GameplayCommand::StartFreeNavigation {
                player,
                pilot_station,
            } => self.start_free_navigation(world, player, pilot_station),
            GameplayCommand::StartTravel {
                player,
                pilot_station,
            } => self.start_travel(world, player, pilot_station),
            GameplayCommand::RegisterNavigationRoute { route } => {
                match world.register_navigation_route(route) {
                    Ok(()) => {
                        world.publish(crate::events::GameplayEvent::NavigationRouteRegistered {
                            from: route.from,
                            to: route.to,
                        });
                        CommandResult::success(true)
                    }
                    Err(crate::navigation::NavigationRouteError::DuplicateRoute) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::NavigationRouteAlreadyRegistered,
                        )
                    }
                    Err(crate::navigation::NavigationRouteError::DestinationNotFound) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::NavigationRouteNotFound,
                        )
                    }
                    Err(crate::navigation::NavigationRouteError::DestinationNotDiscovered) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::NavigationRouteNotDiscovered,
                        )
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidNavigationRouteConfiguration,
                    ),
                }
            }
            GameplayCommand::PlanAutopilot {
                player,
                pilot_station,
                origin,
                destination,
            } => self.plan_autopilot(world, player, pilot_station, origin, destination),
            GameplayCommand::StartAutopilot {
                player,
                pilot_station,
            } => self.start_autopilot(world, player, pilot_station),
            GameplayCommand::ContinueAutopilot {
                player,
                pilot_station,
            } => self.continue_autopilot(world, player, pilot_station),
            GameplayCommand::CancelAutopilot {
                player,
                pilot_station,
            } => self.cancel_autopilot(world, player, pilot_station),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn reset_event_cursor(&mut self) {
        self.event_cursor = 0;
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        self.observe_time(world);
    }
}

pub struct ResourceSystem;

impl GameplaySystem for ResourceSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::InitializeShipResources {
                ship,
                capacity,
                initial_fuel,
            } => {
                if world.phase() == crate::types::SimulationPhase::Running {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::ResourceInitializationClosed,
                    )
                } else if !world.entities().contains(ship) {
                    CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound)
                } else if world.ship_resources(ship).is_some() {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::ResourcesAlreadyInitialized,
                    )
                } else {
                    match world.initialize_ship_resources(ship, capacity, initial_fuel) {
                        Ok(()) => CommandResult::success(true),
                        Err(_) => CommandResult::invalid(
                            crate::commands::CommandInvalidity::InvalidResourceConfiguration,
                        ),
                    }
                }
            }
            GameplayCommand::AddShipFuel { ship, amount } => {
                let result = {
                    let Some(resources) = world.ship_resources_mut(ship) else {
                        return Some(CommandResult::rejected(
                            crate::commands::CommandRejection::ResourcesNotInitialized,
                        ));
                    };
                    resources
                        .fuel_mut()
                        .add(amount)
                        .map(|changed| (changed, resources.fuel().current()))
                };
                match result {
                    Ok((false, _)) => CommandResult::success(false),
                    Ok((true, fuel)) => {
                        world.publish(crate::events::GameplayEvent::FuelAdded {
                            ship,
                            amount,
                            fuel,
                        });
                        CommandResult::success(true)
                    }
                    Err(crate::resources::FuelError::FuelCapacityExceeded) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::FuelCapacityExceeded,
                        )
                    }
                    Err(crate::resources::FuelError::ArithmeticOverflow) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidResourceConfiguration,
                    ),
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidResourceConfiguration,
                    ),
                }
            }
            GameplayCommand::RemoveShipFuel { ship, amount } => {
                let result = {
                    let Some(resources) = world.ship_resources_mut(ship) else {
                        return Some(CommandResult::rejected(
                            crate::commands::CommandRejection::ResourcesNotInitialized,
                        ));
                    };
                    resources
                        .fuel_mut()
                        .consume(amount)
                        .map(|changed| (changed, resources.fuel().current()))
                };
                match result {
                    Ok((false, _)) => CommandResult::success(false),
                    Ok((true, fuel)) => {
                        world.publish(crate::events::GameplayEvent::FuelRemoved {
                            ship,
                            amount,
                            fuel,
                        });
                        CommandResult::success(true)
                    }
                    Err(crate::resources::FuelError::InsufficientFuel) => {
                        CommandResult::rejected(crate::commands::CommandRejection::InsufficientFuel)
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidResourceConfiguration,
                    ),
                }
            }
            GameplayCommand::UseFuelResource {
                player,
                ship,
                item,
                amount,
            } => {
                if amount == 0 || world.player(player).is_none_or(|value| !value.is_active) {
                    CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive)
                } else if world.item_definition(item).is_none_or(|definition| {
                    definition.category != crate::collectible::ItemCategory::Fuel
                }) {
                    CommandResult::rejected(
                        crate::commands::CommandRejection::ItemDefinitionNotFound,
                    )
                } else {
                    let Some(inventory) = world.inventory(player).cloned() else {
                        return Some(CommandResult::rejected(
                            crate::commands::CommandRejection::InventoryNotInitialized,
                        ));
                    };
                    let Some(mut resources) = world.ship_resources(ship) else {
                        return Some(CommandResult::rejected(
                            crate::commands::CommandRejection::ResourcesNotInitialized,
                        ));
                    };
                    if inventory.quantity(item) < amount {
                        CommandResult::rejected(crate::commands::CommandRejection::ResourceNotOwned)
                    } else if resources
                        .fuel()
                        .capacity()
                        .saturating_sub(resources.fuel().current())
                        < amount
                    {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::FuelCapacityExceeded,
                        )
                    } else if resources.fuel_mut().add(amount).is_err() {
                        CommandResult::invalid(
                            crate::commands::CommandInvalidity::InvalidResourceConfiguration,
                        )
                    } else {
                        let mut inventory_after = inventory;
                        if inventory_after.try_remove(item, amount).is_err() {
                            return Some(CommandResult::rejected(
                                crate::commands::CommandRejection::ResourceNotOwned,
                            ));
                        }
                        *world.inventory_mut(player).expect("inventory validated") =
                            inventory_after;
                        *world.ship_resources_mut(ship).expect("resources validated") = resources;
                        world.publish(crate::events::GameplayEvent::ItemRemoved {
                            player,
                            item,
                            amount,
                        });
                        world.publish(crate::events::GameplayEvent::FuelAdded {
                            ship,
                            amount,
                            fuel: resources.fuel().current(),
                        });
                        CommandResult::success(true)
                    }
                }
            }
            _ => return None,
        };
        Some(result)
    }
}

pub struct DoorSystem;

impl GameplaySystem for DoorSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::LockDoor { door } => {
                self.change_door(world, door, DoorOperation::Lock)
            }
            GameplayCommand::UnlockDoor { door } => {
                self.change_door(world, door, DoorOperation::Unlock)
            }
            _ => return None,
        };
        Some(result)
    }
}

#[derive(Clone, Copy)]
enum DoorOperation {
    Lock,
    Unlock,
}

impl DoorSystem {
    fn change_door(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        operation: DoorOperation,
    ) -> CommandResult {
        let Some(interactable) = world.interactable(entity).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        let crate::interaction::InteractionBehavior::Door { mut door } = interactable.behavior
        else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::IncompatibleInteraction,
            );
        };
        let result = match operation {
            DoorOperation::Lock => door.lock(),
            DoorOperation::Unlock => {
                if !world.access_requirement_satisfied(door.access_requirement) {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::DependenciesIncomplete,
                    );
                }
                door.unlock()
            }
        };
        if let Err(error) = result {
            return match error {
                crate::door::DoorTransitionError::Locked => {
                    CommandResult::rejected(crate::commands::CommandRejection::DoorLocked)
                }
                crate::door::DoorTransitionError::InvalidTransition => CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidDoorTransition,
                ),
            };
        }
        let state = door.state;
        let Some(stored) = world.interactable_mut(entity) else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        stored.behavior = crate::interaction::InteractionBehavior::Door { door };
        let event = match operation {
            DoorOperation::Lock => crate::events::GameplayEvent::DoorLocked {
                door: entity,
                state,
                role: door.role,
            },
            DoorOperation::Unlock => crate::events::GameplayEvent::DoorUnlocked {
                door: entity,
                state,
                role: door.role,
            },
        };
        world.publish(event);
        CommandResult::success(true)
    }
}

pub struct StationSystem;

impl GameplaySystem for StationSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::ReleaseStation { station, player } => {
                self.release(world, station, player)
            }
            GameplayCommand::SetStationEnabled { station, enabled } => {
                self.set_enabled(world, station, enabled)
            }
            _ => return None,
        };
        Some(result)
    }
}

impl StationSystem {
    fn release(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        player: engine::EntityId,
    ) -> CommandResult {
        let Some(interactable) = world.interactable(entity).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        let crate::interaction::InteractionBehavior::Station { mut station } =
            interactable.behavior
        else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::IncompatibleInteraction,
            );
        };
        if station.release(player).is_err() {
            return CommandResult::rejected(
                crate::commands::CommandRejection::StationNotOccupiedByPlayer,
            );
        }
        let state = station.state(interactable.enabled);
        let Some(stored) = world.interactable_mut(entity) else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        stored.behavior = crate::interaction::InteractionBehavior::Station { station };
        world.publish(crate::events::GameplayEvent::StationReleased {
            station: entity,
            player,
            state,
        });
        CommandResult::success(true)
    }

    fn set_enabled(
        &self,
        world: &mut GameplayWorld,
        entity: engine::EntityId,
        enabled: bool,
    ) -> CommandResult {
        let Some(interactable) = world.interactable(entity).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        };
        let crate::interaction::InteractionBehavior::Station { station } = interactable.behavior
        else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::IncompatibleInteraction,
            );
        };
        if interactable.enabled == enabled {
            return CommandResult::success(false);
        }
        if !world.set_interactable_enabled(entity, enabled) {
            return CommandResult::rejected(crate::commands::CommandRejection::TargetNotFound);
        }
        let state = station.state(enabled);
        let event = if enabled {
            crate::events::GameplayEvent::StationEnabled {
                station: entity,
                state,
            }
        } else {
            crate::events::GameplayEvent::StationDisabled {
                station: entity,
                state,
            }
        };
        world.publish(event);
        CommandResult::success(true)
    }
}

pub struct CombatSystem;

impl GameplaySystem for CombatSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::CreateEnemy { configuration } => {
                if world.phase() != crate::types::SimulationPhase::Running {
                    return Some(CommandResult::rejected(
                        crate::commands::CommandRejection::SimulationNotRunning,
                    ));
                }
                Some(match world.register_enemy(configuration) {
                    Ok(enemy) => {
                        world.publish(crate::events::GameplayEvent::EnemyCreated {
                            enemy,
                            archetype: configuration.archetype,
                            position: configuration.position,
                        });
                        CommandResult::success(true)
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidCombatConfiguration,
                    ),
                })
            }
            GameplayCommand::EquipPlayerWeapon { player, weapon } => {
                Some(self.equip_player_weapon(world, player, weapon))
            }
            GameplayCommand::EquipPlayerWeaponItem { player, item } => {
                Some(self.equip_player_weapon_item(world, player, item))
            }
            GameplayCommand::AttackEnemy { player, enemy } => {
                Some(self.attack(world, player, enemy, true))
            }
            GameplayCommand::EnemyAttackPlayer { enemy, player } => {
                Some(self.attack(world, enemy, player, false))
            }
            GameplayCommand::EnemyAttackShip { enemy, ship } => {
                Some(self.attack_ship(world, enemy, ship))
            }
            GameplayCommand::SetEnemyActive { enemy, active } => {
                Some(self.set_enemy_active(world, enemy, active))
            }
            GameplayCommand::SetShipShieldActive {
                player,
                ship,
                active,
            } => Some(self.set_ship_shield_active(world, player, ship, active)),
            _ => None,
        }
    }
}

impl CombatSystem {
    fn equip_player_weapon_item(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        item: crate::types::ItemId,
    ) -> CommandResult {
        let Some(definition) = world.item_definition(item) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            );
        };
        if definition.category != crate::collectible::ItemCategory::Weapon {
            return CommandResult::rejected(crate::commands::CommandRejection::WeaponUnavailable);
        }
        if world
            .inventory(player)
            .is_none_or(|inventory| inventory.quantity(item) == 0)
        {
            return CommandResult::rejected(crate::commands::CommandRejection::WeaponNotOwned);
        }
        let Some(weapon) = crate::combat::weapon_for_item(item) else {
            return CommandResult::rejected(crate::commands::CommandRejection::WeaponUnavailable);
        };
        self.equip_player_weapon(world, player, weapon)
    }

    fn set_ship_shield_active(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        ship: engine::EntityId,
        active: bool,
    ) -> CommandResult {
        if world.player(player).is_none_or(|value| !value.is_active) {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        if world.navigation().ship_id() != Some(ship) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        }
        if !world.navigation().control_active() {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipControlInactive);
        }
        CommandResult::success(world.set_ship_shield_active(ship, active))
    }

    fn validate_weapon_ammunition(
        &self,
        world: &GameplayWorld,
        weapon: crate::combat::WeaponConfiguration,
    ) -> Result<(), CommandResult> {
        let Some(ammunition) = weapon.ammunition else {
            return Ok(());
        };
        let Some(definition) = world.item_definition(ammunition.item) else {
            return Err(CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            ));
        };
        if definition.category != crate::collectible::ItemCategory::CombatAmmunition {
            return Err(CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidCombatConfiguration,
            ));
        }
        Ok(())
    }

    fn equip_player_weapon(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        weapon: crate::combat::WeaponConfiguration,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        if weapon.validate().is_err() {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidCombatConfiguration,
            );
        }
        if let Err(result) = self.validate_weapon_ammunition(world, weapon) {
            return result;
        }
        let Some(combatant) = world.combatant_mut(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !combatant.is_player || !combatant.active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if combatant.is_defeated() {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        if combatant.equipped_weapon == Some(weapon) {
            return CommandResult::success(false);
        }
        combatant.weapon = weapon;
        combatant.equipped_weapon = Some(weapon);
        world.publish(crate::events::GameplayEvent::WeaponStateChanged {
            owner: player,
            state: crate::domains::weapon::WeaponState::Ready,
        });
        CommandResult::success(true)
    }

    fn set_enemy_active(
        &self,
        world: &mut GameplayWorld,
        enemy: engine::EntityId,
        active: bool,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(combatant) = world.combatant_mut(enemy) else {
            return CommandResult::rejected(crate::commands::CommandRejection::EnemyNotFound);
        };
        if combatant.is_player {
            return CommandResult::rejected(crate::commands::CommandRejection::InvalidCombatTarget);
        }
        if combatant.is_defeated() && active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        if combatant.active == active {
            return CommandResult::success(false);
        }
        combatant.active = active;
        CommandResult::success(true)
    }

    fn attack(
        &self,
        world: &mut GameplayWorld,
        attacker: engine::EntityId,
        target: engine::EntityId,
        player_attack: bool,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(attacker_state) = world.combatant(attacker).copied() else {
            return if player_attack {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound)
            } else {
                CommandResult::rejected(crate::commands::CommandRejection::EnemyNotFound)
            };
        };
        let Some(target_state) = world.combatant(target).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::InvalidCombatTarget);
        };
        if attacker_state.is_player != player_attack || target_state.is_player == player_attack {
            return CommandResult::rejected(crate::commands::CommandRejection::InvalidCombatTarget);
        }
        let target_archetype = if target_state.is_player {
            None
        } else {
            let Some(archetype) = target_state.archetype else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidCombatTarget,
                );
            };
            Some(archetype)
        };
        if !attacker_state.active || !target_state.active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if (attacker_state.is_player
            && !world
                .player(attacker)
                .is_some_and(|player| world.entities().contains(attacker) && player.is_active))
            || (target_state.is_player
                && !world
                    .player(target)
                    .is_some_and(|player| world.entities().contains(target) && player.is_active))
        {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if attacker_state.is_defeated() || target_state.is_defeated() {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        let weapon = if player_attack {
            let Some(weapon) = attacker_state.equipped_weapon else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::WeaponUnavailable,
                );
            };
            weapon
        } else {
            attacker_state.weapon
        };
        if weapon.validate().is_err() {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidCombatConfiguration,
            );
        }
        if player_attack {
            if let Err(result) = self.validate_weapon_ammunition(world, weapon) {
                return result;
            }
        }
        let attacker_position = if attacker_state.is_player {
            if world.navigation().control_active() {
                // While the player is piloting, the combatant remains attached
                // to the interior pilot station. Ship weapons must nevertheless
                // use Echo-7's authoritative position for range validation.
                world.navigation().current_position()
            } else {
                let Some(player) = world.player(attacker) else {
                    return CommandResult::rejected(
                        crate::commands::CommandRejection::PlayerNotFound,
                    );
                };
                player.position
            }
        } else {
            attacker_state.position
        };
        let target_position = if target_state.is_player {
            let Some(player) = world.player(target) else {
                return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
            };
            player.position
        } else {
            target_state.position
        };
        let distance = Vec3Distance::between(attacker_position, target_position);
        if distance.is_none() || distance.unwrap_or(u32::MAX) > weapon.range {
            return CommandResult::rejected(crate::commands::CommandRejection::OutOfAttackRange);
        }
        let now = world.time().elapsed;
        if let Some(last) = attacker_state.last_attack_at {
            if now.0.saturating_sub(last.0) < weapon.cooldown.0 {
                return CommandResult::rejected(crate::commands::CommandRejection::AttackCooldown);
            }
        }

        let ammunition = if player_attack {
            weapon.ammunition
        } else {
            None
        };
        if let Some(ammunition) = ammunition {
            let Some(inventory) = world.inventory_mut(attacker) else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryNotInitialized,
                );
            };
            if inventory.quantity(ammunition.item) < ammunition.amount_per_attack {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::AmmunitionUnavailable,
                );
            }
            if inventory
                .try_remove(ammunition.item, ammunition.amount_per_attack)
                .is_err()
            {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::AmmunitionUnavailable,
                );
            }
        }

        let attack_type = weapon.attack_type;
        let base_damage = if player_attack {
            match world.player_attack_damage(weapon.damage) {
                Some(value) => value,
                None => {
                    return CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidCombatConfiguration,
                    )
                }
            }
        } else {
            weapon.damage
        };
        let mut health_damage = base_damage;
        let mut shield_event = None;
        let mut shield_disabled = false;
        let remaining_health;
        {
            let Some(target_mut) = world.combatant_mut(target) else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidCombatTarget,
                );
            };
            if let Some(mut shield) = target_mut.shield {
                if shield.is_active() && attack_type != shield.weakness {
                    let absorbed = base_damage.min(shield.current);
                    shield.current -= absorbed;
                    health_damage = 0;
                    shield_event = Some((absorbed, shield.current));
                    target_mut.shield = Some(shield);
                } else if shield.is_active() && attack_type == shield.weakness {
                    shield.current = 0;
                    shield_disabled = true;
                    target_mut.shield = Some(shield);
                }
            }
            target_mut.current_health = target_mut.current_health.saturating_sub(health_damage);
            remaining_health = target_mut.current_health;
        }
        if let Some((amount, remaining)) = shield_event {
            world.publish(crate::events::GameplayEvent::ShieldDamaged {
                enemy: target,
                amount,
                remaining,
            });
        }
        if shield_disabled {
            world.publish(crate::events::GameplayEvent::ShieldDisabled { enemy: target });
        }
        {
            let Some(attacker_mut) = world.combatant_mut(attacker) else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidCombatTarget,
                );
            };
            attacker_mut.last_attack_at = Some(now);
        }
        if let Some(ammunition) = ammunition {
            world.publish(crate::events::GameplayEvent::ItemRemoved {
                player: attacker,
                item: ammunition.item,
                amount: ammunition.amount_per_attack,
            });
            world.publish(crate::events::GameplayEvent::AmmunitionConsumed {
                player: attacker,
                item: ammunition.item,
                amount: ammunition.amount_per_attack,
                attack_type,
                at: now,
            });
        }
        world.publish(crate::events::GameplayEvent::AttackExecuted {
            attacker,
            target,
            attack_type,
            damage: health_damage,
        });
        world.publish(crate::events::GameplayEvent::CombatDamageApplied {
            attacker,
            target,
            amount: health_damage,
            remaining_health,
        });
        if remaining_health == 0 {
            if let Some(target_mut) = world.combatant_mut(target) {
                target_mut.active = false;
            }
            if target_state.is_player {
                world.deactivate_entity(target);
                world.publish(crate::events::GameplayEvent::PlayerDefeated { player: target });
            } else {
                world.publish(crate::events::GameplayEvent::EnemyDefeated {
                    enemy: target,
                    archetype: target_archetype.unwrap_or(crate::combat::EnemyArchetype::Shooter),
                });
            }
        }
        CommandResult::success(true)
    }

    pub(crate) fn autonomous_attack(
        &self,
        world: &mut GameplayWorld,
        enemy: engine::EntityId,
        player: engine::EntityId,
    ) -> CommandResult {
        self.attack(world, enemy, player, false)
    }

    fn attack_ship(
        &self,
        world: &mut GameplayWorld,
        enemy: engine::EntityId,
        ship: engine::EntityId,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(attacker) = world.combatant(enemy).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::EnemyNotFound);
        };
        if attacker.is_player {
            return CommandResult::rejected(crate::commands::CommandRejection::InvalidCombatTarget);
        }
        if !attacker.active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if attacker.is_defeated() {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        match attacker.archetype {
            Some(crate::combat::EnemyArchetype::Shielded)
            | Some(crate::combat::EnemyArchetype::Shooter)
            | Some(crate::combat::EnemyArchetype::Charger) => {}
            None => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InvalidCombatTarget,
                )
            }
        }
        let Some(navigation_ship) = world.navigation().ship_id() else {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        };
        if navigation_ship != ship || !world.entities().contains(ship) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipNotFound);
        }
        if world.ship_is_destroyed(ship) == Some(true) {
            return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed);
        }
        let Some(distance) =
            Vec3Distance::between(attacker.position, world.navigation().current_position())
        else {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidCombatConfiguration,
            );
        };
        if distance > attacker.weapon.range {
            return CommandResult::rejected(crate::commands::CommandRejection::OutOfAttackRange);
        }
        let now = world.time().elapsed;
        if let Some(last) = attacker.last_attack_at {
            if now.0.saturating_sub(last.0) < attacker.weapon.cooldown.0 {
                return CommandResult::rejected(crate::commands::CommandRejection::AttackCooldown);
            }
        }
        if attacker.weapon.validate().is_err() {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidCombatConfiguration,
            );
        }
        let amount = attacker.weapon.damage;
        let integrity_before = world.ship_integrity(ship).map(|value| value.current());
        let remaining = match world.apply_ship_damage(ship, amount) {
            Ok((remaining, _, _)) => remaining,
            Err(crate::integrity::IntegrityError::Destroyed) => {
                return CommandResult::rejected(crate::commands::CommandRejection::ShipDestroyed)
            }
            Err(crate::integrity::IntegrityError::InvalidAmount) => {
                return CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidCombatConfiguration,
                )
            }
            Err(_) => {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::ShipIntegrityNotInitialized,
                )
            }
        };
        if let Some(attacker_mut) = world.combatant_mut(enemy) {
            attacker_mut.last_attack_at = Some(now);
        }
        world.publish(crate::events::GameplayEvent::AttackExecuted {
            attacker: enemy,
            target: ship,
            attack_type: attacker.weapon.attack_type,
            damage: amount,
        });
        world.publish(crate::events::GameplayEvent::ShipCombatDamageApplied {
            attacker: enemy,
            ship,
            amount: integrity_before.map_or(amount, |before| before.saturating_sub(remaining)),
            remaining_integrity: remaining,
        });
        CommandResult::success(true)
    }
}

pub struct EnemyAiSystem {
    event_cursor: usize,
    anchors: BTreeMap<engine::EntityId, engine::Vec3>,
    targets: BTreeMap<engine::EntityId, engine::EntityId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnemyAiState {
    Patrol,
    Pursuit(engine::EntityId),
}

impl EnemyAiSystem {
    pub const DETECTION_RADIUS: f64 = 30.0;
    pub const LOSS_RADIUS: f64 = 45.0;

    pub const fn new() -> Self {
        Self {
            event_cursor: 0,
            anchors: BTreeMap::new(),
            targets: BTreeMap::new(),
        }
    }

    fn observe_time(&mut self, world: &mut GameplayWorld, delta: engine::Duration) {
        if delta.0 == 0 {
            return;
        }
        let enemies: Vec<_> = world.enemies().copied().collect();
        for enemy in enemies {
            if !enemy.active || enemy.is_defeated() {
                continue;
            }
            let anchor = *self.anchors.entry(enemy.entity).or_insert(enemy.position);
            let state = self.state_for_enemy(world, enemy.entity, enemy.position);
            let EnemyAiState::Pursuit(target) = state else {
                Self::move_on_patrol(world, enemy, anchor, world.time().elapsed.0, delta);
                continue;
            };
            let Some(target_position) = world
                .player(target)
                .filter(|player| player.is_active)
                .map(|player| player.position)
            else {
                self.targets.remove(&enemy.entity);
                Self::move_on_patrol(world, enemy, anchor, world.time().elapsed.0, delta);
                continue;
            };
            let Some(distance_squared) = safe_distance_squared(enemy.position, target_position)
            else {
                continue;
            };
            let range_squared = f64::from(enemy.weapon.range) * f64::from(enemy.weapon.range);
            if distance_squared <= range_squared {
                let _ = CombatSystem.autonomous_attack(world, enemy.entity, target);
                match enemy
                    .archetype
                    .unwrap_or(crate::combat::EnemyArchetype::Shooter)
                {
                    crate::combat::EnemyArchetype::Shooter => Self::move_on_patrol(
                        world,
                        enemy,
                        enemy.position,
                        world.time().elapsed.0,
                        delta,
                    ),
                    crate::combat::EnemyArchetype::Shielded => Self::move_towards_scaled(
                        world,
                        enemy,
                        target_position,
                        delta,
                        distance_squared,
                        0.35,
                    ),
                    crate::combat::EnemyArchetype::Charger => Self::move_towards_scaled(
                        world,
                        enemy,
                        target_position,
                        delta,
                        distance_squared,
                        0.9,
                    ),
                }
            } else {
                Self::move_for_archetype(world, enemy, target_position, delta, distance_squared);
            }
        }
        self.anchors.retain(|entity, _| {
            world
                .combatant(*entity)
                .is_some_and(|enemy| !enemy.is_player && enemy.active && !enemy.is_defeated())
        });
        self.targets.retain(|entity, target| {
            world
                .combatant(*entity)
                .is_some_and(|enemy| !enemy.is_player && enemy.active && !enemy.is_defeated())
                && world.player(*target).is_some_and(|player| player.is_active)
        });
    }

    fn state_for_enemy(
        &mut self,
        world: &GameplayWorld,
        enemy: engine::EntityId,
        enemy_position: engine::Vec3,
    ) -> EnemyAiState {
        if let Some(target) = self.targets.get(&enemy).copied() {
            let valid = world
                .player(target)
                .filter(|player| player.is_active)
                .and_then(|player| safe_distance_squared(enemy_position, player.position))
                .is_some_and(|distance| distance <= Self::LOSS_RADIUS * Self::LOSS_RADIUS);
            if valid {
                return EnemyAiState::Pursuit(target);
            }
            self.targets.remove(&enemy);
        }
        if let Some((target, _, _)) =
            Self::select_target(world, enemy_position, Self::DETECTION_RADIUS)
        {
            self.targets.insert(enemy, target);
            EnemyAiState::Pursuit(target)
        } else {
            EnemyAiState::Patrol
        }
    }

    fn select_target(
        world: &GameplayWorld,
        enemy_position: engine::Vec3,
        detection_radius: f64,
    ) -> Option<(engine::EntityId, engine::Vec3, f64)> {
        world
            .players()
            .filter_map(|player| {
                let combatant = world.combatant(player.entity_id)?;
                if !player.is_active
                    || !world.entities().contains(player.entity_id)
                    || !combatant.active
                    || combatant.is_defeated()
                    || !finite_position(player.position)
                {
                    return None;
                }
                let distance_squared = safe_distance_squared(enemy_position, player.position)?;
                if distance_squared > detection_radius * detection_radius {
                    return None;
                }
                Some((player.entity_id, player.position, distance_squared))
            })
            .min_by(|left, right| {
                left.2
                    .total_cmp(&right.2)
                    .then_with(|| left.0.cmp(&right.0))
            })
    }

    fn move_towards(
        world: &mut GameplayWorld,
        enemy: crate::combat::Combatant,
        target: engine::Vec3,
        delta: engine::Duration,
        distance_squared: f64,
    ) {
        Self::move_towards_scaled(world, enemy, target, delta, distance_squared, 1.0);
    }

    fn move_towards_scaled(
        world: &mut GameplayWorld,
        enemy: crate::combat::Combatant,
        target: engine::Vec3,
        delta: engine::Duration,
        distance_squared: f64,
        speed_scale: f64,
    ) {
        if !finite_position(enemy.position) || !finite_position(target) {
            return;
        }
        let distance = distance_squared.sqrt();
        if !distance.is_finite() || distance == 0.0 {
            return;
        }
        let seconds = delta.0 as f64 / 1_000.0;
        let travel = f64::from(enemy.mobility) * seconds * speed_scale;
        if !travel.is_finite() || travel <= 0.0 {
            return;
        }
        let factor = (travel / distance).min(1.0);
        let delta_x = f64::from(target.x) - f64::from(enemy.position.x);
        let delta_y = f64::from(target.y) - f64::from(enemy.position.y);
        let delta_z = f64::from(target.z) - f64::from(enemy.position.z);
        let position = Self::bounded_position(engine::Vec3 {
            x: (f64::from(enemy.position.x) + delta_x * factor) as f32,
            y: (f64::from(enemy.position.y) + delta_y * factor) as f32,
            z: (f64::from(enemy.position.z) + delta_z * factor) as f32,
        });
        Self::apply_position(world, enemy, position);
    }

    fn move_for_archetype(
        world: &mut GameplayWorld,
        enemy: crate::combat::Combatant,
        target: engine::Vec3,
        delta: engine::Duration,
        distance_squared: f64,
    ) {
        let distance = distance_squared.sqrt();
        match enemy
            .archetype
            .unwrap_or(crate::combat::EnemyArchetype::Shooter)
        {
            crate::combat::EnemyArchetype::Shooter => {
                let preferred = f64::from(enemy.weapon.range) * 0.72;
                if distance > preferred + 2.0 {
                    Self::move_towards_scaled(world, enemy, target, delta, distance_squared, 0.85);
                } else if distance < preferred - 2.0 {
                    let fallback = if distance < 0.001 { 1.0 } else { 0.0 };
                    let retreat = engine::Vec3 {
                        x: enemy.position.x + (enemy.position.x - target.x) + fallback,
                        y: enemy.position.y + (enemy.position.y - target.y) * 0.25,
                        z: enemy.position.z + (enemy.position.z - target.z),
                    };
                    let retreat_distance =
                        safe_distance_squared(enemy.position, retreat).unwrap_or(distance_squared);
                    Self::move_towards_scaled(world, enemy, retreat, delta, retreat_distance, 0.75);
                } else {
                    let clockwise = if enemy.entity.value() % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let tangent = engine::Vec3 {
                        x: enemy.position.x + (target.z - enemy.position.z) * clockwise,
                        y: enemy.position.y,
                        z: enemy.position.z - (target.x - enemy.position.x) * clockwise,
                    };
                    let tangent_distance =
                        safe_distance_squared(enemy.position, tangent).unwrap_or(distance_squared);
                    Self::move_towards_scaled(world, enemy, tangent, delta, tangent_distance, 0.65);
                }
            }
            crate::combat::EnemyArchetype::Shielded => {
                Self::move_towards_scaled(world, enemy, target, delta, distance_squared, 0.55);
            }
            crate::combat::EnemyArchetype::Charger => {
                Self::move_towards_scaled(world, enemy, target, delta, distance_squared, 1.65);
            }
        }
    }

    fn move_on_patrol(
        world: &mut GameplayWorld,
        enemy: crate::combat::Combatant,
        anchor: engine::Vec3,
        elapsed_ms: u64,
        delta: engine::Duration,
    ) {
        let elapsed = elapsed_ms as f64 / 1_000.0;
        let phase = f64::from((enemy.entity.value() % 31) as u32) * 0.17;
        let angular_speed = 0.45 + f64::from(enemy.mobility) * 0.01;
        // Keep the close-range patrol deliberately tight so it remains an
        // observable autonomous motion without changing the existing planar
        // engagement and reward positions.
        let radius = (f64::from(enemy.mobility) * 0.01).clamp(0.05, 0.1);
        let angle = phase + elapsed * angular_speed;
        let (x, y, z) = match enemy
            .archetype
            .unwrap_or(crate::combat::EnemyArchetype::Shooter)
        {
            crate::combat::EnemyArchetype::Shooter => {
                (anchor.x, anchor.y + (angle.sin() * radius) as f32, anchor.z)
            }
            crate::combat::EnemyArchetype::Shielded => (
                anchor.x + (angle.cos() * radius) as f32,
                anchor.y,
                anchor.z + (angle.sin() * radius) as f32,
            ),
            crate::combat::EnemyArchetype::Charger => (
                anchor.x + (angle.sin() * radius * 2.0) as f32,
                anchor.y,
                anchor.z,
            ),
        };
        let target = engine::Vec3 { x, y, z };
        let Some(distance_squared) = safe_distance_squared(enemy.position, target) else {
            return;
        };
        Self::move_towards(world, enemy, target, delta, distance_squared);
    }

    fn bounded_position(position: engine::Vec3) -> engine::Vec3 {
        engine::Vec3 {
            x: position.x.clamp(-100.0, 100.0),
            y: position.y.clamp(-50.0, 50.0),
            z: position.z.clamp(-100.0, 100.0),
        }
    }

    fn apply_position(
        world: &mut GameplayWorld,
        enemy: crate::combat::Combatant,
        position: engine::Vec3,
    ) {
        if !finite_position(position) {
            return;
        }
        if let Some(updated) = world.combatant_mut(enemy.entity) {
            updated.position = position;
        }
        world.publish(crate::events::GameplayEvent::EnemyMoved {
            enemy: enemy.entity,
            position,
        });
    }
}

impl Default for EnemyAiSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for EnemyAiSystem {
    fn process(
        &mut self,
        _world: &mut GameplayWorld,
        _command: GameplayCommand,
    ) -> Option<CommandResult> {
        None
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn reset_event_cursor(&mut self) {
        self.event_cursor = 0;
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
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
            if let crate::events::GameplayEvent::TimeAdvanced { from, to } = event {
                self.observe_time(
                    world,
                    engine::Duration(to.elapsed.0.saturating_sub(from.elapsed.0)),
                );
            }
        }
    }
}

struct Vec3Distance;

impl Vec3Distance {
    fn between(a: engine::Vec3, b: engine::Vec3) -> Option<u32> {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        if !dx.is_finite() || !dy.is_finite() || !dz.is_finite() {
            return None;
        }
        let squared = dx * dx + dy * dy + dz * dz;
        if !squared.is_finite() {
            return None;
        }
        Some(squared.sqrt().ceil() as u32)
    }
}

fn safe_distance_squared(a: engine::Vec3, b: engine::Vec3) -> Option<f64> {
    let dx = f64::from(a.x) - f64::from(b.x);
    let dy = f64::from(a.y) - f64::from(b.y);
    let dz = f64::from(a.z) - f64::from(b.z);
    let squared = dx.mul_add(dx, dy.mul_add(dy, dz * dz));
    squared.is_finite().then_some(squared)
}

#[cfg(test)]
mod enemy_ai_tests {
    use super::*;
    use crate::combat::{EnemyArchetype, EnemyConfiguration};
    use crate::pipeline::GameplayProcessor;

    fn session_with_archetype(
        archetype: EnemyArchetype,
        position: engine::Vec3,
    ) -> (
        GameplayWorld,
        GameplayProcessor,
        engine::EntityId,
        engine::EntityId,
    ) {
        let mut world = GameplayWorld::new();
        world.start();
        let player = world.create_player().expect("player should be created");
        let enemy = world
            .register_enemy(EnemyConfiguration::for_archetype(archetype, position))
            .expect("enemy should be created");
        let mut processor = GameplayProcessor::new();
        processor.add_system(PlayerSystem);
        processor.add_system(CombatSystem);
        processor.add_system(EnemyAiSystem::new());
        assert!(world.player(player).is_some());
        (world, processor, enemy, player)
    }

    fn session_with_enemy(
        position: engine::Vec3,
    ) -> (
        GameplayWorld,
        GameplayProcessor,
        engine::EntityId,
        engine::EntityId,
    ) {
        session_with_archetype(EnemyArchetype::Shooter, position)
    }

    #[test]
    fn enemy_moves_autonomously_even_when_player_is_in_weapon_range() {
        let (mut world, mut processor, enemy, _) = session_with_enemy(engine::Vec3::ZERO);
        let before = world.combatant(enemy).expect("enemy should exist").position;

        processor.update(
            &mut world,
            [GameplayCommand::AdvanceTime {
                delta: engine::Duration(500),
            }],
        );

        let after = world.combatant(enemy).expect("enemy should exist").position;
        assert_ne!(before, after);
        assert!(world.events().iter().any(|event| {
            matches!(event, crate::events::GameplayEvent::EnemyMoved { enemy: id, .. } if *id == enemy)
        }));
    }

    #[test]
    fn enemy_patrol_is_deterministic_for_equal_initial_state_and_time() {
        let (mut first, mut first_processor, first_enemy, _) = session_with_enemy(engine::Vec3 {
            x: 4.0,
            y: 0.0,
            z: -3.0,
        });
        let (mut second, mut second_processor, second_enemy, _) =
            session_with_enemy(engine::Vec3 {
                x: 4.0,
                y: 0.0,
                z: -3.0,
            });
        let delta = engine::Duration(750);

        first_processor.update(&mut first, [GameplayCommand::AdvanceTime { delta }]);
        second_processor.update(&mut second, [GameplayCommand::AdvanceTime { delta }]);

        assert_eq!(
            first
                .combatant(first_enemy)
                .expect("first enemy should exist")
                .position,
            second
                .combatant(second_enemy)
                .expect("second enemy should exist")
                .position
        );
    }

    #[test]
    fn enemy_navigation_is_kept_inside_world_limits() {
        let (mut world, mut processor, enemy, _) = session_with_enemy(engine::Vec3 {
            x: 150.0,
            y: 70.0,
            z: -150.0,
        });

        processor.update(
            &mut world,
            [GameplayCommand::AdvanceTime {
                delta: engine::Duration(1_000),
            }],
        );

        let position = world.combatant(enemy).expect("enemy should exist").position;
        assert!(position.x.abs() <= 100.0);
        assert!(position.y.abs() <= 50.0);
        assert!(position.z.abs() <= 100.0);
    }

    #[test]
    fn enemy_detection_pursuit_and_loss_use_hysteresis() {
        assert!(EnemyAiSystem::LOSS_RADIUS > EnemyAiSystem::DETECTION_RADIUS);
        let (mut world, _, enemy, player) = session_with_enemy(engine::Vec3 {
            x: 40.0,
            y: 0.0,
            z: 0.0,
        });
        let mut ai = EnemyAiSystem::new();

        assert_eq!(
            ai.state_for_enemy(&world, enemy, world.combatant(enemy).unwrap().position),
            EnemyAiState::Patrol
        );
        world.player_mut(player).unwrap().position = engine::Vec3 {
            x: 20.0,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(
            ai.state_for_enemy(&world, enemy, world.combatant(enemy).unwrap().position),
            EnemyAiState::Pursuit(player)
        );
        world.player_mut(player).unwrap().position = engine::Vec3 {
            x: 35.0,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(
            ai.state_for_enemy(&world, enemy, world.combatant(enemy).unwrap().position),
            EnemyAiState::Pursuit(player)
        );
        world.player_mut(player).unwrap().position = engine::Vec3 {
            x: 100.0,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(
            ai.state_for_enemy(&world, enemy, world.combatant(enemy).unwrap().position),
            EnemyAiState::Patrol
        );
        assert!(!ai.targets.contains_key(&enemy));
    }

    #[test]
    fn enemy_does_not_target_inactive_or_defeated_players() {
        let (mut world, _, enemy, player) = session_with_enemy(engine::Vec3::ZERO);
        let mut ai = EnemyAiSystem::new();
        world.player_mut(player).unwrap().is_active = false;
        assert_eq!(
            ai.state_for_enemy(&world, enemy, world.combatant(enemy).unwrap().position),
            EnemyAiState::Patrol
        );
    }

    #[test]
    fn archetypes_use_distinct_balanced_engagement_patterns() {
        let start = engine::Vec3 {
            x: 15.0,
            y: 0.0,
            z: 0.0,
        };
        let advance = |archetype| {
            let (mut world, mut processor, enemy, _) = session_with_archetype(archetype, start);
            processor.update(
                &mut world,
                [GameplayCommand::AdvanceTime {
                    delta: engine::Duration(500),
                }],
            );
            world.combatant(enemy).unwrap().position
        };

        let shooter = advance(EnemyArchetype::Shooter);
        let shielded = advance(EnemyArchetype::Shielded);
        let charger = advance(EnemyArchetype::Charger);
        assert!(
            shooter.x >= start.x,
            "shooter should preserve standoff distance"
        );
        assert!(
            shielded.x < start.x,
            "shielded ship should advance steadily"
        );
        assert!(
            charger.x < shielded.x,
            "charger should close distance fastest"
        );
    }
}

pub struct InventorySystem;

#[cfg(test)]
mod resource_tests {
    use super::*;
    use crate::{
        collectible::{ItemCategory, ItemDefinition},
        life_support::{CompartmentId, LifeSupportConfiguration, OxygenCompartmentConfiguration},
        types::ItemId,
    };

    #[test]
    fn fuel_resource_is_consumed_atomically_into_the_real_tank() {
        let mut world = GameplayWorld::new();
        world.start();
        let player = world.create_player().expect("player should exist");
        let ship = world.create_entity();
        world
            .initialize_navigation_ship(ship)
            .expect("ship should exist");
        world
            .initialize_ship_resources(ship, 10, 0)
            .expect("fuel tank should exist");
        world
            .initialize_inventory(player, 10)
            .expect("inventory should exist");
        world
            .register_item_definition(ItemDefinition::new(ItemId(92), ItemCategory::Fuel, 10))
            .unwrap();
        let mut processor = crate::pipeline::GameplayProcessor::new();
        processor.add_system(InventorySystem);
        processor.add_system(ResourceSystem);
        assert!(matches!(
            processor
                .update(
                    &mut world,
                    [GameplayCommand::AddInventoryItem {
                        player,
                        item: ItemId(92),
                        amount: 2
                    }]
                )
                .first()
                .map(|value| value.status),
            Some(crate::commands::CommandStatus::Success { .. })
        ));
        let result = processor.update(
            &mut world,
            [GameplayCommand::UseFuelResource {
                player,
                ship,
                item: ItemId(92),
                amount: 1,
            }],
        );
        assert!(matches!(
            result.first().map(|value| value.status),
            Some(crate::commands::CommandStatus::Success { changed: true })
        ));
        assert_eq!(world.ship_fuel(ship).unwrap().current(), 1);
        assert_eq!(world.inventory(player).unwrap().quantity(ItemId(92)), 1);
        let rejected = processor.update(
            &mut world,
            [GameplayCommand::UseFuelResource {
                player,
                ship,
                item: ItemId(92),
                amount: 10,
            }],
        );
        assert!(matches!(
            rejected.first().map(|value| value.status),
            Some(crate::commands::CommandStatus::Rejected {
                reason: crate::commands::CommandRejection::ResourceNotOwned
            })
        ));
        assert_eq!(world.ship_fuel(ship).unwrap().current(), 1);
        assert_eq!(world.inventory(player).unwrap().quantity(ItemId(92)), 1);
    }

    #[test]
    fn oxygen_resource_replenishes_the_existing_life_support_allocation() {
        let mut world = GameplayWorld::new();
        world.start();
        let player = world.create_player().expect("player should exist");
        world
            .initialize_inventory(player, 10)
            .expect("inventory should exist");
        world
            .register_item_definition(ItemDefinition::new(ItemId(93), ItemCategory::Oxygen, 10))
            .unwrap();
        let panel = world
            .create_life_support_panel(
                engine::Vec3::ZERO,
                3.0,
                LifeSupportConfiguration::new(
                    4,
                    vec![OxygenCompartmentConfiguration {
                        id: CompartmentId::new(1),
                        initial_allocation: 0,
                        minimum_allocation: 0,
                        maximum_allocation: 4,
                    }],
                ),
            )
            .unwrap();
        world.player_mut(player).unwrap().position = engine::Vec3::ZERO;
        let mut processor = crate::pipeline::GameplayProcessor::new();
        processor.add_system(PlayerSystem);
        processor.add_system(InteractionSystem);
        processor.add_system(InventorySystem);
        processor.add_system(LifeSupportSystem::new());
        processor.update(
            &mut world,
            [GameplayCommand::AddInventoryItem {
                player,
                item: ItemId(93),
                amount: 2,
            }],
        );
        processor.update(
            &mut world,
            [GameplayCommand::Interact {
                player,
                target: panel,
            }],
        );
        let result = processor.update(
            &mut world,
            [GameplayCommand::UseOxygenResource {
                player,
                panel,
                compartment: CompartmentId::new(1),
                item: ItemId(93),
                amount: 1,
            }],
        );
        assert!(matches!(
            result.first().map(|value| value.status),
            Some(crate::commands::CommandStatus::Success { changed: true })
        ));
        assert_eq!(
            world.life_support_panel(panel).unwrap().allocated_total(),
            1
        );
        assert_eq!(world.inventory(player).unwrap().quantity(ItemId(93)), 1);
    }
}

impl GameplaySystem for InventorySystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        match command {
            GameplayCommand::InitializePlayerInventory { player, capacity } => {
                Some(self.initialize(world, player, capacity))
            }
            GameplayCommand::RegisterItemDefinition { definition } => {
                Some(match world.register_item_definition(definition) {
                    Ok(()) => CommandResult::success(true),
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidItemDefinition,
                    ),
                })
            }
            GameplayCommand::AddInventoryItem {
                player,
                item,
                amount,
            } => Some(self.add(world, player, item, amount)),
            GameplayCommand::TransferInventoryItem {
                from,
                to,
                item,
                amount,
            } => Some(self.transfer(world, from, to, item, amount)),
            GameplayCommand::RemoveInventoryItem {
                player,
                item,
                amount,
            } => Some(self.remove(world, player, item, amount)),
            GameplayCommand::CreateCollectible {
                item,
                amount,
                position,
                interaction_range,
            } => Some(
                match world.create_collectible(item, amount, position, interaction_range) {
                    Ok(collectible) => {
                        world.publish(crate::events::GameplayEvent::CollectibleCreated {
                            collectible,
                            item,
                            amount,
                            position,
                        });
                        CommandResult::success(true)
                    }
                    Err(crate::collectible::CollectibleError::ItemNotFound) => {
                        CommandResult::rejected(
                            crate::commands::CommandRejection::ItemDefinitionNotFound,
                        )
                    }
                    Err(_) => CommandResult::invalid(
                        crate::commands::CommandInvalidity::InvalidCollectibleConfiguration,
                    ),
                },
            ),
            GameplayCommand::CollectItem {
                player,
                collectible,
                amount,
            } => Some(self.collect(world, player, collectible, amount)),
            _ => None,
        }
    }
}

impl InventorySystem {
    fn initialize(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        capacity: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        match world.initialize_inventory(player, capacity) {
            Ok(()) => CommandResult::success(true),
            Err(crate::components::InventoryError::PlayerNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound)
            }
            Err(crate::components::InventoryError::AlreadyInitialized) => CommandResult::rejected(
                crate::commands::CommandRejection::InventoryAlreadyInitialized,
            ),
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            ),
        }
    }

    fn add(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player) || !player_state.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        let Some(definition) = world.item_definition(item).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            );
        };
        if amount == 0 || amount > definition.max_stack {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            );
        }
        let Some(inventory) = world.inventory_mut(player) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::InventoryNotInitialized,
            );
        };
        match inventory.try_add(item, amount) {
            Ok(()) => {
                world.publish(crate::events::GameplayEvent::ItemAdded {
                    player: crate::types::PlayerId(player),
                    item,
                    amount,
                });
                CommandResult::success(true)
            }
            Err(crate::components::InventoryError::CapacityExceeded) => CommandResult::rejected(
                crate::commands::CommandRejection::InventoryCapacityExceeded,
            ),
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            ),
        }
    }

    fn remove(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player) || !player_state.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        if world.item_definition(item).is_none() {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            );
        }
        let Some(inventory) = world.inventory_mut(player) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::InventoryNotInitialized,
            );
        };
        match inventory.try_remove(item, amount) {
            Ok(()) => {
                world.publish(crate::events::GameplayEvent::ItemRemoved {
                    player,
                    item,
                    amount,
                });
                CommandResult::success(true)
            }
            Err(crate::components::InventoryError::InsufficientQuantity) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryInsufficientQuantity,
                )
            }
            Err(_) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            ),
        }
    }

    fn transfer(
        &self,
        world: &mut GameplayWorld,
        from: engine::EntityId,
        to: engine::EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        match world.transfer_inventory_item(from, to, item, amount) {
            Ok(()) => CommandResult::success(true),
            Err(crate::world::InventoryTransferError::PlayerNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound)
            }
            Err(crate::world::InventoryTransferError::PlayerInactive) => {
                CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive)
            }
            Err(crate::world::InventoryTransferError::SamePlayer) => {
                CommandResult::rejected(crate::commands::CommandRejection::TransferToSelf)
            }
            Err(crate::world::InventoryTransferError::InvalidQuantity) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            ),
            Err(crate::world::InventoryTransferError::ItemDefinitionNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::ItemDefinitionNotFound)
            }
            Err(crate::world::InventoryTransferError::InventoryNotInitialized) => {
                CommandResult::rejected(crate::commands::CommandRejection::InventoryNotInitialized)
            }
            Err(crate::world::InventoryTransferError::InsufficientQuantity) => {
                CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryInsufficientQuantity,
                )
            }
            Err(crate::world::InventoryTransferError::CapacityExceeded) => CommandResult::rejected(
                crate::commands::CommandRejection::InventoryCapacityExceeded,
            ),
            Err(crate::world::InventoryTransferError::StackExceeded) => CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            ),
            Err(crate::world::InventoryTransferError::ArithmeticOverflow) => {
                CommandResult::invalid(
                    crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
                )
            }
        }
    }

    fn collect(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        collectible: engine::EntityId,
        amount: u32,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(
                crate::commands::CommandRejection::SimulationNotRunning,
            );
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player) || !player_state.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
        }
        let Some(item_state) = world.collectible(collectible).copied() else {
            return CommandResult::rejected(crate::commands::CommandRejection::CollectibleNotFound);
        };
        if !item_state.active || item_state.amount == 0 {
            return CommandResult::rejected(
                crate::commands::CommandRejection::CollectibleUnavailable,
            );
        }
        if item_state.owner.is_some_and(|owner| owner != player) {
            return CommandResult::rejected(crate::commands::CommandRejection::CollectibleNotOwned);
        }
        if amount == 0 || amount > item_state.amount {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            );
        }
        let Some(definition) = world.item_definition(item_state.item).copied() else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::ItemDefinitionNotFound,
            );
        };
        if amount > definition.max_stack {
            return CommandResult::invalid(
                crate::commands::CommandInvalidity::InvalidInventoryConfiguration,
            );
        }
        let player_distance = Vec3Distance::between(player_state.position, item_state.position);
        let ship_distance = world
            .ship_position()
            .and_then(|position| Vec3Distance::between(position, item_state.position));
        let player_in_range =
            player_distance.is_some_and(|value| value as f32 <= item_state.interaction_range);
        let ship_in_range =
            ship_distance.is_some_and(|value| value as f32 <= item_state.interaction_range);
        if !player_in_range && !ship_in_range {
            return CommandResult::rejected(
                crate::commands::CommandRejection::CollectibleOutOfRange,
            );
        }
        let Some(inventory) = world.inventory(player) else {
            return CommandResult::rejected(
                crate::commands::CommandRejection::InventoryNotInitialized,
            );
        };
        if !inventory.can_add(amount) {
            return CommandResult::rejected(
                crate::commands::CommandRejection::InventoryCapacityExceeded,
            );
        }
        {
            let Some(inventory) = world.inventory_mut(player) else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryNotInitialized,
                );
            };
            if inventory.try_add(item_state.item, amount).is_err() {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::InventoryCapacityExceeded,
                );
            }
        }
        let remaining;
        {
            let Some(item_mut) = world.collectible_mut(collectible) else {
                return CommandResult::rejected(
                    crate::commands::CommandRejection::CollectibleNotFound,
                );
            };
            item_mut.amount -= amount;
            item_mut.active = item_mut.amount > 0;
            remaining = item_mut.amount;
        }
        world.publish(crate::events::GameplayEvent::ItemAdded {
            player: crate::types::PlayerId(player),
            item: item_state.item,
            amount,
        });
        world.publish(crate::events::GameplayEvent::ItemCollected {
            player,
            collectible,
            item: item_state.item,
            amount,
            remaining,
        });
        if remaining == 0 {
            world.publish(crate::events::GameplayEvent::CollectibleDepleted {
                collectible,
                item: item_state.item,
            });
        }
        CommandResult::success(true)
    }
}

pub mod combat {
    use crate::{
        components::Health,
        domains::enemy::EnemyState,
        events::{EventLog, GameplayEvent},
        types::EnemyId,
    };

    pub fn apply_damage(target: EnemyId, health: &mut Health, amount: f32, events: &mut EventLog) {
        health.current = (health.current - amount.max(0.0)).max(0.0);
        let state = if health.is_alive() {
            EnemyState::Alert
        } else {
            EnemyState::Defeated
        };
        events.push(GameplayEvent::DamageApplied {
            target,
            amount,
            state,
        });
        events.push(GameplayEvent::EnemyStateChanged {
            enemy: target,
            state,
        });
    }
}

pub mod inventory {
    use crate::{
        components::Inventory,
        events::{EventLog, GameplayEvent},
        types::{ItemId, PlayerId},
    };

    pub fn add_item(
        player: PlayerId,
        inventory: &mut Inventory,
        item: ItemId,
        amount: u32,
        events: &mut EventLog,
    ) {
        inventory.add(item, amount);
        events.push(GameplayEvent::ItemAdded {
            player,
            item,
            amount,
        });
    }
}

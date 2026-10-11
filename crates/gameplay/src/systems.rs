use std::collections::BTreeSet;

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

impl GameplaySystem for PlayerSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::CreatePlayer => {
                world.create_player();
                CommandResult::success(true)
            }
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
            if !world.entities().contains(entity) || player.is_active {
                return CommandResult::success(false);
            }
            if let Some(updated) = world.player_mut(entity) {
                updated.is_active = true;
            }
            world.publish(crate::events::GameplayEvent::PlayerActivated {
                player: player.id(),
            });
            CommandResult::success(true)
        } else {
            if !world.entities().contains(entity) || !player.is_active {
                return CommandResult::success(false);
            }
            if let Some(updated) = world.player_mut(entity) {
                updated.is_active = false;
            }
            world.publish(crate::events::GameplayEvent::PlayerDeactivated {
                player: player.id(),
            });
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
        let crate::interaction::InteractionBehavior::Station { station } = interactable.behavior
        else {
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
        Ok(())
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
            Err(crate::navigation::NavigationSelectionError::DestinationNotFound) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationNotFound)
            }
            Err(crate::navigation::NavigationSelectionError::DestinationUnknown) => {
                CommandResult::rejected(crate::commands::CommandRejection::DestinationUnknown)
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
            let deadline = started_at.0.saturating_add(duration.0);
            if to.elapsed.0 < deadline {
                continue;
            }
            let Some(destination_data) = world.destination(destination).cloned() else {
                continue;
            };
            if !world.navigation_mut().complete_travel(destination) {
                continue;
            }
            world.publish(crate::events::GameplayEvent::TravelArrived {
                destination,
                position: destination_data.position,
            });
            if destination_data.destination_type == crate::navigation::DestinationType::Planet
                && world.navigation_mut().discover_destination(destination)
            {
                world.publish(crate::events::GameplayEvent::PlanetDiscovered { destination });
            }
        }
    }
}

impl Default for NavigationSystem {
    fn default() -> Self {
        Self::new()
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
            GameplayCommand::StartTravel {
                player,
                pilot_station,
            } => self.start_travel(world, player, pilot_station),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
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
            GameplayCommand::AttackEnemy { player, enemy } => {
                Some(self.attack(world, player, enemy, true))
            }
            GameplayCommand::EnemyAttackPlayer { enemy, player } => {
                Some(self.attack(world, enemy, player, false))
            }
            GameplayCommand::SetEnemyActive { enemy, active } => {
                Some(self.set_enemy_active(world, enemy, active))
            }
            _ => None,
        }
    }
}

impl CombatSystem {
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
        let Some(combatant) = world.combatant_mut(player) else {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
        };
        if !combatant.is_player || !combatant.active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if combatant.is_defeated() {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        if combatant.weapon == weapon {
            return CommandResult::success(false);
        }
        combatant.weapon = weapon;
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
        if !attacker_state.active || !target_state.active {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantInactive);
        }
        if attacker_state.is_defeated() || target_state.is_defeated() {
            return CommandResult::rejected(crate::commands::CommandRejection::CombatantDefeated);
        }
        let attacker_position = if attacker_state.is_player {
            let Some(player) = world.player(attacker) else {
                return CommandResult::rejected(crate::commands::CommandRejection::PlayerNotFound);
            };
            player.position
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
        if distance.is_none() || distance.unwrap_or(u32::MAX) > attacker_state.weapon.range {
            return CommandResult::rejected(crate::commands::CommandRejection::OutOfAttackRange);
        }
        let now = world.time().elapsed;
        if let Some(last) = attacker_state.last_attack_at {
            if now.0 < last.0.saturating_add(attacker_state.weapon.cooldown.0) {
                return CommandResult::rejected(crate::commands::CommandRejection::AttackCooldown);
            }
        }

        let attack_type = attacker_state.weapon.attack_type;
        let base_damage = attacker_state.weapon.damage;
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
                world.publish(crate::events::GameplayEvent::PlayerDefeated { player: target });
            } else {
                world.publish(crate::events::GameplayEvent::EnemyDefeated { enemy: target });
            }
        }
        CommandResult::success(true)
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

pub struct InventorySystem;

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
        if !player_state.is_active {
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
        if !player_state.is_active {
            return CommandResult::rejected(crate::commands::CommandRejection::PlayerInactive);
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
        if !player_state.is_active {
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
        let distance = Vec3Distance::between(player_state.position, item_state.position);
        if distance.is_none() || distance.unwrap_or(u32::MAX) as f32 > item_state.interaction_range
        {
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
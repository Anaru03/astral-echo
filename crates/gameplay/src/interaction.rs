use engine::{EntityId, Vec3};

use crate::{
    door::{Door, DoorRole},
    station::{Station, StationType},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleState {
    Off,
    On,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleRole {
    Generic,
    ShipStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionBehavior {
    Console {
        state: ConsoleState,
        role: ConsoleRole,
    },
    Door {
        door: Door,
    },
    Station {
        station: Station,
    },
    ElectricalPanel,
    LifeSupportPanel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interactable {
    pub entity_id: EntityId,
    pub position: Vec3,
    pub enabled: bool,
    pub interaction_range: f32,
    pub behavior: InteractionBehavior,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionEffect {
    ConsoleStateChanged {
        state: ConsoleState,
        role: ConsoleRole,
    },
    DoorStateChanged {
        state: crate::domains::door::DoorState,
        role: DoorRole,
    },
    StationOccupied {
        player: EntityId,
        station_type: StationType,
    },
    ElectricalPanelInteractionStarted,
    LifeSupportPanelInteractionStarted,
}

impl Interactable {
    pub fn console(entity_id: EntityId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Console {
                state: ConsoleState::Off,
                role: ConsoleRole::Generic,
            },
        }
    }

    pub fn ship_status_console(
        entity_id: EntityId,
        position: Vec3,
        interaction_range: f32,
    ) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Console {
                state: ConsoleState::Off,
                role: ConsoleRole::ShipStatus,
            },
        }
    }

    pub fn door(entity_id: EntityId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Door { door: Door::new() },
        }
    }

    pub fn bridge_door(entity_id: EntityId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Door {
                door: Door::with_role(DoorRole::CommandBridge),
            },
        }
    }

    pub fn bridge_door_with_requirement(
        entity_id: EntityId,
        position: Vec3,
        interaction_range: f32,
        requirement: crate::door::DoorAccessRequirement,
    ) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Door {
                door: Door::with_locked_access_requirement(DoorRole::CommandBridge, requirement),
            },
        }
    }

    pub fn station(
        entity_id: EntityId,
        position: Vec3,
        interaction_range: f32,
        station: Station,
    ) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::Station { station },
        }
    }

    pub fn electrical_panel(entity_id: EntityId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::ElectricalPanel,
        }
    }

    pub fn life_support_panel(entity_id: EntityId, position: Vec3, interaction_range: f32) -> Self {
        Self {
            entity_id,
            position,
            enabled: true,
            interaction_range,
            behavior: InteractionBehavior::LifeSupportPanel,
        }
    }

    pub fn execute(&mut self, actor: EntityId) -> Result<InteractionEffect, InteractionFailure> {
        match self.behavior {
            InteractionBehavior::Console { state, role } => {
                let next = match state {
                    ConsoleState::Off => ConsoleState::On,
                    ConsoleState::On => ConsoleState::Off,
                };
                self.behavior = InteractionBehavior::Console { state: next, role };
                Ok(InteractionEffect::ConsoleStateChanged { state: next, role })
            }
            InteractionBehavior::Door { mut door } => {
                door.toggle().map_err(InteractionFailure::Door)?;
                self.behavior = InteractionBehavior::Door { door };
                Ok(InteractionEffect::DoorStateChanged {
                    state: door.state,
                    role: door.role,
                })
            }
            InteractionBehavior::Station { mut station } => {
                station.occupy(actor).map_err(InteractionFailure::Station)?;
                self.behavior = InteractionBehavior::Station { station };
                Ok(InteractionEffect::StationOccupied {
                    player: actor,
                    station_type: station.station_type,
                })
            }
            InteractionBehavior::ElectricalPanel => {
                Ok(InteractionEffect::ElectricalPanelInteractionStarted)
            }
            InteractionBehavior::LifeSupportPanel => {
                Ok(InteractionEffect::LifeSupportPanelInteractionStarted)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionFailure {
    Door(crate::door::DoorTransitionError),
    Station(crate::station::StationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionRegistrationError {
    InvalidPosition,
    InvalidRange,
    EntityNotFound,
    DuplicateEntity,
}
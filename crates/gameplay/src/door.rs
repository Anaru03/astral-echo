use crate::domains::door::DoorState;
use crate::types::{MissionId, ObjectiveId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoorRole {
    Generic,
    CommandBridge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoorAccessRequirement {
    None,
    MissionObjectives {
        mission: MissionId,
        required: [ObjectiveId; 2],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Door {
    pub state: DoorState,
    pub role: DoorRole,
    pub access_requirement: DoorAccessRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoorTransitionError {
    Locked,
    InvalidTransition,
}

impl Door {
    pub const fn new() -> Self {
        Self::with_role(DoorRole::Generic)
    }

    pub const fn with_role(role: DoorRole) -> Self {
        Self::with_access_requirement(role, DoorAccessRequirement::None)
    }

    pub const fn with_access_requirement(
        role: DoorRole,
        access_requirement: DoorAccessRequirement,
    ) -> Self {
        Self {
            state: DoorState::Closed,
            role,
            access_requirement,
        }
    }

    pub const fn with_locked_access_requirement(
        role: DoorRole,
        access_requirement: DoorAccessRequirement,
    ) -> Self {
        Self {
            state: DoorState::Locked,
            role,
            access_requirement,
        }
    }

    pub fn open(&mut self) -> Result<(), DoorTransitionError> {
        match self.state {
            DoorState::Closed => {
                self.state = DoorState::Open;
                Ok(())
            }
            DoorState::Locked => Err(DoorTransitionError::Locked),
            _ => Err(DoorTransitionError::InvalidTransition),
        }
    }

    pub fn close(&mut self) -> Result<(), DoorTransitionError> {
        match self.state {
            DoorState::Open => {
                self.state = DoorState::Closed;
                Ok(())
            }
            _ => Err(DoorTransitionError::InvalidTransition),
        }
    }

    pub fn lock(&mut self) -> Result<(), DoorTransitionError> {
        match self.state {
            DoorState::Closed => {
                self.state = DoorState::Locked;
                Ok(())
            }
            _ => Err(DoorTransitionError::InvalidTransition),
        }
    }

    pub fn unlock(&mut self) -> Result<(), DoorTransitionError> {
        match self.state {
            DoorState::Locked => {
                self.state = DoorState::Closed;
                Ok(())
            }
            _ => Err(DoorTransitionError::InvalidTransition),
        }
    }

    pub fn toggle(&mut self) -> Result<(), DoorTransitionError> {
        match self.state {
            DoorState::Closed => self.open(),
            DoorState::Open => self.close(),
            _ => Err(DoorTransitionError::InvalidTransition),
        }
    }
}

impl Default for Door {
    fn default() -> Self {
        Self::new()
    }
}
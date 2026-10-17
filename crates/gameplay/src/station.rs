use engine::EntityId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StationType {
    Piloting,
    Navigation,
    Engineering,
    LifeSupport,
    Weapons,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StationState {
    Available,
    Occupied,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Station {
    pub station_type: StationType,
    pub occupant: Option<EntityId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StationError {
    Occupied,
    NotOccupiedByPlayer,
}

impl Station {
    pub const fn new(station_type: StationType) -> Self {
        Self {
            station_type,
            occupant: None,
        }
    }

    pub const fn state(&self, enabled: bool) -> StationState {
        if !enabled {
            StationState::Disabled
        } else if self.occupant.is_some() {
            StationState::Occupied
        } else {
            StationState::Available
        }
    }

    pub fn occupy(&mut self, player: EntityId) -> Result<(), StationError> {
        if self.occupant.is_some() {
            return Err(StationError::Occupied);
        }
        self.occupant = Some(player);
        Ok(())
    }

    pub fn release(&mut self, player: EntityId) -> Result<(), StationError> {
        if self.occupant != Some(player) {
            return Err(StationError::NotOccupiedByPlayer);
        }
        self.occupant = None;
        Ok(())
    }
}
use engine::{EntityId, GameTime};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlayerId(pub EntityId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MissionId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectiveId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ItemId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DoorId(pub EntityId);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EnemyId(pub EntityId);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ShipId(pub EntityId);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PuzzleId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameResult {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationPhase {
    Loading,
    Running,
    Paused,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameplayState {
    pub phase: SimulationPhase,
    pub time: GameTime,
}

impl Default for GameplayState {
    fn default() -> Self {
        Self {
            phase: SimulationPhase::Loading,
            time: GameTime::new(),
        }
    }
}
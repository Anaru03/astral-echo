pub mod commands;
pub mod components;
pub mod domains;
pub mod door;
pub mod electrical;
pub mod events;
pub mod interaction;
pub mod mission;
pub mod mission_system;
pub mod objective;
pub mod pipeline;
pub mod player;
pub mod station;
pub mod systems;
pub mod types;
pub mod world;

pub use commands::{
    CommandInvalidity, CommandRejection, CommandResult, CommandStatus, GameplayCommand,
};
pub use components::{Health, Inventory, ResourceAmount};
pub use door::{Door, DoorRole, DoorTransitionError};
pub use electrical::{
    BreakerId, BreakerState, ElectricalBreaker, ElectricalBreakerConfiguration, ElectricalPanel,
    ElectricalPanelConfiguration, ElectricalPanelProgress, ElectricalPanelRegistrationError,
    ElectricalPuzzleState,
};
pub use events::{EventLog, GameplayEvent};
pub use interaction::{
    ConsoleRole, ConsoleState, Interactable, InteractionBehavior, InteractionEffect,
    InteractionRegistrationError,
};
pub use mission::{Mission, MissionDefinitionError, MissionProgress, MissionRegistrationError};
pub use mission_system::MissionSystem;
pub use objective::{Objective, ObjectiveTrigger};
pub use pipeline::GameplayProcessor;
pub use player::{MovementParameters, Orientation, Player};
pub use station::{Station, StationError, StationState, StationType};
pub use systems::{DoorSystem, ElectricalSystem, InteractionSystem, StationSystem};
pub use types::{GameResult, GameplayState, SimulationPhase};
pub use world::GameplayWorld;
pub mod collectible;
pub mod combat;
pub mod commands;
pub mod components;
pub mod domains;
pub mod door;
pub mod electrical;
pub mod events;
pub mod integrity;
pub mod interaction;
pub mod life_support;
pub mod mission;
pub mod mission_system;
pub mod navigation;
pub mod objective;
pub mod pipeline;
pub mod player;
pub mod resources;
pub mod station;
pub mod systems;
pub mod types;
pub mod world;

pub use collectible::{
    Collectible, CollectibleError, ItemCategory, ItemDefinition, ItemDefinitionError,
};
pub use combat::{
    AttackType, CombatConfigurationError, Combatant, EnemyArchetype, EnemyConfiguration, Shield,
    WeaponConfiguration,
};
pub use commands::{
    CommandInvalidity, CommandRejection, CommandResult, CommandStatus, GameplayCommand,
};
pub use components::{Health, Inventory, InventoryError, ResourceAmount};
pub use door::{Door, DoorAccessRequirement, DoorRole, DoorTransitionError};
pub use electrical::{
    BreakerId, BreakerState, ElectricalBreaker, ElectricalBreakerConfiguration, ElectricalPanel,
    ElectricalPanelConfiguration, ElectricalPanelProgress, ElectricalPanelRegistrationError,
    ElectricalPuzzleState,
};
pub use events::{EventLog, GameplayEvent};
pub use integrity::{IntegrityError, ShipIntegrity, StructuralState};
pub use interaction::{
    ConsoleRole, ConsoleState, Interactable, InteractionBehavior, InteractionEffect,
    InteractionRegistrationError,
};
pub use life_support::{
    CompartmentId, LifeSupportConfiguration, LifeSupportPanel, LifeSupportProgress,
    LifeSupportRegistrationError, LifeSupportState, OxygenCompartment,
    OxygenCompartmentConfiguration,
};
pub use mission::{Mission, MissionDefinitionError, MissionProgress, MissionRegistrationError};
pub use mission_system::MissionSystem;
pub use navigation::{
    Destination, DestinationId, DestinationType, DiscoveryState, NavigationMovementError,
    NavigationRegistrationError, NavigationState, ScannerError, ShipMovementParameters,
    ShipOrientation, SignalDetection, SignalKnowledge, SignalScanner, TravelProgress, TravelState,
};
pub use objective::{Objective, ObjectiveTrigger, ObjectiveTriggerError};
pub use pipeline::GameplayProcessor;
pub use player::{MovementParameters, Orientation, Player};
pub use resources::{FuelError, FuelTank, ShipResources};
pub use station::{Station, StationError, StationState, StationType};
pub use systems::{
    CombatSystem, DoorSystem, ElectricalSystem, HullIntegritySystem, InteractionSystem,
    InventorySystem, LifeSupportSystem, NavigationSystem, PilotSystem, ResourceSystem,
    ScannerSystem, StationSystem,
};
pub use types::{GameResult, GameplayState, SimulationPhase};
pub use world::GameplayWorld;
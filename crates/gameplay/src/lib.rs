pub mod collectible;
pub mod companion;
pub mod combat;
pub mod commands;
pub mod components;
pub mod derelict;
pub mod domains;
pub mod door;
pub mod electrical;
pub mod encounter;
pub mod events;
pub mod hazard;
pub mod healing;
pub mod healing_system;
pub mod guidance;
pub mod integrity;
pub mod interaction;
pub mod knowledge;
pub mod knowledge_system;
pub mod life_support;
pub mod mission;
pub mod mission_system;
pub mod navigation;
pub mod objective;
pub mod pipeline;
pub mod player;
pub mod persistence;
pub mod resources;
pub mod revival;
pub mod revival_system;
pub mod reward;
pub mod reward_system;
pub mod scaling;
pub mod station;
pub mod systems;
pub mod types;
pub mod world;
pub mod world_activity;

pub use collectible::{
    Collectible, CollectibleError, ItemCategory, ItemDefinition, ItemDefinitionError,
};
pub use companion::{
    Companion, CompanionActionMode, CompanionBehavior, CompanionError, CompanionRecommendation,
    NavigationAdvice, NavigationAlternativeRecommendation, NavigationAssessment, NavigationRisk,
    NavigationRecommendationReason, NavigationRouteOption, NavigationRouteRisk,
    NavigationRouteRiskClassification, NavigationRouteRiskSource, NavigationRouteComparison,
    TacticalAction, TacticalPriority, TacticalRecommendation, ERICK_NAME, FOLLOW_DISTANCE,
    FOLLOW_SPEED,
};
pub use combat::{
    AttackType, CombatConfigurationError, Combatant, EnemyArchetype, EnemyConfiguration, Shield,
    WeaponAmmunition, WeaponConfiguration,
};
pub use commands::{
    CommandInvalidity, CommandRejection, CommandResult, CommandStatus, GameplayCommand,
};
pub use components::{Health, Inventory, InventoryError, ResourceAmount};
pub use door::{Door, DoorAccessRequirement, DoorRole, DoorTransitionError};
pub use derelict::{Derelict, DerelictCommandError, DerelictError, DerelictId, DerelictReward, DerelictState};
pub use electrical::{
    BreakerId, BreakerState, ElectricalBreaker, ElectricalBreakerConfiguration, ElectricalPanel,
    ElectricalPanelConfiguration, ElectricalPanelProgress, ElectricalPanelRegistrationError,
    ElectricalPuzzleState,
};
pub use encounter::{
    EncounterId, SpatialEncounter, SpatialEncounterError, SpatialEncounterKind,
    SpatialEncounterState,
};
pub use events::{EventLog, GameplayEvent};
pub use hazard::{
    EnvironmentalHazard, EnvironmentalHazardError, EnvironmentalHazardType, HazardId,
};
pub use healing::{HealingEffect, HealingError, HealingResult};
pub use healing_system::HealingSystem;
pub use guidance::{
    GuidanceAction, GuidanceError, GuidanceObjective, GuidanceRequirement, GuidanceScope,
    GuidanceState, GuidanceStatus, GuidanceSystem,
};
pub use integrity::{
    IntegrityError, ShipIntegrity, ShipRepairConfiguration, ShipRepairError, StructuralState,
};
pub use interaction::{
    ConsoleRole, ConsoleState, Interactable, InteractionBehavior, InteractionEffect,
    InteractionRegistrationError,
};
pub use knowledge::{KnowledgeCategory, KnowledgeError, KnowledgeState, KnowledgeTarget};
pub use knowledge_system::KnowledgeSystem;
pub use life_support::{
    CompartmentId, LifeSupportConfiguration, LifeSupportPanel, LifeSupportProgress,
    LifeSupportRegistrationError, LifeSupportState, OxygenCompartment,
    OxygenCompartmentConfiguration,
};
pub use mission::{Mission, MissionDefinitionError, MissionProgress, MissionRegistrationError};
pub use mission_system::MissionSystem;
pub use navigation::{
    AutopilotError, AutopilotState, Destination, DestinationId, DestinationType, DiscoveryState,
    NavigationInitializationError, NavigationMovementError, NavigationRegistrationError,
    NavigationRoute, NavigationRouteError, NavigationRouteState, NavigationState, ScannerError,
    ShipMovementParameters, ShipOrientation, SignalDetection, SignalKnowledge, SignalScanner,
    TravelProgress, TravelState,
};
pub use objective::{Objective, ObjectiveTrigger, ObjectiveTriggerError};
pub use pipeline::GameplayProcessor;
pub use player::{MovementParameters, Orientation, Player};
pub use persistence::{GameplaySaveState, SaveError, GAMEPLAY_SAVE_SCHEMA_VERSION};
pub use resources::{FuelError, FuelTank, ShipResources};
pub use revival::{RevivalError, ReviveEffect, ReviveResult};
pub use revival_system::RevivalSystem;
pub use reward::{
    EnemyReward, RewardConfigurationError, RewardDistributionPolicy, RewardEntry, RewardError,
};
pub use reward_system::RewardSystem;
pub use scaling::{GameplayScaling, GroupSize, ScalingError};
pub use station::{Station, StationError, StationState, StationType};
pub use systems::{
    CombatSystem, CompanionSystem, DoorSystem, ElectricalSystem, EnemyAiSystem, EnvironmentalHazardSystem,
    HullIntegritySystem, InteractionSystem, InventorySystem, LifeSupportSystem, NavigationSystem,
    PilotSystem, ResourceSystem, ScannerSystem, SpatialEncounterSystem, StationSystem,
    DerelictSystem, FunctionalWorldSystem,
};
pub use types::{GameResult, GameplayState, SimulationPhase};
pub use world::{GameplayWorld, InventoryTransferError, PlayerCreationError, MAX_LOCAL_PLAYERS};
pub use world_activity::{
    CombatZone, FunctionalWorld, FunctionalWorldError, ResourceSite, StorySite, TradeMarket,
    TradeOffer, WorldActivity, WorldActivityError, WorldCategory,
};
pub mod collectible;
pub mod combat;
pub mod commands;
pub mod companion;
pub mod components;
pub mod derelict;
pub mod domains;
pub mod door;
pub mod electrical;
pub mod encounter;
pub mod events;
pub mod guidance;
pub mod hazard;
pub mod healing;
pub mod healing_system;
pub mod integrity;
pub mod interaction;
pub mod knowledge;
pub mod knowledge_system;
pub mod life_support;
pub mod mission;
pub mod mission_system;
pub mod navigation;
pub mod objective;
pub mod persistence;
pub mod pipeline;
pub mod player;
pub mod resources;
pub mod revival;
pub mod revival_system;
pub mod reward;
pub mod reward_system;
pub mod scaling;
pub mod station;
pub mod systems;
pub mod types;
pub mod visual_events;
pub mod world;
pub mod world_activity;

pub use collectible::{
    Collectible, CollectibleError, ItemCategory, ItemDefinition, ItemDefinitionError,
};
pub use combat::{
    weapon_for_item, AttackType, CombatConfigurationError, Combatant, EnemyArchetype,
    EnemyConfiguration, Shield, WeaponAmmunition, WeaponConfiguration, ENERGY_WEAPON_ITEM,
    PLASMA_WEAPON_ITEM,
};
pub use companion::{
    Companion, CompanionActionMode, CompanionBehavior, CompanionError, CompanionRecommendation,
    ConsistencyAssessment, ConsistencyAuditState, ConsistencyEvidence, Contradiction,
    ContradictionCategory, ContradictionReason, ContradictionSeverity,
    CooperativeEncounterReadinessAssessment, CooperativeEncounterReadinessStatus,
    CooperativeNavigationAction, CooperativeNavigationContingencyAction,
    CooperativeNavigationContingencyAssessment, CooperativeNavigationContingencyStatus,
    CooperativeNavigationRiskAssessment, CooperativeNavigationRiskStatus,
    CooperativePreparationAction, CooperativeRecoveryAssignmentAssessment,
    CooperativeRecoveryAssignmentStatus, CooperativeRecoveryPlanAssessment,
    CooperativeRecoveryPlanDependency, CooperativeRecoveryPlanStatus, CooperativeRecoveryPlanTask,
    CooperativeRecoveryPlanTaskStatus, CooperativeRecoveryTask, CooperativeRecoveryTaskAction,
    CooperativeRecoveryTaskStatus, CooperativeSupportAssessment, CooperativeTacticalAssessment,
    CooperativeTaskCompatibility, CooperativeTaskCompatibilityAssessment,
    CooperativeTaskCompatibilityReason, CooperativeTaskCompatibilityStatus, CoordinationAction,
    CoordinationAssessment, CoordinationCategory, CoordinationPriority, CoordinationReason,
    CoordinationStep, CoordinationStepStatus, ExplainedAlternative, ExplanationChangeReason,
    ExplanationEvidence, ExplanationEvidenceKind, ExplanationReason, ExplanationStatus,
    ExplorationAction, ExplorationAssessment, ExplorationPriority, ExplorationRecommendation,
    ExplorationRecommendationReason, ExplorationTargetCategory, FuelCondition, HullCondition,
    MaintenancePriority, MaintenanceRecommendation, NavigationAdvice,
    NavigationAlternativeRecommendation, NavigationAssessment, NavigationRecommendationReason,
    NavigationRisk, NavigationRouteComparison, NavigationRouteOption, NavigationRouteRisk,
    NavigationRouteRiskClassification, NavigationRouteRiskSource, OperationalAssessment,
    PlayerSupportRecommendation, PlayerTacticalRecommendation, PreparationAction,
    PreparationAssessment, PreparationCategory, PreparationPriority, PreparationReason,
    PreparationRequirement, ReadinessBlocker, ReadinessRequirement, RecommendationExplanation,
    RecommendationExplanationComparison, RecoveryAction, RecoveryAlternative, RecoveryAssessment,
    RecoveryBlocker, RecoveryCategory, RecoveryReason, RecoveryStatus, SupportEvidence,
    SupportRecommendationReason, SupportRecommendationStatus, TacticalAction,
    TacticalAssignmentEvidence, TacticalAssignmentReason, TacticalAssignmentStatus,
    TacticalPriority, TacticalRecommendation, ERICK_NAME, FOLLOW_DISTANCE, FOLLOW_SPEED,
};
pub const FUEL_RESOURCE_ITEM: types::ItemId = types::ItemId(92);
pub const OXYGEN_RESOURCE_ITEM: types::ItemId = types::ItemId(93);
pub use commands::{
    CommandInvalidity, CommandRejection, CommandResult, CommandStatus, GameplayCommand,
};
pub use components::{Health, Inventory, InventoryError, ResourceAmount};
pub use derelict::{
    Derelict, DerelictCommandError, DerelictError, DerelictId, DerelictReward, DerelictState,
};
pub use door::{Door, DoorAccessRequirement, DoorRole, DoorTransitionError};
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
pub use guidance::{
    GuidanceAction, GuidanceError, GuidanceObjective, GuidanceRequirement, GuidanceScope,
    GuidanceState, GuidanceStatus, GuidanceSystem,
};
pub use hazard::{
    EnvironmentalHazard, EnvironmentalHazardError, EnvironmentalHazardType, HazardId,
};
pub use healing::{HealingEffect, HealingError, HealingResult};
pub use healing_system::HealingSystem;
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
    is_free_navigation_destination, AutopilotError, AutopilotState, Destination, DestinationId,
    DestinationType, DiscoveryState, FreeNavigationEvaluation, FreeNavigationEvaluationError,
    FreeNavigationProposal, FreeNavigationRisk, FreeNavigationSelectionError,
    FreeNavigationStartError, NavigationInitializationError, NavigationMovementError,
    NavigationRegistrationError, NavigationRoute, NavigationRouteError, NavigationRouteState,
    NavigationState, PlanetFlightState, PlanetPhysics, ScannerError, ShipMovementParameters,
    ShipOrientation, SignalDetection, SignalKnowledge, SignalScanner, TravelProgress, TravelState,
    SUN_DESTINATION_ID, TESSARA_DESTINATION_ID, VEYRA_DESTINATION_ID,
};
pub use objective::{Objective, ObjectiveTrigger, ObjectiveTriggerError};
pub use persistence::{GameplaySaveState, SaveError, GAMEPLAY_SAVE_SCHEMA_VERSION};
pub use pipeline::GameplayProcessor;
pub use player::{MovementParameters, Orientation, Player};
pub use resources::{FuelError, FuelTank, ShipResources, ShipUpgrade};
pub use revival::{RevivalError, ReviveEffect, ReviveResult};
pub use revival_system::RevivalSystem;
pub use reward::{
    EnemyReward, RewardConfigurationError, RewardDistributionPolicy, RewardEntry, RewardError,
};
pub use reward_system::RewardSystem;
pub use scaling::{GameplayScaling, GroupSize, ScalingError};
pub use station::{Station, StationError, StationState, StationType};
pub use systems::{
    CombatSystem, CompanionSystem, DerelictSystem, DoorSystem, ElectricalSystem, EnemyAiSystem,
    EnvironmentalHazardSystem, FunctionalWorldSystem, HullIntegritySystem, InteractionSystem,
    InventorySystem, LifeSupportSystem, NavigationSystem, PilotSystem, ResourceSystem,
    ScannerSystem, SpatialEncounterSystem, StationSystem,
};
pub use types::{GameResult, GameplayState, SimulationPhase};
pub use visual_events::{GameplayVisualEvent, GameplayVisualEventKind, VISUAL_EVENT_DURATION};
pub use world::{GameplayWorld, InventoryTransferError, PlayerCreationError, MAX_LOCAL_PLAYERS};
pub use world_activity::{
    CombatZone, FunctionalWorld, FunctionalWorldError, ResourceSite, StorySite, TradeMarket,
    TradeOffer, WorldActivity, WorldActivityError, WorldCategory,
};

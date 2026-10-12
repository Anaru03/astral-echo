use crate::collectible::ItemDefinition;
use crate::companion::CompanionBehavior;
use crate::combat::{EnemyConfiguration, WeaponConfiguration};
use crate::derelict::Derelict;
use crate::world_activity::FunctionalWorld;
use crate::electrical::{BreakerId, BreakerState, ElectricalPanelConfiguration};
use crate::encounter::{EncounterId, SpatialEncounter};
use crate::knowledge::KnowledgeTarget;
use crate::life_support::{CompartmentId, LifeSupportConfiguration};
use crate::navigation::ShipMovementParameters;
use crate::navigation::{Destination, DestinationId, NavigationRoute};
use crate::reward::EnemyReward;
use crate::types::{MissionId, ObjectiveId};
use engine::{Duration, EntityId, Vec3};

#[derive(Clone, Debug, PartialEq)]
pub enum GameplayCommand {
    AdvanceTime {
        delta: Duration,
    },
    CreatePlayer,
    CreateCompanion {
        followed_player: EntityId,
        position: Vec3,
    },
    SetCompanionBehavior {
        companion: EntityId,
        behavior: CompanionBehavior,
    },
    MovePlayer {
        player: EntityId,
        direction: Vec3,
        delta: Duration,
    },
    RotatePlayer {
        player: EntityId,
        yaw_delta_degrees: f32,
        pitch_delta_degrees: f32,
    },
    SetPlayerActive {
        player: EntityId,
        active: bool,
    },
    Interact {
        player: EntityId,
        target: EntityId,
    },
    CreateElectricalPanel {
        position: Vec3,
        interaction_range: f32,
        configuration: ElectricalPanelConfiguration,
    },
    SetElectricalBreakerState {
        player: EntityId,
        panel: EntityId,
        breaker: BreakerId,
        state: BreakerState,
    },
    CreateLifeSupportPanel {
        position: Vec3,
        interaction_range: f32,
        configuration: LifeSupportConfiguration,
    },
    SetOxygenAllocation {
        player: EntityId,
        panel: EntityId,
        compartment: CompartmentId,
        allocation: u32,
    },
    RegisterDestination {
        destination: Destination,
    },
    RegisterNavigationRoute {
        route: NavigationRoute,
    },
    PlanAutopilot {
        player: EntityId,
        pilot_station: EntityId,
        origin: DestinationId,
        destination: DestinationId,
    },
    StartAutopilot {
        player: EntityId,
        pilot_station: EntityId,
    },
    ContinueAutopilot {
        player: EntityId,
        pilot_station: EntityId,
    },
    CancelAutopilot {
        player: EntityId,
        pilot_station: EntityId,
    },
    SelectDestination {
        player: EntityId,
        pilot_station: EntityId,
        destination: DestinationId,
    },
    StartTravel {
        player: EntityId,
        pilot_station: EntityId,
    },
    InitializeShipResources {
        ship: EntityId,
        capacity: u32,
        initial_fuel: u32,
    },
    AddShipFuel {
        ship: EntityId,
        amount: u32,
    },
    RemoveShipFuel {
        ship: EntityId,
        amount: u32,
    },
    CreateEnemy {
        configuration: EnemyConfiguration,
    },
    EquipPlayerWeapon {
        player: EntityId,
        weapon: WeaponConfiguration,
    },
    AttackEnemy {
        player: EntityId,
        enemy: EntityId,
    },
    EnemyAttackPlayer {
        enemy: EntityId,
        player: EntityId,
    },
    EnemyAttackShip {
        enemy: EntityId,
        ship: EntityId,
    },
    SetEnemyActive {
        enemy: EntityId,
        active: bool,
    },
    RegisterEnemyReward {
        reward: EnemyReward,
    },
    InitializePlayerInventory {
        player: EntityId,
        capacity: u32,
    },
    RegisterItemDefinition {
        definition: ItemDefinition,
    },
    AddInventoryItem {
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    TransferInventoryItem {
        from: EntityId,
        to: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    RemoveInventoryItem {
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    CreateCollectible {
        item: crate::types::ItemId,
        amount: u32,
        position: Vec3,
        interaction_range: f32,
    },
    CollectItem {
        player: EntityId,
        collectible: EntityId,
        amount: u32,
    },
    RegisterKnowledge {
        target: KnowledgeTarget,
    },
    IdentifyKnowledge {
        target: KnowledgeTarget,
    },
    StudyKnowledge {
        player: EntityId,
        target: KnowledgeTarget,
    },
    UseHealingConsumable {
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    RevivePlayer {
        helper: EntityId,
        target: EntityId,
        item: crate::types::ItemId,
    },
    ConfigureShipMovement {
        ship: EntityId,
        parameters: ShipMovementParameters,
    },
    AccelerateShip {
        player: EntityId,
        pilot_station: EntityId,
        input: Vec3,
    },
    RotateShip {
        player: EntityId,
        pilot_station: EntityId,
        yaw_input: f32,
        pitch_input: f32,
        delta: Duration,
    },
    BrakeShip {
        player: EntityId,
        pilot_station: EntityId,
    },
    SetShipControlActive {
        player: EntityId,
        pilot_station: EntityId,
        active: bool,
    },
    ConfigureSignalScanner {
        ship: EntityId,
        range: f32,
        cooldown: Duration,
    },
    SetSignalScannerEnabled {
        ship: EntityId,
        enabled: bool,
    },
    ScanSignals {
        player: EntityId,
        ship: EntityId,
    },
    InitializeShipIntegrity {
        ship: EntityId,
        maximum: u32,
        initial: u32,
        critical_threshold: u32,
    },
    DamageShip {
        ship: EntityId,
        amount: u32,
    },
    RepairShip {
        ship: EntityId,
        amount: u32,
    },
    RepairShipAtStation {
        player: EntityId,
        ship: EntityId,
        station: EntityId,
    },
    RegisterEnvironmentalHazard {
        hazard: crate::hazard::EnvironmentalHazard,
    },
    RegisterSpatialEncounter {
        encounter: SpatialEncounter,
    },
    ActivateSpatialEncounter {
        encounter: EncounterId,
    },
    ResolveSpatialEncounter {
        player: EntityId,
        encounter: EncounterId,
    },
    RegisterDerelict {
        derelict: Derelict,
    },
    DiscoverDerelict {
        player: EntityId,
        derelict: crate::derelict::DerelictId,
    },
    ExploreDerelict {
        player: EntityId,
        derelict: crate::derelict::DerelictId,
    },
    ResolveDerelict {
        player: EntityId,
        derelict: crate::derelict::DerelictId,
    },
    RegisterFunctionalWorld {
        world: FunctionalWorld,
    },
    ExtractWorldResource {
        player: EntityId,
        destination: DestinationId,
    },
    BuyWorldItem {
        player: EntityId,
        destination: DestinationId,
        item: crate::types::ItemId,
        amount: u32,
    },
    SellWorldItem {
        player: EntityId,
        destination: DestinationId,
        item: crate::types::ItemId,
        amount: u32,
    },
    InteractStoryWorld {
        player: EntityId,
        destination: DestinationId,
    },
    ActivateWorldCombat {
        player: EntityId,
        destination: DestinationId,
    },
    LockDoor {
        door: EntityId,
    },
    UnlockDoor {
        door: EntityId,
    },
    ReleaseStation {
        station: EntityId,
        player: EntityId,
    },
    SetStationEnabled {
        station: EntityId,
        enabled: bool,
    },
    UnlockMission {
        mission: MissionId,
    },
    StartMission {
        mission: MissionId,
    },
    FailMission {
        mission: MissionId,
    },
    CompleteMission {
        mission: MissionId,
    },
    StartObjective {
        objective: ObjectiveId,
    },
    CompleteObjective {
        objective: ObjectiveId,
    },
    FailObjective {
        objective: ObjectiveId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRejection {
    SimulationNotRunning,
    InvalidDuration,
    PlayerNotFound,
    PlayerInactive,
    MaximumPlayersReached,
    CompanionNotFound,
    CompanionPlayerInvalid,
    CompanionInactive,
    CompanionKnowledgeUnavailable,
    CompanionInvalidTarget,
    TargetNotFound,
    TargetDisabled,
    OutOfInteractionRange,
    IncompatibleInteraction,
    InvalidDoorTransition,
    DoorLocked,
    StationOccupied,
    StationNotOccupiedByPlayer,
    MissionNotFound,
    ObjectiveNotFound,
    InvalidMissionTransition,
    InvalidObjectiveTransition,
    DependenciesIncomplete,
    RequiredObjectivesIncomplete,
    ElectricalPanelNotFound,
    ElectricalPanelDisabled,
    ElectricalPanelRepaired,
    BreakerNotFound,
    ElectricalInteractionRequired,
    LifeSupportPanelNotFound,
    LifeSupportPanelDisabled,
    LifeSupportPanelRepaired,
    CompartmentNotFound,
    LifeSupportInteractionRequired,
    LifeSupportCapacityExceeded,
    DestinationNotFound,
    DestinationUnknown,
    TravelActive,
    DestinationNotSelected,
    PilotStationNotFound,
    PilotStationDisabled,
    PilotStationNotOccupiedByPlayer,
    ShipNotFound,
    ResourcesNotInitialized,
    FuelCapacityExceeded,
    InsufficientFuel,
    ResourcesAlreadyInitialized,
    ResourceInitializationClosed,
    EnemyNotFound,
    CombatantInactive,
    CombatantDefeated,
    WeaponUnavailable,
    AmmunitionUnavailable,
    OutOfAttackRange,
    AttackCooldown,
    InvalidCombatTarget,
    InventoryNotInitialized,
    InventoryAlreadyInitialized,
    InventoryCapacityExceeded,
    InventoryInsufficientQuantity,
    ItemDefinitionNotFound,
    CollectibleNotFound,
    CollectibleUnavailable,
    CollectibleOutOfRange,
    CollectibleNotOwned,
    ShipControlInactive,
    InvalidShipMovement,
    ScannerNotFound,
    ScannerDisabled,
    ScannerCooldown,
    ShipIntegrityNotInitialized,
    ShipIntegrityAlreadyInitialized,
    ShipDestroyed,
    ShipIntegrityInvalidAmount,
    ShipIntegrityAtMaximum,
    RepairRequiresEngineeringStation,
    RepairStationNotFound,
    RepairStationDisabled,
    RepairStationNotOccupiedByPlayer,
    RepairStationWrongType,
    RepairMaterialNotFound,
    ShipAtMaximumIntegrity,
    EnvironmentalHazardAlreadyRegistered,
    EnvironmentalHazardNotFound,
    KnowledgeTargetNotFound,
    KnowledgeAlreadyRegistered,
    KnowledgeUnknown,
    KnowledgeNotIdentified,
    KnowledgeTransitionNotAllowed,
    RewardAlreadyRegistered,
    HealingNotConfigured,
    PlayerHealthAtMaximum,
    RevivalNotConfigured,
    ReviveTargetNotDefeated,
    ReviveTargetInvalid,
    ReviveOutOfRange,
    TransferToSelf,
    SpatialEncounterNotFound,
    SpatialEncounterAlreadyRegistered,
    SpatialEncounterAlreadyActive,
    SpatialEncounterAlreadyResolved,
    SpatialEncounterOutOfRange,
    SpatialEncounterRequiresScanner,
    SpatialEncounterRequiresActivePlayer,
    SpatialEncounterResolutionFailed,
    DerelictNotFound,
    DerelictAlreadyRegistered,
    DerelictNotDiscovered,
    DerelictAlreadyExploring,
    DerelictAlreadyResolved,
    DerelictOutOfRange,
    DerelictRequiresActivePlayer,
    DerelictShipNotOperational,
    DerelictEncounterUnavailable,
    DerelictEnemyRegistrationFailed,
    DerelictEnemyStillActive,
    DerelictRewardNotCollected,
    FunctionalWorldNotFound,
    FunctionalWorldAlreadyRegistered,
    FunctionalWorldDestinationNotDiscovered,
    FunctionalWorldOutOfRange,
    FunctionalWorldRequiresActivePlayer,
    FunctionalWorldShipNotOperational,
    WorldActivityNotFound,
    WorldInvalidQuantity,
    WorldInsufficientFunds,
    WorldPriceOverflow,
    WorldInventoryCapacity,
    WorldInventoryInsufficient,
    WorldResourceUnavailable,
    WorldCombatAlreadyActivated,
    WorldCombatResolved,
    WorldStoryAlreadyInteracted,
    WorldStoryKnowledgeUnavailable,
    WorldEnemyCreationFailed,
    NavigationRouteAlreadyRegistered,
    NavigationRouteNotFound,
    NavigationRouteNotDiscovered,
    AutopilotNotPlanned,
    AutopilotAlreadyPlanned,
    AutopilotNoRoute,
    AutopilotSameDestination,
    AutopilotOriginMismatch,
    AutopilotInterrupted,
    AutopilotNotInTransit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandInvalidity {
    UnsupportedCommand,
    InvalidMovementDirection,
    InvalidOrientation,
    InvalidInteractionData,
    InvalidElectricalConfiguration,
    InvalidLifeSupportConfiguration,
    InvalidNavigationConfiguration,
    InvalidResourceConfiguration,
    InvalidCombatConfiguration,
    InvalidInventoryConfiguration,
    InvalidItemDefinition,
    InvalidCollectibleConfiguration,
    InvalidShipMovementConfiguration,
    InvalidScannerConfiguration,
    InvalidShipIntegrityConfiguration,
    InvalidEnvironmentalHazardConfiguration,
    InvalidRewardConfiguration,
    InvalidHealingConfiguration,
    InvalidRevivalConfiguration,
    InvalidSpatialEncounterConfiguration,
    InvalidNavigationRouteConfiguration,
    InvalidDerelictConfiguration,
    InvalidFunctionalWorldConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStatus {
    Success { changed: bool },
    Rejected { reason: CommandRejection },
    Invalid { reason: CommandInvalidity },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandResult {
    pub status: CommandStatus,
}

impl CommandResult {
    pub const fn success(changed: bool) -> Self {
        Self {
            status: CommandStatus::Success { changed },
        }
    }
    pub const fn rejected(reason: CommandRejection) -> Self {
        Self {
            status: CommandStatus::Rejected { reason },
        }
    }
    pub const fn invalid(reason: CommandInvalidity) -> Self {
        Self {
            status: CommandStatus::Invalid { reason },
        }
    }
}
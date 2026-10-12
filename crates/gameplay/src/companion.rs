use engine::{Duration, EntityId, Vec3};

use crate::{
    encounter::EncounterId,
    guidance::{GuidanceAction, GuidanceRequirement},
    hazard::HazardId,
    knowledge::KnowledgeTarget,
    navigation::{AutopilotState, DestinationId, TravelState},
};

pub const ERICK_NAME: &str = "ERICK";
pub const FOLLOW_DISTANCE: f32 = 2.0;
pub const FOLLOW_SPEED: f32 = 5.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionBehavior {
    Accompany,
    Investigate(KnowledgeTarget),
    CombatSupport,
    Waiting,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionActionMode {
    PlayerRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompanionRecommendation {
    pub companion: EntityId,
    pub player: EntityId,
    pub action: GuidanceAction,
    pub mode: CompanionActionMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticalPriority {
    Critical,
    High,
    Normal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticalAction {
    DefeatEnemy(EntityId),
    AddressEncounter(EncounterId),
    WithdrawFromHazard(HazardId),
    PlayerAction(GuidanceAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TacticalRecommendation {
    pub companion: EntityId,
    pub player: EntityId,
    pub priority: TacticalPriority,
    pub action: TacticalAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationRisk {
    InsufficientFuel { required: u32, available: u32 },
    KnownHazard(HazardId),
    KnownEncounter(EncounterId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationAdvice {
    StartPlannedAutopilot { destination: DestinationId },
    MonitorAutopilot { destination: DestinationId },
    ReplanAutopilot { destination: DestinationId },
    InspectHazard(HazardId),
    AddressEncounter(EncounterId),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NavigationRouteRiskSource {
    Hazard(HazardId),
    Encounter(EncounterId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationRouteRiskClassification {
    NearSegment { segment: usize, distance: f64 },
    OutsideMargin { segment: usize, distance: f64 },
    NotEvaluable,
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavigationRouteRisk {
    pub source: NavigationRouteRiskSource,
    pub classification: NavigationRouteRiskClassification,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationRouteOption {
    pub route: Vec<DestinationId>,
    pub distance: f64,
    pub eta: Duration,
    pub fuel_required: u32,
    pub risks: Vec<NavigationRouteRisk>,
    pub critical_risks: u32,
    pub active_risks: u32,
    pub unevaluable_risks: u32,
    pub viable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationRecommendationReason {
    AvoidsKnownRisk(NavigationRouteRiskSource),
    FewerKnownRisks,
    LowerEta,
    LowerFuel,
    ShorterDistance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationAlternativeRecommendation {
    pub current: Vec<DestinationId>,
    pub alternative: Vec<DestinationId>,
    pub reason: NavigationRecommendationReason,
    pub avoided_risk: Option<NavigationRouteRiskSource>,
    pub current_eta: Duration,
    pub alternative_eta: Duration,
    pub current_fuel: u32,
    pub alternative_fuel: u32,
    pub current_distance: f64,
    pub alternative_distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationRouteComparison {
    pub current: NavigationRouteOption,
    pub alternatives: Vec<NavigationRouteOption>,
    pub recommendation: Option<NavigationAlternativeRecommendation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HullCondition {
    Normal,
    Reduced,
    Critical,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuelCondition {
    Unknown,
    Insufficient,
    Tight,
    Sufficient,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenancePriority {
    Critical,
    High,
    Normal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceRecommendation {
    AvoidShipActivity { ship: EntityId },
    ManageFuel {
        ship: EntityId,
        required: u32,
        available: u32,
    },
    RepairShip { ship: EntityId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperationalAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub ship: EntityId,
    pub hull: HullCondition,
    pub integrity: u32,
    pub maximum_integrity: u32,
    pub critical_threshold: u32,
    pub fuel: FuelCondition,
    pub fuel_available: Option<u32>,
    pub fuel_required: Option<u32>,
    pub navigation: Option<NavigationAssessment>,
    pub route_comparison: Option<NavigationRouteComparison>,
    pub priority: MaintenancePriority,
    pub recommendation: Option<MaintenanceRecommendation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorationTargetCategory {
    MissionObjective,
    Encounter,
    Derelict,
    Knowledge,
    WorldActivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorationPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorationRecommendationReason {
    MissionProgress,
    PendingEncounter,
    DerelictContent,
    KnowledgeProgress,
    WorldActivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorationAction {
    Guidance(GuidanceAction),
    ResolveEncounter(EncounterId),
    ExploreDerelict(crate::derelict::DerelictId),
    PerformWorldActivity {
        destination: DestinationId,
        category: crate::world_activity::WorldCategory,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorationRecommendation {
    pub companion: EntityId,
    pub player: EntityId,
    pub category: ExplorationTargetCategory,
    pub mission: Option<crate::types::MissionId>,
    pub objective: Option<crate::types::ObjectiveId>,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub reason: ExplorationRecommendationReason,
    pub priority: ExplorationPriority,
    pub fulfilled_requirements: Vec<GuidanceRequirement>,
    pub missing_requirements: Vec<GuidanceRequirement>,
    pub accessible: bool,
    pub action: ExplorationAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorationAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub primary: Option<ExplorationRecommendation>,
    pub alternatives: Vec<ExplorationRecommendation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationCategory {
    Ready,
    Fuel,
    Inventory,
    Ammunition,
    Repair,
    Activity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationPriority {
    Critical,
    High,
    Normal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationReason {
    Ready,
    FuelShortage,
    InventoryCapacity,
    AmmunitionShortage,
    RepairMaterialShortage,
    ActivityResourceShortage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationRequirement {
    Fuel { required: u32, available: u32 },
    InventoryCapacity { required: u32, available: u32 },
    Ammunition {
        item: Option<crate::types::ItemId>,
        required: u32,
        available: u32,
    },
    RepairMaterial {
        item: crate::types::ItemId,
        required: u32,
        available: u32,
    },
    ActivityResource {
        item: crate::types::ItemId,
        required: u32,
        available: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationAction {
    PlayerRequired,
    FreeInventory,
    CollectItem(EntityId),
    RepairShip(EntityId),
    PrepareCombat(EntityId),
    PrepareActivity(DestinationId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparationAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub exploration: Option<ExplorationRecommendation>,
    pub category: PreparationCategory,
    pub priority: PreparationPriority,
    pub reason: PreparationReason,
    pub prepared: bool,
    pub satisfied_requirements: Vec<PreparationRequirement>,
    pub missing_requirements: Vec<PreparationRequirement>,
    pub action: Option<PreparationAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinationPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinationStepStatus {
    Blocked,
    Recommended,
    Future,
    Satisfied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinationCategory {
    Preparation,
    Risk,
    Navigation,
    Exploration,
    Combat,
    Activity,
    Mission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinationAction {
    Preparation(PreparationAction),
    Exploration(ExplorationAction),
    Guidance(GuidanceAction),
    Tactical(TacticalAction),
    StartAutopilot(DestinationId),
    MonitorAutopilot(DestinationId),
    PlayerRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinationReason {
    PreparationRequired,
    CriticalRisk,
    RouteToObjective,
    ObjectiveAction,
    FutureDependency,
    ObjectiveCompleted,
    Ready,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoordinationStep {
    pub order: u32,
    pub category: CoordinationCategory,
    pub action: CoordinationAction,
    pub objective: Option<crate::types::ObjectiveId>,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub status: CoordinationStepStatus,
    pub requirements: Vec<PreparationRequirement>,
    pub reason: CoordinationReason,
    pub priority: CoordinationPriority,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoordinationAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub objective: Option<crate::types::ObjectiveId>,
    pub mission: Option<crate::types::MissionId>,
    pub blocked: bool,
    pub ready: bool,
    pub next_step: Option<CoordinationStep>,
    pub later_steps: Vec<CoordinationStep>,
    pub missing_requirements: Vec<PreparationRequirement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub autopilot: AutopilotState,
    pub travel: TravelState,
    pub destination: Option<DestinationId>,
    pub route: Vec<DestinationId>,
    pub eta: Option<Duration>,
    pub fuel_available: Option<u32>,
    pub fuel_required: Option<u32>,
    pub risks: Vec<NavigationRisk>,
    pub route_risks: Vec<NavigationRouteRisk>,
    pub recommendation_segment: Option<usize>,
    pub recommendation: Option<NavigationAdvice>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Companion {
    pub entity: EntityId,
    pub followed_player: EntityId,
    pub position: Vec3,
    pub behavior: CompanionBehavior,
    pub active: bool,
}

impl Companion {
    pub fn new(entity: EntityId, followed_player: EntityId, position: Vec3) -> Self {
        Self {
            entity,
            followed_player,
            position,
            behavior: CompanionBehavior::Waiting,
            active: true,
        }
    }

    pub fn validate_position(position: Vec3) -> Result<(), CompanionError> {
        if position.x.is_finite() && position.y.is_finite() && position.z.is_finite() {
            Ok(())
        } else {
            Err(CompanionError::InvalidPosition)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionError {
    PlayerNotFound,
    PlayerInactive,
    CompanionNotFound,
    InvalidPosition,
    CompanionInactive,
    KnowledgeTargetUnavailable,
    GuidanceUnavailable,
    InvalidTarget,
}
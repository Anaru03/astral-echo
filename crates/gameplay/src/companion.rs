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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticalAssignmentStatus {
    Actionable,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticalAssignmentReason {
    AssignedByDistance,
    AssignedByStableTieBreak,
    MissingWeapon,
    MissingAmmunition,
    AttackCooldown,
    OutOfRange,
    NoThreatAvailable,
    InvalidPosition,
    NoAvailablePlayer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TacticalAssignmentEvidence {
    pub description: String,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTacticalRecommendation {
    pub player: EntityId,
    pub threat: Option<EntityId>,
    pub action: Option<TacticalAction>,
    pub status: TacticalAssignmentStatus,
    pub reason: TacticalAssignmentReason,
    pub preferred: bool,
    pub evidence: Vec<TacticalAssignmentEvidence>,
    pub missing_requirements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeTacticalAssessment {
    pub companion: EntityId,
    pub followed_player: EntityId,
    pub active_players: Vec<EntityId>,
    pub scaling: crate::scaling::GameplayScaling,
    pub threats: Vec<EntityId>,
    pub recommendations: Vec<PlayerTacticalRecommendation>,
    pub unassigned_threats: Vec<EntityId>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportRecommendationStatus {
    Actionable,
    Blocked,
    Informational,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportRecommendationReason {
    MissingAmmunition,
    ProviderAvailable,
    ProviderInsufficientQuantity,
    ReceiverCapacityExceeded,
    StackExceeded,
    ItemDefinitionMissing,
    ReceiverUnavailable,
    ProviderUnavailable,
    NoProvider,
    PartialTransfer,
    NoSupportRequired,
    InsufficientEvidence,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SupportEvidence {
    pub description: String,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSupportRecommendation {
    pub needy_player: EntityId,
    pub provider: Option<EntityId>,
    pub item: Option<crate::types::ItemId>,
    pub amount: u32,
    pub blocked_action: Option<TacticalAction>,
    pub status: SupportRecommendationStatus,
    pub reason: SupportRecommendationReason,
    pub evidence: Vec<SupportEvidence>,
    pub missing_requirements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeSupportAssessment {
    pub companion: EntityId,
    pub followed_player: EntityId,
    pub active_players: Vec<EntityId>,
    pub threats: Vec<EntityId>,
    pub status: SupportRecommendationStatus,
    pub recommendations: Vec<PlayerSupportRecommendation>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeEncounterReadinessStatus {
    Prepared,
    PreparedWithReservations,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadinessRequirement {
    pub player: EntityId,
    pub requirement: PreparationRequirement,
    pub satisfied: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadinessBlocker {
    pub player: EntityId,
    pub threat: Option<EntityId>,
    pub requirement: Option<PreparationRequirement>,
    pub description: String,
    pub support: Option<PlayerSupportRecommendation>,
    pub evidence: Vec<SupportEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativePreparationAction {
    TransferInventory {
        from: EntityId,
        to: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    Prepare(PreparationAction),
    ExecuteTactical(TacticalAction),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeEncounterReadinessAssessment {
    pub companion: EntityId,
    pub followed_player: EntityId,
    pub active_players: Vec<EntityId>,
    pub threats: Vec<EntityId>,
    pub status: CooperativeEncounterReadinessStatus,
    pub satisfied_requirements: Vec<ReadinessRequirement>,
    pub missing_requirements: Vec<ReadinessRequirement>,
    pub blockers: Vec<ReadinessBlocker>,
    pub support_opportunities: Vec<PlayerSupportRecommendation>,
    pub actions: Vec<CooperativePreparationAction>,
    pub evidence: Vec<SupportEvidence>,
    pub conclusive: bool,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryCategory {
    Preparation,
    Navigation,
    Combat,
    Encounter,
    Inventory,
    Knowledge,
    Mission,
    Exploration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    Actionable,
    Recoverable,
    Blocked,
    Satisfied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReason {
    FuelInsufficient,
    ShipDestroyed,
    ShipNotOperational,
    PreparationRequired,
    ActiveThreat,
    ActiveEncounter,
    InventoryCapacity,
    KnowledgeMissing,
    NoKnownRoute,
    ObjectiveSatisfied,
    PlayerDecisionRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    Coordination(CoordinationAction),
    PlayerRequired,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryBlocker {
    pub category: RecoveryCategory,
    pub reason: RecoveryReason,
    pub action: Option<RecoveryAction>,
    pub objective: Option<crate::types::ObjectiveId>,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub missing_requirements: Vec<PreparationRequirement>,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryAlternative {
    pub category: RecoveryCategory,
    pub action: RecoveryAction,
    pub priority: CoordinationPriority,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub viable: bool,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub objective: Option<crate::types::ObjectiveId>,
    pub mission: Option<crate::types::MissionId>,
    pub attempted: Option<CoordinationStep>,
    pub status: RecoveryStatus,
    pub blocker: Option<RecoveryBlocker>,
    pub alternatives: Vec<RecoveryAlternative>,
    pub missing_requirements: Vec<PreparationRequirement>,
    pub actionable: bool,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplanationStatus {
    Actionable,
    Recoverable,
    Blocked,
    Satisfied,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplanationEvidenceKind {
    Objective,
    Route,
    Fuel,
    Integrity,
    Threat,
    Preparation,
    Knowledge,
    Alternative,
    StateChange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExplanationEvidence {
    pub kind: ExplanationEvidenceKind,
    pub description: String,
    pub objective: Option<crate::types::ObjectiveId>,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub satisfied: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplanationReason {
    ObjectiveAction,
    FuelInsufficient,
    ShipDestroyed,
    ActiveThreat,
    PreparationRequired,
    KnowledgeMissing,
    AlternativeAvailable,
    ObjectiveCompleted,
    NoKnownCause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExplainedAlternative {
    pub alternative: RecoveryAlternative,
    pub selected: bool,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecommendationExplanation {
    pub companion: EntityId,
    pub player: EntityId,
    pub objective: Option<crate::types::ObjectiveId>,
    pub mission: Option<crate::types::MissionId>,
    pub action: Option<RecoveryAction>,
    pub status: ExplanationStatus,
    pub reason: ExplanationReason,
    pub evidence: Vec<ExplanationEvidence>,
    pub satisfied_requirements: Vec<PreparationRequirement>,
    pub missing_requirements: Vec<PreparationRequirement>,
    pub blocker: Option<RecoveryBlocker>,
    pub alternatives: Vec<ExplainedAlternative>,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplanationChangeReason {
    PreparationResolved,
    ThreatResolved,
    ObjectiveCompleted,
    RecommendationChanged,
    NoIdentifiableCause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecommendationExplanationComparison {
    pub previous: Option<RecoveryAction>,
    pub current: Option<RecoveryAction>,
    pub previous_status: ExplanationStatus,
    pub current_status: ExplanationStatus,
    pub reason: ExplanationChangeReason,
    pub evidence: Vec<ExplanationEvidence>,
}

impl RecommendationExplanation {
    pub fn compare(
        previous: &Self,
        current: &Self,
    ) -> RecommendationExplanationComparison {
        let reason = if !previous.missing_requirements.is_empty()
            && current.missing_requirements.is_empty()
        {
            ExplanationChangeReason::PreparationResolved
        } else if previous.reason == ExplanationReason::ActiveThreat
            && current.reason != ExplanationReason::ActiveThreat
        {
            ExplanationChangeReason::ThreatResolved
        } else if previous.objective.is_some()
            && previous.objective != current.objective
        {
            ExplanationChangeReason::ObjectiveCompleted
        } else if previous.action != current.action || previous.status != current.status {
            ExplanationChangeReason::RecommendationChanged
        } else {
            ExplanationChangeReason::NoIdentifiableCause
        };
        let mut evidence = Vec::new();
        if reason == ExplanationChangeReason::PreparationResolved {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::StateChange,
                description: "Los requisitos que bloqueaban la preparación ya no faltan".to_string(),
                objective: current.objective,
                destination: None,
                entity: None,
                satisfied: true,
            });
        } else if reason == ExplanationChangeReason::ThreatResolved {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::StateChange,
                description: "La amenaza que justificaba la prioridad critica ya no está activa".to_string(),
                objective: current.objective,
                destination: None,
                entity: previous.blocker.as_ref().and_then(|value| value.entity),
                satisfied: true,
            });
        } else if reason == ExplanationChangeReason::ObjectiveCompleted {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::StateChange,
                description: "El objetivo anterior dejo de ser el objetivo actual".to_string(),
                objective: previous.objective,
                destination: None,
                entity: None,
                satisfied: true,
            });
        } else if reason == ExplanationChangeReason::NoIdentifiableCause {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::StateChange,
                description: "La causa exacta no puede determinarse con las evaluaciones suministradas".to_string(),
                objective: current.objective,
                destination: None,
                entity: None,
                satisfied: false,
            });
        }
        RecommendationExplanationComparison {
            previous: previous.action,
            current: current.action,
            previous_status: previous.status,
            current_status: current.status,
            reason,
            evidence,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsistencyAuditState {
    Consistent,
    Informational,
    Contradictory,
    NotEvaluable,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContradictionSeverity {
    Critical,
    Important,
    Informational,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContradictionCategory {
    PreparationNavigation,
    TacticalCoordination,
    ExplanationRecovery,
    ObjectiveGuidance,
    Availability,
    RouteViability,
    Alternative,
    InsufficientEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContradictionReason {
    CriticalPreparationConflict,
    ActiveThreatConflict,
    BlockerStateConflict,
    CompletedObjectiveStillRecommended,
    UnavailableActorActionable,
    InfeasibleRoutePresentedAsViable,
    AlternativeWithoutRegisteredRoute,
    InsufficientEvidence,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConsistencyEvidence {
    pub category: ContradictionCategory,
    pub description: String,
    pub objective: Option<crate::types::ObjectiveId>,
    pub destination: Option<DestinationId>,
    pub entity: Option<EntityId>,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Contradiction {
    pub severity: ContradictionSeverity,
    pub category: ContradictionCategory,
    pub reason: ContradictionReason,
    pub explanation: String,
    pub evidence: Vec<ConsistencyEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConsistencyAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub state: ConsistencyAuditState,
    pub coordination: Option<CoordinationAssessment>,
    pub recovery: Option<RecoveryAssessment>,
    pub explanation: Option<RecommendationExplanation>,
    pub contradictions: Vec<Contradiction>,
    pub informational: Vec<ConsistencyEvidence>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeNavigationRiskStatus {
    ViableWithKnownEvidence,
    KnownRisk,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeNavigationAction {
    StartAutopilot(DestinationId),
    MonitorAutopilot(DestinationId),
    InspectHazard(HazardId),
    AddressEncounter(EncounterId),
    ManageFuel { required: u32, available: u32 },
    RepairShip(EntityId),
    PrepareEncounter,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeNavigationRiskAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub autopilot: AutopilotState,
    pub travel: TravelState,
    pub destination: Option<DestinationId>,
    pub route: Vec<DestinationId>,
    pub status: CooperativeNavigationRiskStatus,
    pub fuel_available: Option<u32>,
    pub fuel_required: Option<u32>,
    pub integrity: Option<u32>,
    pub maximum_integrity: Option<u32>,
    pub risks: Vec<NavigationRisk>,
    pub route_risks: Vec<NavigationRouteRisk>,
    pub alternative: Option<NavigationAlternativeRecommendation>,
    pub preparation: Option<CooperativeEncounterReadinessAssessment>,
    pub actions: Vec<CooperativeNavigationAction>,
    pub blockers: Vec<String>,
    pub evidence: Vec<SupportEvidence>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeNavigationContingencyStatus {
    KnownRecovery,
    RestrictedRecovery,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CooperativeNavigationContingencyAction {
    ResumeAutopilot(DestinationId),
    StartAlternative {
        destination: DestinationId,
        route: Vec<DestinationId>,
    },
    MonitorAutopilot(DestinationId),
    InspectHazard(HazardId),
    AddressEncounter(EncounterId),
    ManageFuel { required: u32, available: u32 },
    RepairShip(EntityId),
    PrepareEncounter,
    PlayerRequired,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeNavigationContingencyAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub autopilot: AutopilotState,
    pub travel: TravelState,
    pub destination: Option<DestinationId>,
    pub current_position: Option<Vec3>,
    pub current_origin: Option<DestinationId>,
    pub status: CooperativeNavigationContingencyStatus,
    pub reason: Option<RecoveryReason>,
    pub risk: Option<CooperativeNavigationRiskAssessment>,
    pub recovery: Option<RecoveryAssessment>,
    pub alternatives: Vec<NavigationRouteOption>,
    pub recommended_route: Option<Vec<DestinationId>>,
    pub actions: Vec<CooperativeNavigationContingencyAction>,
    pub blockers: Vec<String>,
    pub evidence: Vec<SupportEvidence>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeRecoveryAssignmentStatus {
    Actionable,
    Partial,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeRecoveryTaskStatus {
    Actionable,
    Blocked,
    NotEvaluable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CooperativeRecoveryTaskAction {
    Navigation(CooperativeNavigationContingencyAction),
    TransferInventory {
        from: EntityId,
        to: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    },
    DefeatEnemy(EntityId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeRecoveryTask {
    pub order: u32,
    pub category: RecoveryCategory,
    pub action: CooperativeRecoveryTaskAction,
    pub assigned_player: Option<EntityId>,
    pub candidates: Vec<EntityId>,
    pub status: CooperativeRecoveryTaskStatus,
    pub priority: CoordinationPriority,
    pub requirements: Vec<PreparationRequirement>,
    pub missing_requirements: Vec<String>,
    pub evidence: Vec<SupportEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeRecoveryAssignmentAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub contingency: Option<CooperativeNavigationContingencyAssessment>,
    pub status: CooperativeRecoveryAssignmentStatus,
    pub tasks: Vec<CooperativeRecoveryTask>,
    pub evidence: Vec<SupportEvidence>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeRecoveryPlanStatus {
    Actionable,
    Partial,
    Blocked,
    Satisfied,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeRecoveryPlanTaskStatus {
    Ready,
    Blocked,
    Satisfied,
    NotEvaluable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeRecoveryPlanDependency {
    pub prerequisite: u32,
    pub dependent: u32,
    pub confirmed: bool,
    pub reason: String,
    pub evidence: Vec<SupportEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeRecoveryPlanTask {
    pub task: CooperativeRecoveryTask,
    pub status: CooperativeRecoveryPlanTaskStatus,
    pub dependency_orders: Vec<u32>,
    pub parallel_candidate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeRecoveryPlanAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub assignment: CooperativeRecoveryAssignmentAssessment,
    pub status: CooperativeRecoveryPlanStatus,
    pub tasks: Vec<CooperativeRecoveryPlanTask>,
    pub dependencies: Vec<CooperativeRecoveryPlanDependency>,
    pub ready_orders: Vec<u32>,
    pub parallel_orders: Vec<u32>,
    pub evidence: Vec<SupportEvidence>,
    pub conclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeTaskCompatibilityStatus {
    CompatibleKnown,
    ConflictConfirmed,
    Sequential,
    NotEvaluable,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CooperativeTaskCompatibilityReason {
    ConfirmedDependency,
    SharedPlayer,
    SharedInventory,
    IncompatibleTravelState,
    DistinctCombatTargets,
    DistinctTransfers,
    InsufficientEvidence,
    TaskNotActionable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeTaskCompatibility {
    pub first_task: u32,
    pub second_task: u32,
    pub status: CooperativeTaskCompatibilityStatus,
    pub reason: CooperativeTaskCompatibilityReason,
    pub shared_entities: Vec<EntityId>,
    pub shared_item: Option<crate::types::ItemId>,
    pub evidence: Vec<SupportEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooperativeTaskCompatibilityAssessment {
    pub companion: EntityId,
    pub player: EntityId,
    pub plan: CooperativeRecoveryPlanAssessment,
    pub status: CooperativeTaskCompatibilityStatus,
    pub relations: Vec<CooperativeTaskCompatibility>,
    pub compatible_pairs: Vec<(u32, u32)>,
    pub sequential_pairs: Vec<(u32, u32)>,
    pub conflict_pairs: Vec<(u32, u32)>,
    pub unevaluable_pairs: Vec<(u32, u32)>,
    pub conclusive: bool,
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
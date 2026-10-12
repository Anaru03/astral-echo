use engine::{Duration, EntityId, Vec3};

use crate::{
    encounter::EncounterId,
    guidance::GuidanceAction,
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
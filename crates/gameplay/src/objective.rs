use engine::EntityId;

use crate::{
    combat::EnemyArchetype,
    door::DoorRole,
    interaction::{ConsoleRole, ConsoleState},
    knowledge::KnowledgeTarget,
    navigation::DestinationId,
    station::{StationState, StationType},
    types::{ItemId, MissionId, ObjectiveId},
};

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectiveTrigger {
    Manual,
    InteractionSucceeded {
        target: Option<EntityId>,
    },
    ConsoleStateChanged {
        console: EntityId,
        state: ConsoleState,
    },
    DoorOpened {
        door: Option<EntityId>,
    },
    StationOccupied {
        station: Option<EntityId>,
    },
    ShipStatusEvaluated {
        role: ConsoleRole,
    },
    ElectricalSystemRepaired,
    OxygenSystemRepaired,
    DoorRoleOpened {
        role: DoorRole,
    },
    StationTypeOccupied {
        station_type: StationType,
    },
    DestinationArrived {
        destination: DestinationId,
    },
    PlanetDiscovered {
        destination: DestinationId,
    },
    PlanetsDiscovered {
        count: u32,
    },
    EnemyDefeated {
        enemy: Option<engine::EntityId>,
    },
    EnemiesDefeated {
        archetype: Option<EnemyArchetype>,
        count: u32,
    },
    ShieldDisabled {
        enemy: Option<engine::EntityId>,
    },
    ItemsCollected {
        item: ItemId,
        amount: u32,
    },
    CollectibleCollected {
        collectible: engine::EntityId,
    },
    FuelConsumed {
        destination: Option<DestinationId>,
        amount: u32,
    },
    SignalDetected {
        destination: Option<DestinationId>,
    },
    DestinationIdentified {
        destination: Option<DestinationId>,
    },
    SignalsDetected {
        count: u32,
    },
    KnowledgeDetected {
        target: Option<KnowledgeTarget>,
    },
    KnowledgeIdentified {
        target: Option<KnowledgeTarget>,
    },
    KnowledgeDiscovered {
        target: Option<KnowledgeTarget>,
    },
    KnowledgeStudied {
        target: Option<KnowledgeTarget>,
    },
    KnowledgeDetectedCount {
        count: u32,
    },
    KnowledgeIdentifiedCount {
        count: u32,
    },
    KnowledgeStudiedCount {
        count: u32,
    },
}

impl ObjectiveTrigger {
    pub fn required_progress(&self) -> u32 {
        match self {
            Self::PlanetsDiscovered { count } | Self::EnemiesDefeated { count, .. } => *count,
            Self::ItemsCollected { amount, .. }
            | Self::FuelConsumed { amount, .. }
            | Self::SignalsDetected { count: amount }
            | Self::KnowledgeDetectedCount { count: amount }
            | Self::KnowledgeIdentifiedCount { count: amount }
            | Self::KnowledgeStudiedCount { count: amount } => *amount,
            _ => 1,
        }
    }

    pub fn validate(&self) -> Result<(), ObjectiveTriggerError> {
        if self.required_progress() == 0 {
            return Err(ObjectiveTriggerError::InvalidThreshold);
        }
        Ok(())
    }

    pub fn progress_for_event(&self, event: &crate::events::GameplayEvent) -> Option<u32> {
        let matches = match (self, event) {
            (
                Self::DestinationArrived { destination },
                crate::events::GameplayEvent::TravelArrived {
                    destination: actual,
                    ..
                },
            ) => destination == actual,
            (
                Self::PlanetDiscovered { destination },
                crate::events::GameplayEvent::PlanetDiscovered {
                    destination: actual,
                },
            ) => destination == actual,
            (
                Self::PlanetsDiscovered { .. },
                crate::events::GameplayEvent::PlanetDiscovered { .. },
            ) => true,
            (
                Self::EnemyDefeated { enemy: expected },
                crate::events::GameplayEvent::EnemyDefeated { enemy, .. },
            ) => expected.is_none_or(|value| value == *enemy),
            (
                Self::EnemiesDefeated {
                    archetype: expected,
                    ..
                },
                crate::events::GameplayEvent::EnemyDefeated { archetype, .. },
            ) => expected.is_none_or(|value| value == *archetype),
            (
                Self::ShieldDisabled { enemy: expected },
                crate::events::GameplayEvent::ShieldDisabled { enemy, .. },
            ) => expected.is_none_or(|value| value == *enemy),
            (
                Self::ItemsCollected { item, .. },
                crate::events::GameplayEvent::ItemCollected { item: actual, .. },
            ) => item == actual,
            (
                Self::CollectibleCollected { collectible },
                crate::events::GameplayEvent::ItemCollected {
                    collectible: actual,
                    ..
                },
            ) => collectible == actual,
            (
                Self::FuelConsumed {
                    destination: expected,
                    ..
                },
                crate::events::GameplayEvent::FuelConsumed { destination, .. },
            ) => expected.is_none_or(|value| value == *destination),
            (
                Self::SignalDetected {
                    destination: expected,
                },
                crate::events::GameplayEvent::SignalDetected { destination, .. },
            ) => expected.is_none_or(|value| value == *destination),
            (
                Self::DestinationIdentified {
                    destination: expected,
                },
                crate::events::GameplayEvent::DestinationIdentified { destination, .. },
            ) => expected.is_none_or(|value| value == *destination),
            (Self::SignalsDetected { .. }, crate::events::GameplayEvent::SignalDetected { .. }) => {
                true
            }
            (
                Self::KnowledgeDetected { target: expected },
                crate::events::GameplayEvent::KnowledgeDetected { target: actual },
            ) => expected.is_none_or(|value| value == *actual),
            (
                Self::KnowledgeIdentified { target: expected },
                crate::events::GameplayEvent::KnowledgeIdentified { target: actual },
            ) => expected.is_none_or(|value| value == *actual),
            (
                Self::KnowledgeDiscovered { target: expected },
                crate::events::GameplayEvent::KnowledgeDiscovered { target: actual },
            ) => expected.is_none_or(|value| value == *actual),
            (
                Self::KnowledgeStudied { target: expected },
                crate::events::GameplayEvent::KnowledgeStudied { target: actual, .. },
            ) => expected.is_none_or(|value| value == *actual),
            (
                Self::KnowledgeDetectedCount { .. },
                crate::events::GameplayEvent::KnowledgeDetected { .. },
            ) => true,
            (
                Self::KnowledgeIdentifiedCount { .. },
                crate::events::GameplayEvent::KnowledgeIdentified { .. },
            ) => true,
            (
                Self::KnowledgeStudiedCount { .. },
                crate::events::GameplayEvent::KnowledgeStudied { .. },
            ) => true,
            _ => self.matches_point_event(event),
        };
        if !matches {
            return None;
        }
        let amount = match (self, event) {
            (
                Self::ItemsCollected { .. },
                crate::events::GameplayEvent::ItemCollected { amount, .. },
            ) => *amount,
            (
                Self::FuelConsumed { .. },
                crate::events::GameplayEvent::FuelConsumed { amount, .. },
            ) => *amount,
            _ => 1,
        };
        Some(amount)
    }

    pub fn matches(&self, event: &crate::events::GameplayEvent) -> bool {
        self.progress_for_event(event).is_some()
    }

    fn matches_point_event(&self, event: &crate::events::GameplayEvent) -> bool {
        match (self, event) {
            (Self::Manual, _) => false,
            (
                Self::InteractionSucceeded { target: expected },
                crate::events::GameplayEvent::InteractionSucceeded { target, .. },
            ) => expected.is_none_or(|value| value == *target),
            (
                Self::ConsoleStateChanged { console, state },
                crate::events::GameplayEvent::ConsoleStateChanged {
                    console: actual,
                    state: actual_state,
                    ..
                },
            ) => console == actual && state == actual_state,
            (
                Self::DoorOpened { door: expected },
                crate::events::GameplayEvent::DoorOpened { door, .. },
            ) => expected.is_none_or(|value| value == *door),
            (
                Self::StationOccupied { station: expected },
                crate::events::GameplayEvent::StationOccupied { station, state, .. },
            ) => *state == StationState::Occupied && expected.is_none_or(|value| value == *station),
            (
                Self::ShipStatusEvaluated { role },
                crate::events::GameplayEvent::ShipStatusEvaluated { role: actual, .. },
            ) => role == actual,
            (
                Self::ElectricalSystemRepaired,
                crate::events::GameplayEvent::ElectricalSystemRepaired { .. },
            ) => true,
            (
                Self::OxygenSystemRepaired,
                crate::events::GameplayEvent::OxygenSystemRepaired { .. },
            ) => true,
            (
                Self::DoorRoleOpened { role },
                crate::events::GameplayEvent::DoorOpened { role: actual, .. },
            ) => role == actual,
            (
                Self::StationTypeOccupied { station_type },
                crate::events::GameplayEvent::StationOccupied {
                    station_type: actual,
                    state,
                    ..
                },
            ) => *state == StationState::Occupied && station_type == actual,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveTriggerError {
    InvalidThreshold,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Objective {
    pub id: ObjectiveId,
    pub mission_id: MissionId,
    pub name: String,
    pub required: bool,
    pub dependencies: Vec<ObjectiveId>,
    pub trigger: ObjectiveTrigger,
    state: crate::domains::objective::ObjectiveState,
    progress: u32,
}

impl Objective {
    pub fn new(
        id: ObjectiveId,
        mission_id: MissionId,
        name: impl Into<String>,
        required: bool,
        trigger: ObjectiveTrigger,
    ) -> Self {
        Self {
            id,
            mission_id,
            name: name.into(),
            required,
            dependencies: Vec::new(),
            trigger,
            state: crate::domains::objective::ObjectiveState::Locked,
            progress: 0,
        }
    }

    pub fn with_dependencies(
        mut self,
        dependencies: impl IntoIterator<Item = ObjectiveId>,
    ) -> Self {
        self.dependencies = dependencies.into_iter().collect();
        self
    }

    pub fn state(&self) -> crate::domains::objective::ObjectiveState {
        self.state
    }
    pub const fn progress(&self) -> u32 {
        self.progress
    }
    pub fn required_progress(&self) -> u32 {
        self.trigger.required_progress()
    }
    pub const fn is_optional(&self) -> bool {
        !self.required
    }
    pub(crate) fn set_state(&mut self, state: crate::domains::objective::ObjectiveState) {
        self.state = state;
    }
    pub(crate) fn add_progress(&mut self, amount: u32) -> bool {
        let required = self.required_progress();
        let next = self.progress.saturating_add(amount).min(required);
        let changed = next != self.progress;
        self.progress = next;
        changed
    }
}
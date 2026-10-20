use engine::EntityId;

use crate::{
    domains::{mission::MissionState, objective::ObjectiveState},
    knowledge::{KnowledgeState, KnowledgeTarget},
    interaction::InteractionBehavior,
    mission::Mission,
    navigation::DestinationId,
    objective::{Objective, ObjectiveTrigger},
    types::{ItemId, MissionId, ObjectiveId},
    world::GameplayWorld,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidanceScope {
    Shared,
    Player(EntityId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidanceStatus {
    Locked,
    Available,
    Active,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidanceRequirement {
    MissionActive(MissionId),
    MissionCompleted(MissionId),
    ObjectiveCompleted(ObjectiveId),
    KnowledgeAtLeast {
        target: KnowledgeTarget,
        state: KnowledgeState,
    },
    DestinationDiscovered(DestinationId),
    DestinationArrived(DestinationId),
    EnemyDefeated(Option<EntityId>),
    ResourceObtained {
        player: EntityId,
        item: ItemId,
        amount: u32,
    },
    WorldActivityCompleted(DestinationId),
    DerelictResolved(crate::derelict::DerelictId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidanceAction {
    UnlockMission(MissionId),
    StartMission(MissionId),
    TravelTo(DestinationId),
    IdentifySignal(Option<DestinationId>),
    DiscoverDestination(DestinationId),
    DefeatEnemy(Option<EntityId>),
    CollectItem { item: ItemId, amount: u32 },
    StudyKnowledge(KnowledgeTarget),
    RepairShip,
    Interact(EntityId),
    CompleteObjective(ObjectiveId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidanceObjective {
    pub id: ObjectiveId,
    pub mission: MissionId,
    pub name: String,
    pub required: bool,
    pub scope: GuidanceScope,
    pub status: GuidanceStatus,
    pub progress: u32,
    pub required_progress: u32,
    pub missing_requirements: Vec<GuidanceRequirement>,
    pub recommended_action: Option<GuidanceAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidanceState {
    pub player: EntityId,
    pub objectives: Vec<GuidanceObjective>,
    pub current_objective: Option<ObjectiveId>,
    pub recommended_action: Option<GuidanceAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidanceError {
    PlayerNotFound,
    PlayerInactive,
}

pub struct GuidanceSystem;

impl GuidanceSystem {
    pub fn query(world: &GameplayWorld, player: EntityId) -> Result<GuidanceState, GuidanceError> {
        let Some(player_state) = world.player(player) else {
            return Err(GuidanceError::PlayerNotFound);
        };
        if !player_state.is_active {
            return Err(GuidanceError::PlayerInactive);
        }

        let mut objectives = Vec::new();
        for mission in world.missions() {
            for objective in mission.objectives() {
                objectives.push(Self::objective(world, player, mission, objective));
            }
        }

        let current_objective = objectives
            .iter()
            .find(|objective| objective.status == GuidanceStatus::Active)
            .or_else(|| {
                objectives
                    .iter()
                    .find(|objective| objective.status == GuidanceStatus::Available)
            })
            .map(|objective| objective.id);
        let recommended_action = current_objective.and_then(|id| {
            objectives
                .iter()
                .find(|objective| objective.id == id)
                .and_then(|objective| objective.recommended_action)
        });

        Ok(GuidanceState {
            player,
            objectives,
            current_objective,
            recommended_action,
        })
    }

    fn objective(
        world: &GameplayWorld,
        player: EntityId,
        mission: &Mission,
        objective: &Objective,
    ) -> GuidanceObjective {
        let status = match objective.state() {
            ObjectiveState::Locked => GuidanceStatus::Locked,
            ObjectiveState::Available => GuidanceStatus::Available,
            ObjectiveState::Active => GuidanceStatus::Active,
            ObjectiveState::Completed => GuidanceStatus::Completed,
            ObjectiveState::Failed => GuidanceStatus::Locked,
        };
        let status = if status == GuidanceStatus::Locked
            && mission.state() == MissionState::Available
            && objective.dependencies.is_empty()
        {
            GuidanceStatus::Available
        } else {
            status
        };
        let mut missing_requirements = objective
            .dependencies
            .iter()
            .filter(|dependency| {
                mission
                    .objectives()
                    .iter()
                    .find(|candidate| candidate.id == **dependency)
                    .is_none_or(|candidate| candidate.state() != ObjectiveState::Completed)
            })
            .map(|dependency| GuidanceRequirement::ObjectiveCompleted(*dependency))
            .collect::<Vec<_>>();

        if mission.state() == MissionState::Locked || mission.state() == MissionState::Available {
            missing_requirements.push(GuidanceRequirement::MissionActive(mission.id));
        }
        if status != GuidanceStatus::Completed {
            if let Some(requirement) = Self::trigger_requirement(&objective.trigger, player) {
                if !Self::requirement_satisfied(world, requirement) {
                    missing_requirements.push(requirement);
                }
            }
        }

        let recommended_action = (status == GuidanceStatus::Available
            || status == GuidanceStatus::Active)
            .then(|| Self::action_for(world, mission, objective));
        GuidanceObjective {
            id: objective.id,
            mission: mission.id,
            name: objective.name.clone(),
            required: objective.required,
            scope: GuidanceScope::Shared,
            status,
            progress: objective.progress(),
            required_progress: objective.required_progress(),
            missing_requirements,
            recommended_action,
        }
    }

    fn trigger_requirement(
        trigger: &ObjectiveTrigger,
        player: EntityId,
    ) -> Option<GuidanceRequirement> {
        Some(match trigger {
            ObjectiveTrigger::DestinationArrived { destination } => {
                GuidanceRequirement::DestinationArrived(*destination)
            }
            ObjectiveTrigger::PlanetDiscovered { destination } => {
                GuidanceRequirement::DestinationDiscovered(*destination)
            }
            ObjectiveTrigger::DestinationIdentified { destination } => {
                GuidanceRequirement::KnowledgeAtLeast {
                    target: KnowledgeTarget::Destination((*destination)?),
                    state: KnowledgeState::Identified,
                }
            }
            ObjectiveTrigger::EnemyDefeated { enemy } => GuidanceRequirement::EnemyDefeated(*enemy),
            ObjectiveTrigger::EnemiesDefeated { .. } => GuidanceRequirement::EnemyDefeated(None),
            ObjectiveTrigger::ItemsCollected { item, amount } => {
                GuidanceRequirement::ResourceObtained {
                    player,
                    item: *item,
                    amount: *amount,
                }
            }
            ObjectiveTrigger::SignalDetected { destination } => {
                GuidanceRequirement::KnowledgeAtLeast {
                    target: KnowledgeTarget::Destination((*destination)?),
                    state: KnowledgeState::Detected,
                }
            }
            ObjectiveTrigger::KnowledgeDetected { target }
            | ObjectiveTrigger::KnowledgeIdentified { target }
            | ObjectiveTrigger::KnowledgeDiscovered { target }
            | ObjectiveTrigger::KnowledgeStudied { target } => GuidanceRequirement::KnowledgeAtLeast {
                target: (*target)?,
                state: match trigger {
                    ObjectiveTrigger::KnowledgeDetected { .. } => KnowledgeState::Detected,
                    ObjectiveTrigger::KnowledgeIdentified { .. } => KnowledgeState::Identified,
                    ObjectiveTrigger::KnowledgeDiscovered { .. } => KnowledgeState::Discovered,
                    _ => KnowledgeState::Studied,
                },
            },
            ObjectiveTrigger::ShipDamagedByHazard { .. }
            | ObjectiveTrigger::ShipRepaired { .. }
            | ObjectiveTrigger::FuelConsumed { .. }
            | ObjectiveTrigger::InteractionSucceeded { .. }
            | ObjectiveTrigger::ConsoleStateChanged { .. }
            | ObjectiveTrigger::DoorOpened { .. }
            | ObjectiveTrigger::StationOccupied { .. }
            | ObjectiveTrigger::ShipStatusEvaluated { .. }
            | ObjectiveTrigger::ElectricalSystemRepaired
            | ObjectiveTrigger::OxygenSystemRepaired
            | ObjectiveTrigger::DoorRoleOpened { .. }
            | ObjectiveTrigger::StationTypeOccupied { .. }
            | ObjectiveTrigger::PlanetsDiscovered { .. }
            | ObjectiveTrigger::ShieldDisabled { .. }
            | ObjectiveTrigger::ShieldDamaged { .. }
            | ObjectiveTrigger::SignalsDetected { .. }
            | ObjectiveTrigger::KnowledgeDetectedCount { .. }
            | ObjectiveTrigger::KnowledgeIdentifiedCount { .. }
            | ObjectiveTrigger::KnowledgeStudiedCount { .. }
            | ObjectiveTrigger::CollectibleCollected { .. }
            | ObjectiveTrigger::Manual => return None,
        })
    }

    fn action_for(
        world: &GameplayWorld,
        mission: &Mission,
        objective: &Objective,
    ) -> GuidanceAction {
        if mission.state() == MissionState::Locked {
            return GuidanceAction::UnlockMission(mission.id);
        }
        if mission.state() == MissionState::Available {
            return GuidanceAction::StartMission(mission.id);
        }
        match objective.trigger {
            ObjectiveTrigger::ShipStatusEvaluated { role } => world
                .interactables()
                .find_map(|interactable| match interactable.behavior {
                    InteractionBehavior::Console { role: current, .. } if current == role => {
                        Some(GuidanceAction::Interact(interactable.entity_id))
                    }
                    _ => None,
                })
                .unwrap_or(GuidanceAction::CompleteObjective(objective.id)),
            ObjectiveTrigger::ElectricalSystemRepaired => world
                .interactables()
                .find(|interactable| {
                    matches!(interactable.behavior, InteractionBehavior::ElectricalPanel)
                })
                .map_or(GuidanceAction::CompleteObjective(objective.id), |interactable| {
                    GuidanceAction::Interact(interactable.entity_id)
                }),
            ObjectiveTrigger::OxygenSystemRepaired => world
                .interactables()
                .find(|interactable| {
                    matches!(interactable.behavior, InteractionBehavior::LifeSupportPanel)
                })
                .map_or(GuidanceAction::CompleteObjective(objective.id), |interactable| {
                    GuidanceAction::Interact(interactable.entity_id)
                }),
            ObjectiveTrigger::DoorRoleOpened { role } => world
                .interactables()
                .find_map(|interactable| match interactable.behavior {
                    InteractionBehavior::Door { door } if door.role == role => {
                        Some(GuidanceAction::Interact(interactable.entity_id))
                    }
                    _ => None,
                })
                .unwrap_or(GuidanceAction::CompleteObjective(objective.id)),
            ObjectiveTrigger::StationTypeOccupied { station_type } => world
                .interactables()
                .find_map(|interactable| match interactable.behavior {
                    InteractionBehavior::Station { station }
                        if station.station_type == station_type =>
                    {
                        Some(GuidanceAction::Interact(interactable.entity_id))
                    }
                    _ => None,
                })
                .unwrap_or(GuidanceAction::CompleteObjective(objective.id)),
            ObjectiveTrigger::DestinationArrived { destination } => {
                GuidanceAction::TravelTo(destination)
            }
            ObjectiveTrigger::PlanetDiscovered { destination } => {
                GuidanceAction::DiscoverDestination(destination)
            }
            ObjectiveTrigger::DestinationIdentified { destination } => {
                GuidanceAction::IdentifySignal(destination)
            }
            ObjectiveTrigger::SignalDetected { destination } => {
                GuidanceAction::IdentifySignal(destination)
            }
            ObjectiveTrigger::EnemyDefeated { enemy } => GuidanceAction::DefeatEnemy(enemy),
            ObjectiveTrigger::EnemiesDefeated { .. } => GuidanceAction::DefeatEnemy(
                world
                    .enemies()
                    .find(|enemy| enemy.active && !enemy.is_defeated())
                    .map(|enemy| enemy.entity),
            ),
            ObjectiveTrigger::ItemsCollected { item, amount } => {
                GuidanceAction::CollectItem { item, amount }
            }
            ObjectiveTrigger::KnowledgeDetected { target }
            | ObjectiveTrigger::KnowledgeIdentified { target }
            | ObjectiveTrigger::KnowledgeDiscovered { target }
            | ObjectiveTrigger::KnowledgeStudied { target } => {
                target.map_or(GuidanceAction::StudyKnowledge(KnowledgeTarget::Item(ItemId(1))), GuidanceAction::StudyKnowledge)
            }
            ObjectiveTrigger::ShipRepaired { .. } => GuidanceAction::RepairShip,
            ObjectiveTrigger::InteractionSucceeded { target }
            | ObjectiveTrigger::DoorOpened { door: target }
            | ObjectiveTrigger::StationOccupied { station: target } => {
                target.map_or(GuidanceAction::CompleteObjective(objective.id), GuidanceAction::Interact)
            }
            ObjectiveTrigger::ConsoleStateChanged { console, .. } => {
                GuidanceAction::Interact(console)
            }
            _ => GuidanceAction::CompleteObjective(objective.id),
        }
    }

    fn requirement_satisfied(world: &GameplayWorld, requirement: GuidanceRequirement) -> bool {
        match requirement {
            GuidanceRequirement::MissionActive(id) => {
                world.mission(id).is_some_and(|mission| mission.state() == MissionState::Active)
            }
            GuidanceRequirement::MissionCompleted(id) => {
                world.mission(id).is_some_and(|mission| mission.state() == MissionState::Completed)
            }
            GuidanceRequirement::ObjectiveCompleted(id) => {
                world.objective(id).is_some_and(|objective| objective.state() == ObjectiveState::Completed)
            }
            GuidanceRequirement::KnowledgeAtLeast { target, state } => world.knowledge(target) >= state,
            GuidanceRequirement::DestinationDiscovered(destination) => world
                .destination(destination)
                .is_some_and(|value| value.discovery == crate::navigation::DiscoveryState::Discovered),
            GuidanceRequirement::DestinationArrived(destination) => world
                .destination(destination)
                .is_some_and(|value| world.navigation().current_position() == value.position),
            GuidanceRequirement::EnemyDefeated(expected) => world.enemies().any(|enemy| {
                !enemy.active
                    && enemy.is_defeated()
                    && expected.is_none_or(|entity| enemy.entity == entity)
            }),
            GuidanceRequirement::ResourceObtained { item, amount, .. } => world
                .players()
                .any(|candidate| world.inventory(candidate.entity_id).is_some_and(|inventory| {
                    inventory.quantity(item) >= amount
                })),
            GuidanceRequirement::WorldActivityCompleted(destination) => world
                .functional_world(destination)
                .is_some_and(|world| world.activities.iter().any(|activity| {
                    matches!(activity, crate::world_activity::WorldActivity::Combat(zone) if zone.resolved)
                        || matches!(activity, crate::world_activity::WorldActivity::Story(site) if site.interacted)
                })),
            GuidanceRequirement::DerelictResolved(id) => world
                .derelict(id)
                .is_some_and(|derelict| derelict.state == crate::derelict::DerelictState::Resolved),
        }
    }
}
use std::collections::BTreeMap;

use crate::{
    commands::{CommandResult, CommandStatus, GameplayCommand},
    domains::{mission::MissionState, objective::ObjectiveState},
    events::GameplayEvent,
    systems::GameplaySystem,
    types::{MissionId, ObjectiveId},
    world::GameplayWorld,
};

pub struct MissionSystem {
    event_cursor: usize,
}

impl MissionSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn unlock_mission(&mut self, world: &mut GameplayWorld, id: MissionId) -> CommandResult {
        let Some(mission) = world.mission_mut(id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if !mission.unlock() {
            return reject(crate::commands::CommandRejection::InvalidMissionTransition);
        }
        world.publish(GameplayEvent::MissionUnlocked {
            mission: id,
            state: MissionState::Available,
        });
        CommandResult::success(true)
    }

    fn start_mission(&mut self, world: &mut GameplayWorld, id: MissionId) -> CommandResult {
        let Some(mission) = world.mission_mut(id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if !mission.start() {
            return reject(crate::commands::CommandRejection::InvalidMissionTransition);
        }
        let available = mission.activate_available_objectives();
        let mut events = vec![GameplayEvent::MissionStarted {
            mission: id,
            state: MissionState::Active,
        }];
        events.extend(
            available
                .into_iter()
                .map(|objective| GameplayEvent::ObjectiveAvailable {
                    objective,
                    state: ObjectiveState::Available,
                }),
        );
        for event in events {
            world.publish(event);
        }
        CommandResult::success(true)
    }

    fn fail_mission(&mut self, world: &mut GameplayWorld, id: MissionId) -> CommandResult {
        let Some(mission) = world.mission_mut(id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if !mission.fail() {
            return reject(crate::commands::CommandRejection::InvalidMissionTransition);
        }
        let failed: Vec<ObjectiveId> = mission
            .objectives_mut()
            .iter_mut()
            .filter_map(|objective| {
                if objective.state() == ObjectiveState::Active {
                    objective.set_state(ObjectiveState::Failed);
                    Some(objective.id)
                } else {
                    None
                }
            })
            .collect();
        for objective in failed {
            world.publish(GameplayEvent::ObjectiveFailed {
                objective,
                state: ObjectiveState::Failed,
            });
        }
        world.publish(GameplayEvent::MissionFailed {
            mission: id,
            state: MissionState::Failed,
        });
        CommandResult::success(true)
    }

    fn complete_mission(&mut self, world: &mut GameplayWorld, id: MissionId) -> CommandResult {
        let Some(mission) = world.mission_mut(id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if mission.state() != MissionState::Active {
            return reject(crate::commands::CommandRejection::InvalidMissionTransition);
        }
        if mission.progress().required_completed != mission.progress().required_total {
            return reject(crate::commands::CommandRejection::RequiredObjectivesIncomplete);
        }
        mission.complete();
        world.publish(GameplayEvent::MissionCompleted {
            mission: id,
            state: MissionState::Completed,
        });
        CommandResult::success(true)
    }

    fn start_objective(&mut self, world: &mut GameplayWorld, id: ObjectiveId) -> CommandResult {
        let Some(mission_id) = world.objective(id).map(|objective| objective.mission_id) else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        let Some(mission) = world.mission_mut(mission_id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if mission.state() != MissionState::Active {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        let Some(objective) = mission
            .objectives_mut()
            .iter_mut()
            .find(|objective| objective.id == id)
        else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        if objective.state() != ObjectiveState::Available {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        objective.set_state(ObjectiveState::Active);
        world.publish(GameplayEvent::ObjectiveStarted {
            objective: id,
            state: ObjectiveState::Active,
        });
        CommandResult::success(true)
    }

    fn complete_objective(&mut self, world: &mut GameplayWorld, id: ObjectiveId) -> CommandResult {
        let Some(mission_id) = world.objective(id).map(|objective| objective.mission_id) else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        let Some(mission) = world.mission_mut(mission_id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if mission.state() != MissionState::Active {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        let Some(objective) = mission
            .objectives_mut()
            .iter_mut()
            .find(|objective| objective.id == id)
        else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        if objective.state() != ObjectiveState::Active {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        objective.set_state(ObjectiveState::Completed);
        let available = mission.activate_available_objectives();
        let mission_completed =
            mission.progress().required_completed == mission.progress().required_total;
        if mission_completed {
            mission.complete();
        }
        let mut events = vec![GameplayEvent::ObjectiveCompleted {
            objective: id,
            state: ObjectiveState::Completed,
        }];
        events.extend(
            available
                .into_iter()
                .map(|objective| GameplayEvent::ObjectiveAvailable {
                    objective,
                    state: ObjectiveState::Available,
                }),
        );
        if mission_completed {
            events.push(GameplayEvent::MissionCompleted {
                mission: mission_id,
                state: MissionState::Completed,
            });
        }
        for event in events {
            world.publish(event);
        }
        CommandResult::success(true)
    }

    fn fail_objective(&mut self, world: &mut GameplayWorld, id: ObjectiveId) -> CommandResult {
        let Some(mission_id) = world.objective(id).map(|objective| objective.mission_id) else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        let Some(mission) = world.mission_mut(mission_id) else {
            return reject(crate::commands::CommandRejection::MissionNotFound);
        };
        if mission.state() != MissionState::Active {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        let Some(objective) = mission
            .objectives_mut()
            .iter_mut()
            .find(|objective| objective.id == id)
        else {
            return reject(crate::commands::CommandRejection::ObjectiveNotFound);
        };
        if objective.state() != ObjectiveState::Active {
            return reject(crate::commands::CommandRejection::InvalidObjectiveTransition);
        }
        objective.set_state(ObjectiveState::Failed);
        world.publish(GameplayEvent::ObjectiveFailed {
            objective: id,
            state: ObjectiveState::Failed,
        });
        CommandResult::success(true)
    }

    fn advance_from_event(&mut self, world: &mut GameplayWorld, id: ObjectiveId, amount: u32) {
        let Some(mission_id) = world.objective(id).map(|objective| objective.mission_id) else {
            return;
        };
        let mut started = false;
        let available;
        let mission_completed;
        {
            let Some(mission) = world.mission_mut(mission_id) else {
                return;
            };
            if mission.state() != MissionState::Active {
                return;
            }
            let Some(objective) = mission
                .objectives_mut()
                .iter_mut()
                .find(|objective| objective.id == id)
            else {
                return;
            };
            if objective.state() == ObjectiveState::Available {
                objective.set_state(ObjectiveState::Active);
                started = true;
            }
            if objective.state() != ObjectiveState::Active {
                return;
            }
            if !objective.add_progress(amount) {
                return;
            }
            if objective.progress() < objective.required_progress() {
                if started {
                    world.publish(GameplayEvent::ObjectiveStarted {
                        objective: id,
                        state: ObjectiveState::Active,
                    });
                }
                return;
            }
            objective.set_state(ObjectiveState::Completed);
            available = mission.activate_available_objectives();
            mission_completed =
                mission.progress().required_completed == mission.progress().required_total;
            if mission_completed {
                mission.complete();
            }
        }
        if started {
            world.publish(GameplayEvent::ObjectiveStarted {
                objective: id,
                state: ObjectiveState::Active,
            });
        }
        world.publish(GameplayEvent::ObjectiveCompleted {
            objective: id,
            state: ObjectiveState::Completed,
        });
        for objective in available {
            world.publish(GameplayEvent::ObjectiveAvailable {
                objective,
                state: ObjectiveState::Available,
            });
        }
        if mission_completed {
            world.publish(GameplayEvent::MissionCompleted {
                mission: mission_id,
                state: MissionState::Completed,
            });
        }
    }

    fn observe_new_events(&mut self, world: &mut GameplayWorld) {
        let length = world.events().len();
        if self.event_cursor > length {
            self.event_cursor = 0;
        }
        let events: Vec<_> = world
            .events()
            .iter()
            .skip(self.event_cursor)
            .cloned()
            .collect();
        self.event_cursor = length;
        let targets: BTreeMap<ObjectiveId, u32> = events
            .iter()
            .flat_map(|event| {
                world.missions().flat_map(move |mission| {
                    mission.objectives().iter().filter_map(move |objective| {
                        objective
                            .trigger
                            .progress_for_event(event)
                            .map(|amount| (objective.id, amount))
                    })
                })
            })
            .fold(BTreeMap::new(), |mut progress, (objective, amount)| {
                progress
                    .entry(objective)
                    .and_modify(|total| *total = total.saturating_add(amount))
                    .or_insert(amount);
                progress
            });
        for (objective, amount) in targets {
            self.advance_from_event(world, objective, amount);
        }
    }
}

impl Default for MissionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplaySystem for MissionSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::UnlockMission { mission } => self.unlock_mission(world, mission),
            GameplayCommand::StartMission { mission } => self.start_mission(world, mission),
            GameplayCommand::FailMission { mission } => self.fail_mission(world, mission),
            GameplayCommand::CompleteMission { mission } => self.complete_mission(world, mission),
            GameplayCommand::StartObjective { objective } => self.start_objective(world, objective),
            GameplayCommand::CompleteObjective { objective } => {
                self.complete_objective(world, objective)
            }
            GameplayCommand::FailObjective { objective } => self.fail_objective(world, objective),
            _ => return None,
        };
        Some(result)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn reset_event_cursor(&mut self) {
        self.event_cursor = 0;
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        self.observe_new_events(world);
    }
}

fn reject(reason: crate::commands::CommandRejection) -> CommandResult {
    CommandResult {
        status: CommandStatus::Rejected { reason },
    }
}
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    domains::{mission::MissionState, objective::ObjectiveState},
    objective::Objective,
    types::{MissionId, ObjectiveId},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Mission {
    pub id: MissionId,
    pub name: String,
    pub description: Option<String>,
    state: MissionState,
    objectives: Vec<Objective>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionDefinitionError {
    DuplicateObjective,
    ObjectiveMissionMismatch,
    UnknownDependency,
    CircularDependency,
    InvalidTrigger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionRegistrationError {
    Definition(MissionDefinitionError),
    DuplicateMission,
    DuplicateObjective,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionProgress {
    pub state: MissionState,
    pub required_completed: usize,
    pub required_total: usize,
    pub optional_completed: usize,
    pub optional_total: usize,
}

impl Mission {
    pub fn new(
        id: MissionId,
        name: impl Into<String>,
        objectives: Vec<Objective>,
    ) -> Result<Self, MissionDefinitionError> {
        let mission = Self {
            id,
            name: name.into(),
            description: None,
            state: MissionState::Locked,
            objectives,
        };
        mission.validate_for_registration()?;
        Ok(mission)
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn state(&self) -> MissionState {
        self.state
    }
    pub fn objectives(&self) -> &[Objective] {
        &self.objectives
    }
    pub(crate) fn objectives_mut(&mut self) -> &mut [Objective] {
        &mut self.objectives
    }

    pub fn progress(&self) -> MissionProgress {
        let required_total = self
            .objectives
            .iter()
            .filter(|objective| objective.required)
            .count();
        let required_completed = self
            .objectives
            .iter()
            .filter(|objective| {
                objective.required && objective.state() == ObjectiveState::Completed
            })
            .count();
        let optional_total = self
            .objectives
            .iter()
            .filter(|objective| !objective.required)
            .count();
        let optional_completed = self
            .objectives
            .iter()
            .filter(|objective| {
                !objective.required && objective.state() == ObjectiveState::Completed
            })
            .count();
        MissionProgress {
            state: self.state,
            required_completed,
            required_total,
            optional_completed,
            optional_total,
        }
    }

    pub(crate) fn unlock(&mut self) -> bool {
        if self.state == MissionState::Locked {
            self.state = MissionState::Available;
            true
        } else {
            false
        }
    }

    pub(crate) fn start(&mut self) -> bool {
        if self.state == MissionState::Available {
            self.state = MissionState::Active;
            true
        } else {
            false
        }
    }

    pub(crate) fn fail(&mut self) -> bool {
        if self.state == MissionState::Active {
            self.state = MissionState::Failed;
            true
        } else {
            false
        }
    }

    pub(crate) fn complete(&mut self) -> bool {
        if self.state == MissionState::Active
            && self.progress().required_completed == self.progress().required_total
        {
            self.state = MissionState::Completed;
            true
        } else {
            false
        }
    }

    pub(crate) fn activate_available_objectives(&mut self) -> Vec<ObjectiveId> {
        let completed: BTreeSet<ObjectiveId> = self
            .objectives
            .iter()
            .filter(|objective| objective.state() == ObjectiveState::Completed)
            .map(|objective| objective.id)
            .collect();
        let mut available = Vec::new();
        for objective in &mut self.objectives {
            if objective.state() == ObjectiveState::Locked
                && objective
                    .dependencies
                    .iter()
                    .all(|dependency| completed.contains(dependency))
            {
                objective.set_state(ObjectiveState::Available);
                available.push(objective.id);
            }
        }
        available
    }

    pub(crate) fn validate_for_registration(&self) -> Result<(), MissionDefinitionError> {
        let mut objectives = BTreeMap::new();
        for objective in &self.objectives {
            if objective.trigger.validate().is_err() {
                return Err(MissionDefinitionError::InvalidTrigger);
            }
            if objective.mission_id != self.id {
                return Err(MissionDefinitionError::ObjectiveMissionMismatch);
            }
            if objectives.insert(objective.id, ()).is_some() {
                return Err(MissionDefinitionError::DuplicateObjective);
            }
        }
        for objective in &self.objectives {
            for dependency in &objective.dependencies {
                if !objectives.contains_key(dependency) || *dependency == objective.id {
                    return Err(MissionDefinitionError::UnknownDependency);
                }
            }
        }
        for objective in &self.objectives {
            let mut visiting = BTreeSet::new();
            if has_cycle(objective.id, &objectives, &self.objectives, &mut visiting) {
                return Err(MissionDefinitionError::CircularDependency);
            }
        }
        Ok(())
    }
}

fn has_cycle(
    current: ObjectiveId,
    known: &BTreeMap<ObjectiveId, ()>,
    objectives: &[Objective],
    visiting: &mut BTreeSet<ObjectiveId>,
) -> bool {
    if !visiting.insert(current) {
        return true;
    }
    let Some(objective) = objectives.iter().find(|objective| objective.id == current) else {
        return false;
    };
    let cycle = objective.dependencies.iter().any(|dependency| {
        known.contains_key(dependency) && has_cycle(*dependency, known, objectives, visiting)
    });
    visiting.remove(&current);
    cycle
}
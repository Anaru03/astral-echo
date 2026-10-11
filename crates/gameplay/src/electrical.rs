use engine::{EntityId, Vec3};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BreakerId(u16);

impl BreakerId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Off,
    On,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElectricalBreakerConfiguration {
    pub id: BreakerId,
    pub initial_state: BreakerState,
    pub target_state: BreakerState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElectricalPanelConfiguration {
    breakers: Vec<ElectricalBreakerConfiguration>,
}

impl ElectricalPanelConfiguration {
    pub fn new(breakers: Vec<ElectricalBreakerConfiguration>) -> Self {
        Self { breakers }
    }

    pub fn breakers(&self) -> &[ElectricalBreakerConfiguration] {
        &self.breakers
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElectricalBreaker {
    pub id: BreakerId,
    pub state: BreakerState,
    pub target_state: BreakerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElectricalPuzzleState {
    Pending,
    Repaired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElectricalPanelRegistrationError {
    InvalidPosition,
    InvalidRange,
    EmptyConfiguration,
    DuplicateBreakerId,
    InitiallySolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElectricalPanelProgress {
    pub configured_breakers: usize,
    pub total_breakers: usize,
    pub repaired: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElectricalPanel {
    pub entity_id: EntityId,
    breakers: Vec<ElectricalBreaker>,
    state: ElectricalPuzzleState,
}

impl ElectricalPanel {
    pub fn new(
        entity_id: EntityId,
        position: Vec3,
        interaction_range: f32,
        configuration: ElectricalPanelConfiguration,
    ) -> Result<Self, ElectricalPanelRegistrationError> {
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(ElectricalPanelRegistrationError::InvalidPosition);
        }
        if !interaction_range.is_finite() || interaction_range <= 0.0 {
            return Err(ElectricalPanelRegistrationError::InvalidRange);
        }
        if configuration.breakers.is_empty() {
            return Err(ElectricalPanelRegistrationError::EmptyConfiguration);
        }

        let mut ids = BTreeSet::new();
        let mut breakers = Vec::with_capacity(configuration.breakers.len());
        let mut solved = true;
        for definition in configuration.breakers {
            if !ids.insert(definition.id) {
                return Err(ElectricalPanelRegistrationError::DuplicateBreakerId);
            }
            solved &= definition.initial_state == definition.target_state;
            breakers.push(ElectricalBreaker {
                id: definition.id,
                state: definition.initial_state,
                target_state: definition.target_state,
            });
        }
        if solved {
            return Err(ElectricalPanelRegistrationError::InitiallySolved);
        }

        Ok(Self {
            entity_id,
            breakers,
            state: ElectricalPuzzleState::Pending,
        })
    }

    pub fn breakers(&self) -> &[ElectricalBreaker] {
        &self.breakers
    }

    pub fn state(&self) -> ElectricalPuzzleState {
        self.state
    }

    pub fn is_repaired(&self) -> bool {
        self.state == ElectricalPuzzleState::Repaired
    }

    pub fn progress(&self) -> ElectricalPanelProgress {
        ElectricalPanelProgress {
            configured_breakers: self
                .breakers
                .iter()
                .filter(|breaker| breaker.state == breaker.target_state)
                .count(),
            total_breakers: self.breakers.len(),
            repaired: self.is_repaired(),
        }
    }

    pub(crate) fn set_breaker_state(
        &mut self,
        breaker_id: BreakerId,
        state: BreakerState,
    ) -> Option<bool> {
        if self.is_repaired() {
            return None;
        }
        let breaker = self
            .breakers
            .iter_mut()
            .find(|breaker| breaker.id == breaker_id)?;
        if breaker.state == state {
            return Some(false);
        }
        breaker.state = state;
        if self
            .breakers
            .iter()
            .all(|breaker| breaker.state == breaker.target_state)
        {
            self.state = ElectricalPuzzleState::Repaired;
        }
        Some(true)
    }
}
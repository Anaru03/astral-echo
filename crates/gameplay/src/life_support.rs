use engine::EntityId;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompartmentId(u16);

impl CompartmentId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OxygenCompartmentConfiguration {
    pub id: CompartmentId,
    pub initial_allocation: u32,
    pub minimum_allocation: u32,
    pub maximum_allocation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifeSupportConfiguration {
    capacity: u32,
    compartments: Vec<OxygenCompartmentConfiguration>,
}

impl LifeSupportConfiguration {
    pub fn new(capacity: u32, compartments: Vec<OxygenCompartmentConfiguration>) -> Self {
        Self {
            capacity,
            compartments,
        }
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn compartments(&self) -> &[OxygenCompartmentConfiguration] {
        &self.compartments
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OxygenCompartment {
    pub id: CompartmentId,
    pub allocation: u32,
    pub minimum_allocation: u32,
    pub maximum_allocation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeSupportState {
    Pending,
    Repaired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeSupportRegistrationError {
    EmptyConfiguration,
    InvalidCompartmentId,
    DuplicateCompartmentId,
    InvalidCompartmentLimits,
    InitialAllocationOutOfBounds,
    InitialCapacityExceeded,
    MinimumCapacityExceeded,
    CapacityExceedsCompartmentLimits,
    ArithmeticOverflow,
    InitiallySolved,
    InvalidPosition,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifeSupportProgress {
    pub allocated_total: u32,
    pub capacity: u32,
    pub compartments_meeting_minimum: usize,
    pub total_compartments: usize,
    pub repaired: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LifeSupportPanel {
    pub entity_id: EntityId,
    capacity: u32,
    compartments: Vec<OxygenCompartment>,
    state: LifeSupportState,
}

impl LifeSupportPanel {
    pub(crate) fn validate_configuration(
        position: engine::Vec3,
        interaction_range: f32,
        configuration: &LifeSupportConfiguration,
    ) -> Result<(), LifeSupportRegistrationError> {
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(LifeSupportRegistrationError::InvalidPosition);
        }
        if !interaction_range.is_finite() || interaction_range <= 0.0 {
            return Err(LifeSupportRegistrationError::InvalidRange);
        }
        if configuration.compartments.is_empty() {
            return Err(LifeSupportRegistrationError::EmptyConfiguration);
        }
        let mut ids = BTreeSet::new();
        let mut minimum_total = 0u32;
        let mut maximum_total = 0u32;
        let mut initial_total = 0u32;
        for definition in &configuration.compartments {
            if definition.id.value() == 0 {
                return Err(LifeSupportRegistrationError::InvalidCompartmentId);
            }
            if !ids.insert(definition.id) {
                return Err(LifeSupportRegistrationError::DuplicateCompartmentId);
            }
            if definition.minimum_allocation > definition.maximum_allocation {
                return Err(LifeSupportRegistrationError::InvalidCompartmentLimits);
            }
            if definition.initial_allocation > definition.maximum_allocation {
                return Err(LifeSupportRegistrationError::InitialAllocationOutOfBounds);
            }
            minimum_total = minimum_total
                .checked_add(definition.minimum_allocation)
                .ok_or(LifeSupportRegistrationError::ArithmeticOverflow)?;
            maximum_total = maximum_total
                .checked_add(definition.maximum_allocation)
                .ok_or(LifeSupportRegistrationError::ArithmeticOverflow)?;
            initial_total = initial_total
                .checked_add(definition.initial_allocation)
                .ok_or(LifeSupportRegistrationError::ArithmeticOverflow)?;
        }
        if minimum_total > configuration.capacity {
            return Err(LifeSupportRegistrationError::MinimumCapacityExceeded);
        }
        if configuration.capacity > maximum_total {
            return Err(LifeSupportRegistrationError::CapacityExceedsCompartmentLimits);
        }
        if initial_total > configuration.capacity {
            return Err(LifeSupportRegistrationError::InitialCapacityExceeded);
        }
        if initial_total == configuration.capacity
            && configuration.compartments.iter().all(|compartment| {
                compartment.initial_allocation >= compartment.minimum_allocation
                    && compartment.initial_allocation <= compartment.maximum_allocation
            })
        {
            return Err(LifeSupportRegistrationError::InitiallySolved);
        }
        Ok(())
    }

    pub fn new(
        entity_id: EntityId,
        position: engine::Vec3,
        interaction_range: f32,
        configuration: LifeSupportConfiguration,
    ) -> Result<Self, LifeSupportRegistrationError> {
        Self::validate_configuration(position, interaction_range, &configuration)?;
        let mut compartments = Vec::with_capacity(configuration.compartments.len());

        for definition in configuration.compartments {
            compartments.push(OxygenCompartment {
                id: definition.id,
                allocation: definition.initial_allocation,
                minimum_allocation: definition.minimum_allocation,
                maximum_allocation: definition.maximum_allocation,
            });
        }

        let panel = Self {
            entity_id,
            capacity: configuration.capacity,
            compartments,
            state: LifeSupportState::Pending,
        };
        Ok(panel)
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn compartments(&self) -> &[OxygenCompartment] {
        &self.compartments
    }

    pub const fn state(&self) -> LifeSupportState {
        self.state
    }

    pub const fn is_repaired(&self) -> bool {
        matches!(self.state, LifeSupportState::Repaired)
    }

    pub fn progress(&self) -> LifeSupportProgress {
        let allocated_total = self.allocated_total();
        LifeSupportProgress {
            allocated_total,
            capacity: self.capacity,
            compartments_meeting_minimum: self
                .compartments
                .iter()
                .filter(|compartment| compartment.allocation >= compartment.minimum_allocation)
                .count(),
            total_compartments: self.compartments.len(),
            repaired: self.is_repaired(),
        }
    }

    pub fn allocated_total(&self) -> u32 {
        self.compartments
            .iter()
            .fold(0u32, |total, compartment| total + compartment.allocation)
    }

    pub fn is_solved(&self) -> bool {
        self.allocated_total() == self.capacity
            && self.compartments.iter().all(|compartment| {
                compartment.allocation >= compartment.minimum_allocation
                    && compartment.allocation <= compartment.maximum_allocation
            })
    }

    pub(crate) fn set_allocation(
        &mut self,
        compartment_id: CompartmentId,
        allocation: u32,
    ) -> Option<Result<bool, LifeSupportRegistrationError>> {
        if self.is_repaired() {
            return None;
        }
        let index = self
            .compartments
            .iter()
            .position(|compartment| compartment.id == compartment_id)?;
        let current_allocation = self.compartments[index].allocation;
        let maximum_allocation = self.compartments[index].maximum_allocation;
        if allocation > maximum_allocation {
            return Some(Err(
                LifeSupportRegistrationError::InitialAllocationOutOfBounds,
            ));
        }
        let current_total = self.allocated_total();
        let without_current = current_total - current_allocation;
        let new_total = without_current.checked_add(allocation)?;
        if new_total > self.capacity {
            return Some(Err(LifeSupportRegistrationError::InitialCapacityExceeded));
        }
        if current_allocation == allocation {
            return Some(Ok(false));
        }
        self.compartments[index].allocation = allocation;
        if self.is_solved() {
            self.state = LifeSupportState::Repaired;
        }
        Some(Ok(true))
    }
}
use engine::EntityId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FuelTank {
    current: u32,
    capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuelError {
    ShipNotFound,
    InvalidCapacity,
    InitialFuelExceedsCapacity,
    FuelCapacityExceeded,
    InsufficientFuel,
    ArithmeticOverflow,
    AlreadyInitialized,
}

impl FuelTank {
    pub fn new(capacity: u32, initial: u32) -> Result<Self, FuelError> {
        if capacity == 0 {
            return Err(FuelError::InvalidCapacity);
        }
        if initial > capacity {
            return Err(FuelError::InitialFuelExceedsCapacity);
        }
        Ok(Self {
            current: initial,
            capacity,
        })
    }

    pub const fn current(self) -> u32 {
        self.current
    }

    pub const fn capacity(self) -> u32 {
        self.capacity
    }

    pub const fn is_empty(self) -> bool {
        self.current == 0
    }

    pub const fn can_consume(self, amount: u32) -> bool {
        amount <= self.current
    }

    pub fn add(&mut self, amount: u32) -> Result<bool, FuelError> {
        let next = self
            .current
            .checked_add(amount)
            .ok_or(FuelError::ArithmeticOverflow)?;
        if next > self.capacity {
            return Err(FuelError::FuelCapacityExceeded);
        }
        if next == self.current {
            return Ok(false);
        }
        self.current = next;
        Ok(true)
    }

    pub fn consume(&mut self, amount: u32) -> Result<bool, FuelError> {
        if amount > self.current {
            return Err(FuelError::InsufficientFuel);
        }
        if amount == 0 {
            return Ok(false);
        }
        self.current -= amount;
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShipResources {
    pub ship_id: EntityId,
    fuel: FuelTank,
}

impl ShipResources {
    pub fn new(ship_id: EntityId, capacity: u32, initial_fuel: u32) -> Result<Self, FuelError> {
        Ok(Self {
            ship_id,
            fuel: FuelTank::new(capacity, initial_fuel)?,
        })
    }

    pub const fn fuel(self) -> FuelTank {
        self.fuel
    }

    pub fn fuel_mut(&mut self) -> &mut FuelTank {
        &mut self.fuel
    }
}
use crate::types::{ItemId, ResourceId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(maximum: f32) -> Self {
        Self {
            current: maximum,
            maximum,
        }
    }
    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAmount {
    pub resource: ResourceId,
    pub amount: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    items: Vec<(ItemId, u32)>,
    capacity: u32,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            capacity: u32::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    InvalidCapacity,
    PlayerNotFound,
    AlreadyInitialized,
    InvalidQuantity,
    CapacityExceeded,
    ArithmeticOverflow,
    InsufficientQuantity,
}

impl Inventory {
    pub fn new(capacity: u32) -> Result<Self, InventoryError> {
        if capacity == 0 {
            return Err(InventoryError::InvalidCapacity);
        }
        Ok(Self {
            items: Vec::new(),
            capacity,
        })
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn used_capacity(&self) -> u32 {
        self.items
            .iter()
            .map(|(_, quantity)| *quantity)
            .fold(0, u32::saturating_add)
    }

    pub fn remaining_capacity(&self) -> u32 {
        self.capacity.saturating_sub(self.used_capacity())
    }

    pub fn can_add(&self, quantity: u32) -> bool {
        quantity > 0 && quantity <= self.remaining_capacity()
    }

    pub fn try_add(&mut self, item: ItemId, quantity: u32) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::InvalidQuantity);
        }
        if quantity > self.remaining_capacity() {
            return Err(InventoryError::CapacityExceeded);
        }
        let Some((_, current)) = self.items.iter_mut().find(|(id, _)| *id == item) else {
            self.items.push((item, quantity));
            return Ok(());
        };
        *current = current
            .checked_add(quantity)
            .ok_or(InventoryError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn try_remove(&mut self, item: ItemId, quantity: u32) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::InvalidQuantity);
        }
        let Some((_, current)) = self.items.iter_mut().find(|(id, _)| *id == item) else {
            return Err(InventoryError::InsufficientQuantity);
        };
        if *current < quantity {
            return Err(InventoryError::InsufficientQuantity);
        }
        *current -= quantity;
        Ok(())
    }

    pub fn quantity(&self, item: ItemId) -> u32 {
        self.items
            .iter()
            .find(|(id, _)| *id == item)
            .map_or(0, |(_, quantity)| *quantity)
    }

    pub fn entries(&self) -> impl Iterator<Item = (ItemId, u32)> + '_ {
        self.items.iter().copied()
    }

    pub fn from_entries(
        capacity: u32,
        entries: impl IntoIterator<Item = (ItemId, u32)>,
    ) -> Result<Self, InventoryError> {
        let mut inventory = Self::new(capacity)?;
        for (item, quantity) in entries {
            inventory.try_add(item, quantity)?;
        }
        Ok(inventory)
    }

    pub fn add(&mut self, item: ItemId, quantity: u32) {
        let _ = self.try_add(item, quantity);
    }

    pub fn remove(&mut self, item: ItemId, quantity: u32) -> bool {
        self.try_remove(item, quantity).is_ok()
    }
}
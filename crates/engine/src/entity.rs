use core::fmt;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(u64);

impl EntityId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityRegistry {
    next_id: u64,
    known: BTreeSet<EntityId>,
    active: BTreeSet<EntityId>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            known: BTreeSet::new(),
            active: BTreeSet::new(),
        }
    }

    pub fn create(&mut self) -> EntityId {
        let id = EntityId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.known.insert(id);
        self.active.insert(id);
        id
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.active.contains(&entity)
    }

    pub fn deactivate(&mut self, entity: EntityId) -> bool {
        self.active.remove(&entity)
    }

    pub fn activate(&mut self, entity: EntityId) -> bool {
        if !self.known.contains(&entity) {
            return false;
        }
        self.active.insert(entity)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}
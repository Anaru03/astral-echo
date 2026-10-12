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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityRegistrySnapshot {
    pub next_id: u64,
    pub known: Vec<EntityId>,
    pub active: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityRegistrySnapshotError {
    InvalidNextId,
    DuplicateKnown,
    DuplicateActive,
    ActiveEntityUnknown,
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

    pub fn snapshot(&self) -> EntityRegistrySnapshot {
        EntityRegistrySnapshot {
            next_id: self.next_id,
            known: self.known.iter().copied().collect(),
            active: self.active.iter().copied().collect(),
        }
    }

    pub fn from_snapshot(
        snapshot: EntityRegistrySnapshot,
    ) -> Result<Self, EntityRegistrySnapshotError> {
        if snapshot.next_id == 0 {
            return Err(EntityRegistrySnapshotError::InvalidNextId);
        }
        let known: BTreeSet<_> = snapshot.known.iter().copied().collect();
        if known.len() != snapshot.known.len() {
            return Err(EntityRegistrySnapshotError::DuplicateKnown);
        }
        let active: BTreeSet<_> = snapshot.active.iter().copied().collect();
        if active.len() != snapshot.active.len() {
            return Err(EntityRegistrySnapshotError::DuplicateActive);
        }
        if active.iter().any(|entity| !known.contains(entity)) {
            return Err(EntityRegistrySnapshotError::ActiveEntityUnknown);
        }
        if known.iter().any(|entity| entity.value() >= snapshot.next_id) {
            return Err(EntityRegistrySnapshotError::InvalidNextId);
        }
        Ok(Self {
            next_id: snapshot.next_id,
            known,
            active,
        })
    }
}
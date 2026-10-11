use crate::{navigation::DestinationId, types::ItemId};
use engine::EntityId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KnowledgeTarget {
    Destination(DestinationId),
    Enemy(EntityId),
    Item(ItemId),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KnowledgeState {
    Unknown,
    Detected,
    Identified,
    Discovered,
    Studied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeCategory {
    Destination,
    Enemy,
    Item,
}

impl KnowledgeCategory {
    pub const fn matches(self, target: &KnowledgeTarget) -> bool {
        matches!(
            (self, target),
            (Self::Destination, KnowledgeTarget::Destination(_))
                | (Self::Enemy, KnowledgeTarget::Enemy(_))
                | (Self::Item, KnowledgeTarget::Item(_))
        )
    }
}

impl KnowledgeState {
    pub const fn is_detected(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    pub const fn is_identified(self) -> bool {
        matches!(self, Self::Identified | Self::Discovered | Self::Studied)
    }

    pub const fn is_discovered(self) -> bool {
        matches!(self, Self::Discovered | Self::Studied)
    }

    pub const fn is_studied(self) -> bool {
        matches!(self, Self::Studied)
    }

    pub const fn can_study(self) -> bool {
        self.is_identified()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeError {
    TargetNotFound,
    AlreadyRegistered,
    UnknownTarget,
    NotIdentified,
    TransitionNotAllowed,
}
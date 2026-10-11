pub mod door {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DoorState {
        Closed,
        Opening,
        Open,
        Closing,
        Locked,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DoorCommand {
        Open,
        Close,
        Unlock,
    }
}

pub mod interaction {
    use engine::EntityId;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct InteractionRequest {
        pub actor: EntityId,
        pub target: EntityId,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum InteractionState {
        Available,
        InProgress,
        Completed,
        Unavailable,
    }
}

pub mod mission {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum MissionState {
        Locked,
        Available,
        Active,
        Completed,
        Failed,
    }
}

pub mod objective {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ObjectiveState {
        Locked,
        Available,
        Active,
        Completed,
        Failed,
    }
}

pub mod puzzle {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PuzzleState {
        Unavailable,
        Unsolved,
        Solved,
        Failed,
    }
}

pub mod ship {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ShipState {
        Docked,
        Ready,
        Flying,
        Disabled,
    }
}

pub mod weapon {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WeaponState {
        Holstered,
        Ready,
        Cooldown,
        Empty,
    }
}

pub mod enemy {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EnemyState {
        Dormant,
        Alert,
        Defeated,
    }
}

pub mod resource {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ResourceState {
        Available,
        Collected,
        Depleted,
    }
}

pub mod planet {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PlanetState {
        Unknown,
        Discovered,
        Visited,
    }
}

pub mod progression {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProgressionState {
        Locked,
        Unlocked,
        Completed,
    }
}
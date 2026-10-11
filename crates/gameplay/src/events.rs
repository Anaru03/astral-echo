use crate::{
    domains::{
        door::DoorState, enemy::EnemyState, mission::MissionState, objective::ObjectiveState,
        planet::PlanetState, progression::ProgressionState, puzzle::PuzzleState,
        resource::ResourceState, ship::ShipState, weapon::WeaponState,
    },
    types::{
        DoorId, EnemyId, ItemId, MissionId, ObjectiveId, PlayerId, PuzzleId, ResourceId, ShipId,
    },
};
use engine::{Duration, EntityId, GameTime, Vec3};

#[derive(Clone, Debug, PartialEq)]
pub enum GameplayEvent {
    TimeAdvanced {
        from: GameTime,
        to: GameTime,
    },
    PlayerCreated {
        player: PlayerId,
        position: Vec3,
    },
    PlayerMoved {
        player: PlayerId,
        position: Vec3,
        velocity: Vec3,
    },
    PlayerOrientationChanged {
        player: PlayerId,
        yaw_degrees: f32,
        pitch_degrees: f32,
    },
    PlayerActivated {
        player: PlayerId,
    },
    PlayerDeactivated {
        player: PlayerId,
    },
    InteractionSucceeded {
        player: PlayerId,
        target: EntityId,
    },
    ElectricalPanelInteractionStarted {
        player: EntityId,
        panel: EntityId,
    },
    ElectricalPanelCreated {
        panel: EntityId,
    },
    ElectricalBreakerStateChanged {
        panel: EntityId,
        breaker: crate::electrical::BreakerId,
        state: crate::electrical::BreakerState,
    },
    LifeSupportPanelInteractionStarted {
        player: EntityId,
        panel: EntityId,
    },
    LifeSupportPanelCreated {
        panel: EntityId,
    },
    OxygenAllocationChanged {
        panel: EntityId,
        compartment: crate::life_support::CompartmentId,
        allocation: u32,
    },
    DestinationRegistered {
        destination: crate::navigation::DestinationId,
    },
    DestinationSelected {
        player: EntityId,
        pilot_station: EntityId,
        destination: crate::navigation::DestinationId,
    },
    TravelStarted {
        player: EntityId,
        pilot_station: EntityId,
        destination: crate::navigation::DestinationId,
        started_at: Duration,
        duration: Duration,
        fuel_cost: u32,
    },
    TravelArrived {
        destination: crate::navigation::DestinationId,
        position: Vec3,
    },
    PlanetDiscovered {
        destination: crate::navigation::DestinationId,
    },
    ShipResourcesInitialized {
        ship: EntityId,
        fuel: u32,
        capacity: u32,
    },
    FuelAdded {
        ship: EntityId,
        amount: u32,
        fuel: u32,
    },
    FuelRemoved {
        ship: EntityId,
        amount: u32,
        fuel: u32,
    },
    FuelConsumed {
        ship: EntityId,
        destination: crate::navigation::DestinationId,
        amount: u32,
        remaining: u32,
    },
    EnemyCreated {
        enemy: EntityId,
        archetype: crate::combat::EnemyArchetype,
        position: Vec3,
    },
    AttackExecuted {
        attacker: EntityId,
        target: EntityId,
        attack_type: crate::combat::AttackType,
        damage: u32,
    },
    CombatDamageApplied {
        attacker: EntityId,
        target: EntityId,
        amount: u32,
        remaining_health: u32,
    },
    AmmunitionConsumed {
        player: EntityId,
        item: ItemId,
        amount: u32,
        attack_type: crate::combat::AttackType,
        at: Duration,
    },
    ShipCombatDamageApplied {
        attacker: EntityId,
        ship: EntityId,
        amount: u32,
        remaining_integrity: u32,
    },
    ShieldDamaged {
        enemy: EntityId,
        amount: u32,
        remaining: u32,
    },
    ShieldDisabled {
        enemy: EntityId,
    },
    EnemyDefeated {
        enemy: EntityId,
        archetype: crate::combat::EnemyArchetype,
    },
    RewardCollectibleCreated {
        enemy: EntityId,
        collectible: EntityId,
        item: ItemId,
        amount: u32,
        position: Vec3,
    },
    EnemyRewardProcessed {
        enemy: EntityId,
        archetype: crate::combat::EnemyArchetype,
        collectibles: u32,
    },
    PlayerDefeated {
        player: EntityId,
    },
    PlayerHealed {
        player: EntityId,
        item: ItemId,
        consumed: u32,
        health_before: u32,
        restored: u32,
        health_after: u32,
    },
    PlayerRevived {
        helper: EntityId,
        target: EntityId,
        item: ItemId,
        consumed: u32,
        health_before: u32,
        restored: u32,
        health_after: u32,
    },
    ConsoleStateChanged {
        console: EntityId,
        state: crate::interaction::ConsoleState,
        role: crate::interaction::ConsoleRole,
    },
    DoorOpened {
        door: EntityId,
        state: DoorState,
        role: crate::door::DoorRole,
    },
    DoorClosed {
        door: EntityId,
        state: DoorState,
        role: crate::door::DoorRole,
    },
    DoorLocked {
        door: EntityId,
        state: DoorState,
        role: crate::door::DoorRole,
    },
    DoorUnlocked {
        door: EntityId,
        state: DoorState,
        role: crate::door::DoorRole,
    },
    StationOccupied {
        station: EntityId,
        player: EntityId,
        state: crate::station::StationState,
        station_type: crate::station::StationType,
    },
    StationReleased {
        station: EntityId,
        player: EntityId,
        state: crate::station::StationState,
    },
    StationEnabled {
        station: EntityId,
        state: crate::station::StationState,
    },
    StationDisabled {
        station: EntityId,
        state: crate::station::StationState,
    },
    ElectricalSystemRepaired {
        panel: EntityId,
    },
    OxygenSystemRepaired {
        panel: EntityId,
    },
    ShipStatusEvaluated {
        console: EntityId,
        role: crate::interaction::ConsoleRole,
    },
    MissionUnlocked {
        mission: MissionId,
        state: MissionState,
    },
    MissionStarted {
        mission: MissionId,
        state: MissionState,
    },
    MissionCompleted {
        mission: MissionId,
        state: MissionState,
    },
    MissionFailed {
        mission: MissionId,
        state: MissionState,
    },
    ObjectiveAvailable {
        objective: ObjectiveId,
        state: ObjectiveState,
    },
    ObjectiveStarted {
        objective: ObjectiveId,
        state: ObjectiveState,
    },
    ObjectiveCompleted {
        objective: ObjectiveId,
        state: ObjectiveState,
    },
    ObjectiveFailed {
        objective: ObjectiveId,
        state: ObjectiveState,
    },
    InteractionStarted {
        actor: EntityId,
        target: EntityId,
    },
    DoorStateChanged {
        door: DoorId,
        state: DoorState,
    },
    MissionStateChanged {
        mission: MissionId,
        state: MissionState,
    },
    ObjectiveStateChanged {
        objective: ObjectiveId,
        state: ObjectiveState,
    },
    PuzzleStateChanged {
        puzzle: PuzzleId,
        state: PuzzleState,
    },
    ShipStateChanged {
        ship: ShipId,
        state: ShipState,
    },
    WeaponStateChanged {
        owner: EntityId,
        state: WeaponState,
    },
    DamageApplied {
        target: EnemyId,
        amount: f32,
        state: EnemyState,
    },
    EnemyStateChanged {
        enemy: EnemyId,
        state: EnemyState,
    },
    ResourceStateChanged {
        resource: ResourceId,
        state: ResourceState,
    },
    PlanetStateChanged {
        planet: u32,
        state: PlanetState,
    },
    ItemAdded {
        player: PlayerId,
        item: ItemId,
        amount: u32,
    },
    InventoryInitialized {
        player: EntityId,
        capacity: u32,
    },
    ItemRemoved {
        player: EntityId,
        item: ItemId,
        amount: u32,
    },
    ItemCollected {
        player: EntityId,
        collectible: EntityId,
        item: ItemId,
        amount: u32,
        remaining: u32,
    },
    CollectibleCreated {
        collectible: EntityId,
        item: ItemId,
        amount: u32,
        position: Vec3,
    },
    CollectibleDepleted {
        collectible: EntityId,
        item: ItemId,
    },
    ShipOrientationChanged {
        ship: EntityId,
        yaw_degrees: f32,
        pitch_degrees: f32,
    },
    ShipVelocityChanged {
        ship: EntityId,
        velocity: Vec3,
    },
    ShipPositionChanged {
        ship: EntityId,
        position: Vec3,
    },
    ShipBraked {
        ship: EntityId,
    },
    ShipControlChanged {
        ship: EntityId,
        active: bool,
    },
    SignalScanCompleted {
        ship: EntityId,
        detected: u32,
        at: Duration,
    },
    SignalDetected {
        ship: EntityId,
        destination: crate::navigation::DestinationId,
        distance: f32,
    },
    DestinationIdentified {
        ship: EntityId,
        destination: crate::navigation::DestinationId,
    },
    KnowledgeDetected {
        target: crate::knowledge::KnowledgeTarget,
    },
    KnowledgeIdentified {
        target: crate::knowledge::KnowledgeTarget,
    },
    KnowledgeDiscovered {
        target: crate::knowledge::KnowledgeTarget,
    },
    KnowledgeStudied {
        player: EntityId,
        target: crate::knowledge::KnowledgeTarget,
    },
    ShipIntegrityInitialized {
        ship: EntityId,
        current: u32,
        maximum: u32,
    },
    ShipDamageApplied {
        ship: EntityId,
        amount: u32,
        remaining: u32,
        state: crate::integrity::StructuralState,
    },
    ShipRepairApplied {
        ship: EntityId,
        amount: u32,
        current: u32,
        state: crate::integrity::StructuralState,
    },
    ShipRepaired {
        ship: EntityId,
        player: EntityId,
        station: EntityId,
        amount: u32,
        current: u32,
        material: ItemId,
        material_consumed: u32,
    },
    EnvironmentalHazardRegistered {
        hazard: crate::hazard::HazardId,
        hazard_type: crate::hazard::EnvironmentalHazardType,
        position: Vec3,
    },
    ShipDamagedByHazard {
        hazard: crate::hazard::HazardId,
        hazard_type: crate::hazard::EnvironmentalHazardType,
        ship: EntityId,
        attempted_damage: u32,
        applied_damage: u32,
        remaining_integrity: u32,
        intervals: u64,
    },
    ShipEnteredCritical {
        ship: EntityId,
        remaining: u32,
    },
    ShipDestroyed {
        ship: EntityId,
    },
    TravelCancelledByDestruction {
        ship: EntityId,
        destination: crate::navigation::DestinationId,
    },
    ProgressionStateChanged {
        player: PlayerId,
        state: ProgressionState,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventLog {
    events: Vec<GameplayEvent>,
}

impl EventLog {
    pub fn push(&mut self, event: GameplayEvent) {
        self.events.push(event);
    }
    pub fn iter(&self) -> impl Iterator<Item = &GameplayEvent> {
        self.events.iter()
    }
    pub fn drain(&mut self) -> impl Iterator<Item = GameplayEvent> + '_ {
        self.events.drain(..)
    }
    pub fn take(&mut self) -> Vec<GameplayEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub(crate) fn clear(&mut self) {
        self.events.clear();
    }
}
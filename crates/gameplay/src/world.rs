use engine::{Duration, EntityId, EntityRegistry, GameTime};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    companion::{
        Companion, CompanionActionMode, CompanionBehavior, CompanionError, CompanionRecommendation,
        ExplorationAction, ExplorationAssessment, ExplorationPriority,
        ExplorationRecommendation, ExplorationRecommendationReason, ExplorationTargetCategory,
        FuelCondition, HullCondition, MaintenancePriority, MaintenanceRecommendation,
        NavigationAdvice, NavigationAlternativeRecommendation, NavigationAssessment,
        NavigationRecommendationReason, NavigationRisk, NavigationRouteComparison,
        NavigationRouteOption, NavigationRouteRisk, NavigationRouteRiskClassification,
        NavigationRouteRiskSource, OperationalAssessment, PreparationAction,
        PreparationAssessment, PreparationCategory, PreparationPriority, PreparationReason,
        PreparationRequirement, CooperativeTacticalAssessment,
        PlayerTacticalRecommendation, TacticalAction, TacticalAssignmentEvidence,
        TacticalAssignmentReason, TacticalAssignmentStatus, TacticalPriority,
        TacticalRecommendation,
        CooperativeEncounterReadinessAssessment, CooperativeEncounterReadinessStatus,
        CooperativePreparationAction, ReadinessBlocker, ReadinessRequirement,
        CooperativeSupportAssessment, PlayerSupportRecommendation, SupportEvidence,
        SupportRecommendationReason, SupportRecommendationStatus,
        CoordinationAction, CoordinationAssessment, CoordinationCategory, CoordinationPriority,
        CoordinationReason, CoordinationStep, CoordinationStepStatus,
        RecoveryAction, RecoveryAlternative, RecoveryAssessment, RecoveryBlocker,
        RecoveryCategory, RecoveryReason, RecoveryStatus,
        ConsistencyAuditState, ConsistencyEvidence, Contradiction,
        ContradictionCategory, ContradictionReason, ContradictionSeverity,
        ExplainedAlternative, ExplanationEvidence, ExplanationEvidenceKind, ExplanationReason,
        ExplanationStatus, RecommendationExplanation,
    },
    collectible::{Collectible, CollectibleError, ItemDefinition, ItemDefinitionError},
    combat::{Combatant, EnemyConfiguration},
    components::{Inventory, InventoryError},
    door::{Door, DoorAccessRequirement},
    derelict::{Derelict, DerelictError, DerelictId, DerelictState},
    electrical::{ElectricalPanel, ElectricalPanelConfiguration, ElectricalPanelRegistrationError},
    encounter::{EncounterId, SpatialEncounter, SpatialEncounterError, SpatialEncounterKind},
    events::EventLog,
    hazard::{EnvironmentalHazard, EnvironmentalHazardError, HazardId},
    healing::{HealingError, HealingResult},
    guidance::{GuidanceAction, GuidanceRequirement},
    integrity::{
        IntegrityError, ShipIntegrity, ShipRepairConfiguration, ShipRepairError, StructuralState,
    },
    interaction::{Interactable, InteractionBehavior, InteractionRegistrationError},
    knowledge::{KnowledgeState, KnowledgeTarget},
    life_support::{LifeSupportConfiguration, LifeSupportPanel, LifeSupportRegistrationError},
    mission::{Mission, MissionRegistrationError},
    navigation::{
        Destination, DestinationId, NavigationInitializationError, NavigationRegistrationError,
        NavigationState, TravelProgress, TravelState,
    },
    player::Player,
    resources::{FuelError, FuelTank, ShipResources},
    persistence::{
        CombatantSaveState, CollectibleSaveState, GameplaySaveState, InventorySaveState,
        MissionSaveState, PlayerSaveState, SaveError,
    },
    revival::{RevivalError, ReviveResult},
    reward::{EnemyReward, RewardDistributionPolicy, RewardError},
    scaling::{GameplayScaling, GroupSize, ScalingError},
    station::{Station, StationType},
    types::{GameplayState, SimulationPhase},
    world_activity::{
        FunctionalWorld, FunctionalWorldError, WorldActivity, WorldActivityError,
    },
};

pub const MAX_LOCAL_PLAYERS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerCreationError {
    MaximumPlayersReached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryTransferError {
    PlayerNotFound,
    PlayerInactive,
    SamePlayer,
    InvalidQuantity,
    ItemDefinitionNotFound,
    InventoryNotInitialized,
    InsufficientQuantity,
    CapacityExceeded,
    StackExceeded,
    ArithmeticOverflow,
}

#[derive(Clone, Debug, Default)]
pub struct GameplayWorld {
    state: GameplayState,
    entities: EntityRegistry,
    players: BTreeMap<EntityId, Player>,
    companions: BTreeMap<EntityId, Companion>,
    interactables: BTreeMap<EntityId, Interactable>,
    missions: BTreeMap<crate::types::MissionId, Mission>,
    electrical_panels: BTreeMap<EntityId, ElectricalPanel>,
    life_support_panels: BTreeMap<EntityId, LifeSupportPanel>,
    navigation: NavigationState,
    ship_resources: BTreeMap<EntityId, ShipResources>,
    combatants: BTreeMap<EntityId, Combatant>,
    inventories: BTreeMap<EntityId, Inventory>,
    item_definitions: BTreeMap<crate::types::ItemId, ItemDefinition>,
    collectibles: BTreeMap<EntityId, Collectible>,
    ship_integrity: BTreeMap<EntityId, ShipIntegrity>,
    environmental_hazards: BTreeMap<HazardId, EnvironmentalHazard>,
    spatial_encounters: BTreeMap<EncounterId, SpatialEncounter>,
    derelicts: BTreeMap<DerelictId, Derelict>,
    functional_worlds: BTreeMap<DestinationId, FunctionalWorld>,
    currencies: BTreeMap<EntityId, u64>,
    enemy_rewards: BTreeMap<crate::combat::EnemyArchetype, EnemyReward>,
    processed_enemy_rewards: BTreeSet<EntityId>,
    knowledge: BTreeMap<KnowledgeTarget, KnowledgeState>,
    events: EventLog,
}

impl GameplayWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_state(&self) -> GameplaySaveState {
        let mut save = GameplaySaveState {
            schema_version: crate::persistence::GAMEPLAY_SAVE_SCHEMA_VERSION,
            phase: self.state.phase,
            time: self.state.time,
            registry: self.entities.snapshot(),
            players: self
                .players
                .values()
                .map(|player| PlayerSaveState {
                    id: player.entity_id,
                    position: player.position,
                    yaw: player.orientation.yaw_degrees,
                    pitch: player.orientation.pitch_degrees,
                    velocity: player.velocity,
                    active: player.is_active,
                })
                .collect(),
            companions: self
                .companions
                .values()
                .map(|companion| crate::persistence::CompanionSaveState {
                    entity: companion.entity,
                    followed_player: companion.followed_player,
                    position: companion.position,
                    behavior: companion.behavior,
                    active: companion.active,
                })
                .collect(),
            inventories: self
                .inventories
                .iter()
                .map(|(owner, inventory)| InventorySaveState {
                    owner: *owner,
                    capacity: inventory.capacity(),
                    entries: inventory.entries().collect(),
                })
                .collect(),
            combatants: self
                .combatants
                .values()
                .map(|combatant| CombatantSaveState {
                    id: combatant.entity,
                    is_player: combatant.is_player,
                    active: combatant.active,
                    maximum_health: combatant.maximum_health,
                    current_health: combatant.current_health,
                    position: combatant.position,
                    archetype: combatant.archetype,
                    weapon: combatant.weapon,
                    equipped_weapon: combatant.equipped_weapon,
                    shield: combatant.shield,
                    mobility: combatant.mobility,
                    last_attack_at: combatant.last_attack_at,
                })
                .collect(),
            currencies: self.currencies.iter().map(|(id, value)| (*id, *value)).collect(),
            missions: self
                .missions
                .values()
                .map(|mission| MissionSaveState {
                    id: mission.id,
                    state: mission.state(),
                    objectives: mission
                        .objectives()
                        .iter()
                        .map(|objective| (objective.id, objective.state(), objective.progress()))
                        .collect(),
                })
                .collect(),
            knowledge: self.knowledge.iter().map(|(target, state)| (*target, *state)).collect(),
            collectibles: self
                .collectibles
                .values()
                .map(|item| CollectibleSaveState {
                    entity: item.entity,
                    item: item.item,
                    amount: item.amount,
                    position: item.position,
                    interaction_range: item.interaction_range,
                    owner: item.owner,
                    active: item.active,
                })
                .collect(),
            ship_fuel: self
                .ship_resources
                .values()
                .map(|resources| {
                    let fuel = resources.fuel();
                    (resources.ship_id, fuel.current(), fuel.capacity())
                })
                .collect(),
            ship_integrity: self
                .ship_integrity
                .values()
                .map(|integrity| {
                    (
                        integrity.ship_id,
                        integrity.current(),
                        integrity.maximum(),
                        integrity.critical_threshold(),
                        integrity.current(),
                    )
                })
                .collect(),
            ship_position: self.navigation.ship_id().map(|_| self.navigation.current_position()),
            destinations: self.navigation.destinations().cloned().collect(),
            routes: self.navigation.routes().cloned().collect(),
            item_definitions: self.item_definitions.values().copied().collect(),
            enemy_rewards: self.enemy_rewards.values().cloned().collect(),
            processed_enemy_rewards: self.processed_enemy_rewards.iter().copied().collect(),
            hazards: self.environmental_hazards.values().copied().collect(),
            encounters: self.spatial_encounters.values().copied().collect(),
            derelicts: self.derelicts.values().cloned().collect(),
            functional_worlds: self.functional_worlds.values().cloned().collect(),
            world_snapshot: None,
        };
        save.world_snapshot = Some(Box::new(self.clone()));
        save
    }

    pub fn load_state(&mut self, save: GameplaySaveState) -> Result<(), SaveError> {
        if save.schema_version != crate::persistence::GAMEPLAY_SAVE_SCHEMA_VERSION {
            return Err(SaveError::UnsupportedSchema);
        }
        if let Some(mut snapshot) = save.world_snapshot {
            snapshot.events = EventLog::default();
            *self = *snapshot;
            return Ok(());
        }
        let registry = EntityRegistry::from_snapshot(save.registry.clone())
            .map_err(|_| SaveError::InvalidRegistry)?;
        let mut item_definitions = BTreeMap::new();
        for definition in &save.item_definitions {
            definition.validate().map_err(|_| SaveError::InvalidState)?;
            if item_definitions.insert(definition.id, *definition).is_some() {
                return Err(SaveError::DuplicateRecord);
            }
        }
        if item_definitions.is_empty() {
            item_definitions = self.item_definitions.clone();
        }
        let mut players = BTreeMap::new();
        let mut player_ids = BTreeSet::new();
        for saved in &save.players {
            if !registry.snapshot().known.contains(&saved.id)
                || !player_ids.insert(saved.id)
                || !finite_vec3(saved.position)
                || !finite_vec3(saved.velocity)
                || !saved.yaw.is_finite()
                || !saved.pitch.is_finite()
            {
                return Err(SaveError::InvalidState);
            }
            let mut player = self.players.get(&saved.id).copied().unwrap_or_else(|| Player::new(saved.id));
            player.position = saved.position;
            player.orientation.yaw_degrees = saved.yaw;
            player.orientation.pitch_degrees = saved.pitch;
            player.velocity = saved.velocity;
            player.is_active = saved.active;
            if saved.active != registry.snapshot().active.contains(&saved.id) {
                return Err(SaveError::InvalidState);
            }
            players.insert(saved.id, player);
        }
        let mut inventories = BTreeMap::new();
        for saved in &save.inventories {
            if !players.contains_key(&saved.owner) {
                return Err(SaveError::InvalidReference);
            }
            for (item, _) in &saved.entries {
                if !item_definitions.contains_key(item) {
                    return Err(SaveError::InvalidReference);
                }
            }
            let inventory = Inventory::from_entries(saved.capacity, saved.entries.clone())
                .map_err(|_| SaveError::InvalidState)?;
            inventories.insert(saved.owner, inventory);
        }
        if inventories.len() > players.len() {
            return Err(SaveError::InvalidState);
        }
        let mut combatants = BTreeMap::new();
        for saved in &save.combatants {
            if !registry.snapshot().known.contains(&saved.id)
                || saved.maximum_health == 0
                || saved.current_health > saved.maximum_health
                || !finite_vec3(saved.position)
            {
                return Err(SaveError::InvalidState);
            }
            if saved.is_player && !players.contains_key(&saved.id) { return Err(SaveError::InvalidReference); }
            if saved.is_player != saved.archetype.is_none()
                || saved.is_player != players.get(&saved.id).is_some_and(|player| player.is_active == saved.active)
                || (saved.is_player && saved.active != registry.snapshot().active.contains(&saved.id))
            { return Err(SaveError::InvalidState); }
            combatants.insert(saved.id, Combatant {
                entity: saved.id, is_player: saved.is_player, active: saved.active,
                maximum_health: saved.maximum_health, current_health: saved.current_health,
                archetype: saved.archetype, position: saved.position, weapon: saved.weapon,
                equipped_weapon: saved.equipped_weapon, shield: saved.shield,
                mobility: saved.mobility, last_attack_at: saved.last_attack_at,
            });
        }
        for saved in &save.collectibles {
            if !registry.snapshot().known.contains(&saved.entity)
                || !item_definitions.contains_key(&saved.item)
                || (saved.active && saved.amount == 0)
                || (!saved.active && saved.amount != 0)
                || saved.owner.is_some_and(|owner| !players.contains_key(&owner))
                || !saved.position.x.is_finite()
                || !saved.position.y.is_finite()
                || !saved.position.z.is_finite()
            {
                return Err(SaveError::InvalidState);
            }
        }
        let mut ship_resources = BTreeMap::new();
        for (ship, current, capacity) in &save.ship_fuel {
            if !registry.snapshot().known.contains(ship) { return Err(SaveError::InvalidReference); }
            ship_resources.insert(
                *ship,
                ShipResources::new(*ship, *capacity, *current).map_err(|_| SaveError::InvalidState)?,
            );
        }
        let mut ship_integrity = BTreeMap::new();
        for (ship, current, maximum, threshold, _) in &save.ship_integrity {
            if !registry.snapshot().known.contains(ship) { return Err(SaveError::InvalidReference); }
            ship_integrity.insert(
                *ship,
                ShipIntegrity::new(*ship, *maximum, *current, *threshold)
                    .map_err(|_| SaveError::InvalidState)?,
            );
        }
        let mut restored_navigation = self.navigation.clone();
        if let Some(position) = save.ship_position {
            restored_navigation
                .restore_position(position)
                .map_err(|_| SaveError::InvalidState)?;
        }
        restored_navigation
            .restore_destinations_and_routes(save.destinations.clone(), save.routes.clone())
            .map_err(|_| SaveError::InvalidState)?;
        let mut companions = BTreeMap::new();
        for saved in &save.companions {
            if !registry.snapshot().known.contains(&saved.entity)
                || players.contains_key(&saved.entity)
                || combatants.contains_key(&saved.entity)
                || !players.contains_key(&saved.followed_player)
                || !saved.position.x.is_finite()
                || !saved.position.y.is_finite()
                || !saved.position.z.is_finite()
                || saved.active != registry.contains(saved.entity)
            {
                return Err(SaveError::InvalidState);
            }
            if let crate::companion::CompanionBehavior::Investigate(target) = saved.behavior {
                let known = match target {
                    KnowledgeTarget::Destination(destination) => restored_navigation.destination(destination).is_some(),
                    KnowledgeTarget::Enemy(enemy) => combatants.get(&enemy).is_some_and(|value| !value.is_player),
                    KnowledgeTarget::Item(item) => item_definitions.contains_key(&item),
                };
                let discovered = save
                    .knowledge
                    .iter()
                    .any(|(candidate, state)| *candidate == target && state.is_detected());
                if !known || !discovered {
                    return Err(SaveError::InvalidReference);
                }
            }
            if companions.insert(saved.entity, Companion {
                entity: saved.entity,
                followed_player: saved.followed_player,
                position: saved.position,
                behavior: saved.behavior,
                active: saved.active,
            }).is_some() {
                return Err(SaveError::DuplicateRecord);
            }
        }
        for mission_save in &save.missions {
            let mission = self
                .missions
                .get(&mission_save.id)
                .ok_or(SaveError::InvalidReference)?;
            let mut objective_ids = BTreeSet::new();
            for (objective_id, _, progress) in &mission_save.objectives {
                let objective = mission
                    .objectives()
                    .iter()
                    .find(|objective| objective.id == *objective_id)
                    .ok_or(SaveError::InvalidReference)?;
                if !objective_ids.insert(*objective_id) || *progress > objective.required_progress() {
                    return Err(SaveError::InvalidState);
                }
            }
        }
        for (target, _) in &save.knowledge {
            let valid = match target {
                KnowledgeTarget::Destination(destination) => restored_navigation.destination(*destination).is_some(),
                KnowledgeTarget::Enemy(enemy) => combatants.get(enemy).is_some_and(|combatant| !combatant.is_player),
                KnowledgeTarget::Item(item) => item_definitions.contains_key(item),
            };
            if !valid {
                return Err(SaveError::InvalidReference);
            }
        }
        if save.ship_fuel.len() != ship_resources.len()
            || save.ship_integrity.len() != ship_integrity.len()
            || save.currencies.len()
                != save
                    .currencies
                    .iter()
                    .map(|(entity, _)| *entity)
                    .collect::<BTreeSet<_>>()
                    .len()
        {
            return Err(SaveError::DuplicateRecord);
        }
        if save.currencies.iter().any(|(entity, _)| !players.contains_key(entity)) {
            return Err(SaveError::InvalidReference);
        }
        let mut enemy_rewards = BTreeMap::new();
        for reward in &save.enemy_rewards {
            reward.validate().map_err(|_| SaveError::InvalidState)?;
            if enemy_rewards.insert(reward.archetype, reward.clone()).is_some() {
                return Err(SaveError::DuplicateRecord);
            }
            if reward.entries.iter().any(|entry| !item_definitions.contains_key(&entry.item)) {
                return Err(SaveError::InvalidReference);
            }
        }
        let processed_enemy_rewards = save.processed_enemy_rewards.iter().copied().collect::<BTreeSet<_>>();
        if processed_enemy_rewards.len() != save.processed_enemy_rewards.len()
            || processed_enemy_rewards.iter().any(|enemy| !combatants.get(enemy).is_some_and(|combatant| !combatant.is_player && combatant.is_defeated()))
        { return Err(SaveError::InvalidState); }
        let mut hazards = BTreeMap::new();
        for hazard in &save.hazards {
            hazard.validate().map_err(|_| SaveError::InvalidState)?;
            if hazards.insert(hazard.id, *hazard).is_some() { return Err(SaveError::DuplicateRecord); }
        }
        let mut encounters = BTreeMap::new();
        for encounter in &save.encounters {
            encounter.validate().map_err(|_| SaveError::InvalidState)?;
            if encounters.insert(encounter.id, *encounter).is_some() { return Err(SaveError::DuplicateRecord); }
        }
        for encounter in encounters.values() {
            match encounter.kind {
                crate::encounter::SpatialEncounterKind::Signal { destination } if !restored_navigation.destinations().any(|value| value.id == destination) => return Err(SaveError::InvalidReference),
                crate::encounter::SpatialEncounterKind::Hazard { hazard } if !hazards.contains_key(&hazard) => return Err(SaveError::InvalidReference),
                crate::encounter::SpatialEncounterKind::Resource { item, .. } if !item_definitions.contains_key(&item) => return Err(SaveError::InvalidReference),
                _ => {}
            }
            if encounter.spawned_enemy.is_some_and(|enemy| !combatants.get(&enemy).is_some_and(|combatant| !combatant.is_player)) { return Err(SaveError::InvalidReference); }
        }
        let mut derelicts = BTreeMap::new();
        for derelict in &save.derelicts {
            derelict.validate().map_err(|_| SaveError::InvalidState)?;
            if derelict.rewards.iter().any(|reward| !item_definitions.contains_key(&reward.item)) { return Err(SaveError::InvalidReference); }
            if derelict.associated_encounter.is_some_and(|id| !encounters.contains_key(&id)) { return Err(SaveError::InvalidReference); }
            if derelict.spawned_enemy.is_some_and(|enemy| !combatants.get(&enemy).is_some_and(|combatant| !combatant.is_player)) { return Err(SaveError::InvalidReference); }
            if derelict.reward_collectibles.iter().any(|collectible| !save.collectibles.iter().any(|value| value.entity == *collectible)) { return Err(SaveError::InvalidReference); }
            if derelicts.insert(derelict.id, derelict.clone()).is_some() { return Err(SaveError::DuplicateRecord); }
        }
        let mut functional_worlds = BTreeMap::new();
        for functional_world in &save.functional_worlds {
            if !restored_navigation.destinations().any(|value| value.id == functional_world.destination) { return Err(SaveError::InvalidReference); }
            crate::persistence::validate_persisted_functional_world(functional_world)?;
            if functional_world.activities.iter().any(|activity| match activity { WorldActivity::Trade(market) => market.offers.iter().any(|offer| !item_definitions.contains_key(&offer.item)), WorldActivity::Resource(site) => !item_definitions.contains_key(&site.item), WorldActivity::Combat(zone) => zone.configuration.validate().is_err(), WorldActivity::Story(site) => !self.knowledge_target_exists(site.target) }) { return Err(SaveError::InvalidReference); }
            if functional_world.activities.iter().any(|activity| matches!(activity, WorldActivity::Combat(zone) if zone.active_enemy.is_some_and(|enemy| !combatants.get(&enemy).is_some_and(|combatant| !combatant.is_player)))) { return Err(SaveError::InvalidReference); }
            if functional_worlds.insert(functional_world.destination, functional_world.clone()).is_some() { return Err(SaveError::DuplicateRecord); }
        }
        self.entities = registry;
        self.item_definitions = item_definitions;
        self.state.time = save.time;
        self.state.phase = save.phase;
        self.players = players;
        self.companions = companions;
        self.inventories = inventories;
        self.combatants = combatants;
        self.enemy_rewards = enemy_rewards;
        self.processed_enemy_rewards = processed_enemy_rewards;
        self.environmental_hazards = hazards;
        self.spatial_encounters = encounters;
        self.derelicts = derelicts;
        self.functional_worlds = functional_worlds;
        self.currencies = save.currencies.into_iter().collect();
        self.ship_resources = ship_resources;
        self.ship_integrity = ship_integrity;
        self.collectibles = save
            .collectibles
            .into_iter()
            .map(|item| {
                (
                    item.entity,
                    Collectible {
                        entity: item.entity,
                        item: item.item,
                        amount: item.amount,
                        position: item.position,
                        interaction_range: item.interaction_range,
                        owner: item.owner,
                        active: item.active,
                    },
                )
            })
            .collect();
        for mission_save in save.missions {
            let mission = self.missions.get_mut(&mission_save.id).ok_or(SaveError::InvalidReference)?;
            mission.restore_state(mission_save.state);
            for (objective_id, state, progress) in mission_save.objectives {
                let objective = mission
                    .objectives_mut()
                    .iter_mut()
                    .find(|objective| objective.id == objective_id)
                    .ok_or(SaveError::InvalidReference)?;
                if !objective.restore_progress(state, progress) {
                    return Err(SaveError::InvalidState);
                }
            }
        }
        self.knowledge = save.knowledge.into_iter().collect();
        self.events = EventLog::default();
        self.navigation = restored_navigation;
        Ok(())
    }
    pub fn state(&self) -> GameplayState {
        self.state
    }
    pub fn time(&self) -> GameTime {
        self.state.time
    }
    pub fn phase(&self) -> SimulationPhase {
        self.state.phase
    }
    pub fn start(&mut self) {
        self.state.phase = SimulationPhase::Running;
    }
    pub fn entities(&self) -> &EntityRegistry {
        &self.entities
    }
    pub(crate) fn activate_entity(&mut self, entity: EntityId) -> bool {
        self.entities.activate(entity)
    }
    pub fn player(&self, entity: EntityId) -> Option<&Player> {
        self.players.get(&entity)
    }
    pub fn companions(&self) -> impl Iterator<Item = &Companion> {
        self.companions.values()
    }
    pub fn companion(&self, entity: EntityId) -> Option<&Companion> {
        self.companions.get(&entity)
    }
    pub fn create_companion(
        &mut self,
        followed_player: EntityId,
        position: engine::Vec3,
    ) -> Result<EntityId, CompanionError> {
        Companion::validate_position(position)?;
        let Some(player) = self.players.get(&followed_player) else {
            return Err(CompanionError::PlayerNotFound);
        };
        if !player.is_active || !self.entities.contains(followed_player) {
            return Err(CompanionError::PlayerInactive);
        }
        let entity = self.entities.create();
        self.companions.insert(entity, Companion::new(entity, followed_player, position));
        self.publish(crate::events::GameplayEvent::CompanionCreated { companion: entity, followed_player });
        Ok(entity)
    }
    pub fn set_companion_behavior(
        &mut self,
        entity: EntityId,
        behavior: CompanionBehavior,
    ) -> Result<(), CompanionError> {
        let Some(companion) = self.companions.get(&entity).copied() else {
            return Err(CompanionError::CompanionNotFound);
        };
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        let player_active = self.players.get(&companion.followed_player).is_some_and(|player| {
            player.is_active && self.entities.contains(companion.followed_player)
        });
        if !matches!(behavior, CompanionBehavior::Unavailable)
            && !player_active
        {
            return Err(CompanionError::PlayerInactive);
        }
        if let CompanionBehavior::Investigate(target) = behavior {
            if !self.knowledge(target).is_detected() {
                return Err(CompanionError::KnowledgeTargetUnavailable);
            }
        }
        if let Some(stored) = self.companions.get_mut(&entity) {
            stored.behavior = behavior;
        }
        self.publish(crate::events::GameplayEvent::CompanionBehaviorChanged {
            companion: entity,
            behavior,
        });
        Ok(())
    }
    pub fn companion_decision(&self, entity: EntityId) -> Result<CompanionBehavior, CompanionError> {
        let companion = self.companion(entity).ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if !self.players.get(&companion.followed_player).is_some_and(|player| {
            player.is_active && self.entities.contains(companion.followed_player)
        }) {
            return Ok(CompanionBehavior::Unavailable);
        }
        Ok(companion.behavior)
    }
    pub fn companion_recommended_behavior(
        &self,
        entity: EntityId,
    ) -> Result<CompanionBehavior, CompanionError> {
        let companion = self.companion(entity).ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if !self.players.get(&companion.followed_player).is_some_and(|player| {
            player.is_active && self.entities.contains(companion.followed_player)
        }) {
            return Ok(CompanionBehavior::Unavailable);
        }
        let guidance = self
            .guidance_for_player(companion.followed_player)
            .map_err(|_| CompanionError::GuidanceUnavailable)?;
        let recommendation = match guidance.recommended_action {
            Some(crate::guidance::GuidanceAction::StudyKnowledge(target)) => {
                Some(CompanionBehavior::Investigate(target))
            }
            Some(crate::guidance::GuidanceAction::IdentifySignal(Some(destination))) => {
                Some(CompanionBehavior::Investigate(KnowledgeTarget::Destination(destination)))
            }
            Some(crate::guidance::GuidanceAction::DefeatEnemy(Some(_))) => {
                Some(CompanionBehavior::CombatSupport)
            }
            _ => None,
        };
        Ok(recommendation.unwrap_or(CompanionBehavior::Accompany))
    }
    pub fn companion_action_recommendation(
        &self,
        entity: EntityId,
    ) -> Result<Option<CompanionRecommendation>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let guidance = self
            .guidance_for_player(companion.followed_player)
            .map_err(|_| CompanionError::GuidanceUnavailable)?;
        let Some(action) = guidance.recommended_action else {
            return Ok(None);
        };
        if !self.guidance_action_is_available(companion.followed_player, action) {
            return Ok(None);
        }
        Ok(Some(CompanionRecommendation {
            companion: entity,
            player: companion.followed_player,
            action,
            mode: CompanionActionMode::PlayerRequired,
        }))
    }
    pub fn companion_tactical_recommendation(
        &self,
        entity: EntityId,
    ) -> Result<Option<TacticalRecommendation>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let mut candidates = Vec::new();
        for enemy in self.enemies() {
            if !enemy.active || enemy.is_defeated() || !finite_vec3(enemy.position) {
                continue;
            }
            let Some(distance_squared) = crate::encounter::distance_squared(player.position, enemy.position) else {
                continue;
            };
            let priority = if distance_squared <= f64::from(enemy.weapon.range).powi(2) {
                TacticalPriority::Critical
            } else {
                TacticalPriority::High
            };
            candidates.push((
                tactical_priority_rank(priority),
                0_u8,
                enemy.entity.value(),
                TacticalRecommendation {
                    companion: entity,
                    player: companion.followed_player,
                    priority,
                    action: TacticalAction::DefeatEnemy(enemy.entity),
                },
            ));
        }
        if let Some(ship_position) = self.ship_position() {
            for hazard in self.active_environmental_hazards() {
                let Some(distance_squared) = crate::hazard::distance_squared(ship_position, hazard.position) else {
                    continue;
                };
                if distance_squared <= f64::from(hazard.radius).powi(2) {
                    candidates.push((
                        tactical_priority_rank(TacticalPriority::Critical),
                        1_u8,
                        u64::from(hazard.id.value()),
                        TacticalRecommendation {
                            companion: entity,
                            player: companion.followed_player,
                            priority: TacticalPriority::Critical,
                            action: TacticalAction::WithdrawFromHazard(hazard.id),
                        },
                    ));
                }
            }
            for encounter in self.spatial_encounters() {
                if !matches!(
                    encounter.state,
                    crate::encounter::SpatialEncounterState::Pending
                        | crate::encounter::SpatialEncounterState::Active
                ) {
                    continue;
                }
                let Some(distance_squared) = crate::encounter::distance_squared(ship_position, encounter.position) else {
                    continue;
                };
                if distance_squared > f64::from(encounter.activation_radius).powi(2) {
                    continue;
                }
                let priority = if encounter.state == crate::encounter::SpatialEncounterState::Active {
                    TacticalPriority::High
                } else {
                    TacticalPriority::Normal
                };
                candidates.push((
                    tactical_priority_rank(priority),
                    2_u8,
                    u64::from(encounter.id.value()),
                    TacticalRecommendation {
                        companion: entity,
                        player: companion.followed_player,
                        priority,
                        action: TacticalAction::AddressEncounter(encounter.id),
                    },
                ));
            }
        }
        if let Some(action) = self.companion_action_recommendation(entity)? {
            candidates.push((
                tactical_priority_rank(TacticalPriority::Normal),
                3_u8,
                0,
                TacticalRecommendation {
                    companion: entity,
                    player: companion.followed_player,
                    priority: TacticalPriority::Normal,
                    action: TacticalAction::PlayerAction(action.action),
                },
            ));
        }
        candidates.sort_by_key(|(priority, category, target, _)| (*priority, *category, *target));
        Ok(candidates.into_iter().next().map(|(_, _, _, recommendation)| recommendation))
    }
    pub fn companion_cooperative_tactical_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<CooperativeTacticalAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(followed_player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !followed_player.is_active
            || !self.entities.contains(followed_player.entity_id)
            || !self
                .combatant(followed_player.entity_id)
                .is_some_and(|combatant| combatant.active && !combatant.is_defeated())
        {
            return Ok(None);
        }
        let mut active_players = self
            .players()
            .filter(|player| {
                player.is_active
                    && self.entities.contains(player.entity_id)
                    && finite_vec3(player.position)
                    && self.combatant(player.entity_id).is_some_and(|combatant| {
                        combatant.is_player && combatant.active && !combatant.is_defeated()
                    })
            })
            .map(|player| player.entity_id)
            .collect::<Vec<_>>();
        active_players.sort_by_key(|player| player.value());
        let scaling = self.current_group_scaling();
        let mut threats = self
            .enemies()
            .filter(|enemy| enemy.active && !enemy.is_defeated() && finite_vec3(enemy.position))
            .map(|enemy| enemy.entity)
            .collect::<Vec<_>>();
        threats.sort_by_key(|enemy| enemy.value());
        let mut assigned_players = BTreeSet::new();
        let mut assignments = BTreeMap::new();
        let mut unassigned_threats = Vec::new();
        for threat in &threats {
            let Some(enemy) = self.combatant(*threat) else {
                unassigned_threats.push(*threat);
                continue;
            };
            let mut candidates = Vec::new();
            for player in &active_players {
                if assigned_players.contains(player) {
                    continue;
                }
                let Some(player_state) = self.player(*player) else {
                    continue;
                };
                let distance = crate::encounter::distance_squared(player_state.position, enemy.position);
                let Some(weapon) = self.equipped_weapon(*player) else {
                    candidates.push((
                        false,
                        distance.unwrap_or(f64::INFINITY),
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::MissingWeapon,
                        vec!["Arma equipada requerida".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: "El jugador no tiene un arma equipada".to_string(),
                            confirmed: true,
                        }],
                    ));
                    continue;
                };
                if weapon.validate().is_err() {
                    candidates.push((
                        false,
                        distance.unwrap_or(f64::INFINITY),
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::MissingWeapon,
                        vec!["Configuracion de arma valida requerida".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: "La configuracion del arma no es valida".to_string(),
                            confirmed: true,
                        }],
                    ));
                    continue;
                }
                let Some(distance) = distance else {
                    candidates.push((
                        false,
                        f64::INFINITY,
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::InvalidPosition,
                        vec!["Posiciones finitas requeridas".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: "No se pudo calcular una distancia segura".to_string(),
                            confirmed: true,
                        }],
                    ));
                    continue;
                };
                if distance > f64::from(weapon.range) * f64::from(weapon.range) {
                    candidates.push((
                        false,
                        distance,
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::OutOfRange,
                        vec!["Alcance del arma requerido".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: format!("Distancia cuadrada {distance} fuera del alcance"),
                            confirmed: true,
                        }],
                    ));
                    continue;
                }
                if let Some(ammunition) = weapon.ammunition {
                    let available = self
                        .inventory(*player)
                        .map_or(0, |inventory| inventory.quantity(ammunition.item));
                    if available < ammunition.amount_per_attack {
                        candidates.push((
                            false,
                            distance,
                            *player,
                            TacticalAssignmentStatus::Blocked,
                            TacticalAssignmentReason::MissingAmmunition,
                            vec![format!(
                                "Municion {:?}: requiere {}, disponible {}",
                                ammunition.item, ammunition.amount_per_attack, available
                            )],
                            vec![TacticalAssignmentEvidence {
                                description: "La municion disponible no alcanza para un ataque".to_string(),
                                confirmed: true,
                            }],
                        ));
                        continue;
                    }
                }
                if self.combatant(*player).is_some_and(|combatant| {
                    combatant.last_attack_at.is_some_and(|last| {
                        self.time().elapsed.0.saturating_sub(last.0) < weapon.cooldown.0
                    })
                }) {
                    candidates.push((
                        false,
                        distance,
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::AttackCooldown,
                        vec!["Enfriamiento de ataque pendiente".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: "El comando de ataque rechazaria la accion por cooldown".to_string(),
                            confirmed: true,
                        }],
                    ));
                    continue;
                }
                if !self.player_weapon_ready(*player) {
                    candidates.push((
                        false,
                        distance,
                        *player,
                        TacticalAssignmentStatus::Blocked,
                        TacticalAssignmentReason::MissingAmmunition,
                        vec!["El arma no esta lista para atacar".to_string()],
                        vec![TacticalAssignmentEvidence {
                            description: "La validacion publica de preparacion de arma rechazo la accion".to_string(),
                            confirmed: true,
                        }],
                    ));
                    continue;
                }
                candidates.push((
                    self.player_weapon_ready(*player),
                    distance,
                    *player,
                    TacticalAssignmentStatus::Actionable,
                    TacticalAssignmentReason::AssignedByDistance,
                    Vec::new(),
                    vec![TacticalAssignmentEvidence {
                        description: "Jugador activo, armado, dentro de alcance y con municion valida".to_string(),
                        confirmed: true,
                    }],
                ));
            }
            let Some((best_index, best)) = candidates.iter().enumerate().max_by(
                |(_, left), (_, right)| {
                    left.0
                        .cmp(&right.0)
                        .then_with(|| {
                            right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| right.2.value().cmp(&left.2.value()))
                },
            ) else {
                unassigned_threats.push(*threat);
                continue;
            };
            let tie_break_used = best.0
                && candidates.iter().enumerate().any(|(index, candidate)| {
                    index != best_index && candidate.0 && candidate.1 == best.1
                });
            let (actionable, _distance, player, status, mut reason, missing, evidence) =
                candidates.remove(best_index);
            if tie_break_used {
                reason = TacticalAssignmentReason::AssignedByStableTieBreak;
            }
            assigned_players.insert(player);
            assignments.insert(
                player,
                PlayerTacticalRecommendation {
                    player,
                    threat: Some(*threat),
                    action: Some(TacticalAction::DefeatEnemy(*threat)),
                    status,
                    reason,
                    preferred: actionable,
                    evidence,
                    missing_requirements: missing,
                },
            );
            if !actionable {
                unassigned_threats.push(*threat);
            }
        }
        let recommendations = active_players
            .iter()
            .map(|player| {
                assignments.remove(player).unwrap_or_else(|| PlayerTacticalRecommendation {
                    player: *player,
                    threat: None,
                    action: None,
                    status: if threats.is_empty() {
                        TacticalAssignmentStatus::NotEvaluable
                    } else {
                        TacticalAssignmentStatus::NotEvaluable
                    },
                    reason: if threats.is_empty() {
                        TacticalAssignmentReason::NoThreatAvailable
                    } else {
                        TacticalAssignmentReason::NoAvailablePlayer
                    },
                    preferred: false,
                    evidence: vec![TacticalAssignmentEvidence {
                        description: "No hay una tarea tactica adicional asignada a este jugador".to_string(),
                        confirmed: !threats.is_empty(),
                    }],
                    missing_requirements: Vec::new(),
                })
            })
            .collect::<Vec<_>>();
        let conclusive = !threats.is_empty() && unassigned_threats.is_empty();
        Ok(Some(CooperativeTacticalAssessment {
            companion: entity,
            followed_player: companion.followed_player,
            active_players,
            scaling,
            threats,
            conclusive,
            recommendations,
            unassigned_threats,
        }))
    }
    pub fn companion_cooperative_support_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<CooperativeSupportAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(followed_player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !followed_player.is_active
            || !self.entities.contains(followed_player.entity_id)
            || !self
                .combatant(followed_player.entity_id)
                .is_some_and(|combatant| combatant.is_player && combatant.active && !combatant.is_defeated())
        {
            return Ok(None);
        }

        let mut active_players = self
            .players()
            .filter(|player| {
                player.is_active
                    && self.entities.contains(player.entity_id)
                    && finite_vec3(player.position)
                    && self.combatant(player.entity_id).is_some_and(|combatant| {
                        combatant.is_player && combatant.active && !combatant.is_defeated()
                    })
            })
            .map(|player| player.entity_id)
            .collect::<Vec<_>>();
        active_players.sort_by_key(|player| player.value());

        let mut threats = self
            .enemies()
            .filter(|enemy| enemy.active && !enemy.is_defeated() && finite_vec3(enemy.position))
            .map(|enemy| enemy.entity)
            .collect::<Vec<_>>();
        threats.sort_by_key(|enemy| enemy.value());
        if threats.is_empty() {
            return Ok(Some(CooperativeSupportAssessment {
                companion: entity,
                followed_player: companion.followed_player,
                active_players,
                threats,
                status: SupportRecommendationStatus::NotEvaluable,
                recommendations: Vec::new(),
                conclusive: false,
            }));
        }

        let mut recommendations = Vec::new();
        for needy_player in &active_players {
            let Some(weapon) = self.equipped_weapon(*needy_player) else {
                continue;
            };
            let Some(ammunition) = weapon.ammunition else {
                continue;
            };
            let Some(available) = self
                .inventory(*needy_player)
                .map(|inventory| inventory.quantity(ammunition.item))
            else {
                recommendations.push(PlayerSupportRecommendation {
                    needy_player: *needy_player,
                    provider: None,
                    item: Some(ammunition.item),
                    amount: ammunition.amount_per_attack,
                    blocked_action: Some(TacticalAction::DefeatEnemy(threats[0])),
                    status: SupportRecommendationStatus::Blocked,
                    reason: SupportRecommendationReason::ReceiverUnavailable,
                    evidence: vec![SupportEvidence {
                        description: "El inventario del jugador no esta inicializado".to_string(),
                        confirmed: true,
                    }],
                    missing_requirements: vec!["Inventario inicializado".to_string()],
                });
                continue;
            };
            if available >= ammunition.amount_per_attack {
                continue;
            }
            if weapon.validate().is_err() {
                continue;
            }
            let Some(player_state) = self.player(*needy_player) else {
                continue;
            };
            let in_range = threats.iter().any(|threat| {
                self.combatant(*threat).is_some_and(|enemy| {
                    crate::encounter::distance_squared(player_state.position, enemy.position)
                        .is_some_and(|distance| {
                            distance <= f64::from(weapon.range) * f64::from(weapon.range)
                        })
                })
            });
            if !in_range {
                continue;
            }
            let needed = ammunition.amount_per_attack - available;
            let threat = threats[0];
            let Some(definition) = self.item_definition(ammunition.item) else {
                recommendations.push(PlayerSupportRecommendation {
                    needy_player: *needy_player,
                    provider: None,
                    item: Some(ammunition.item),
                    amount: needed,
                    blocked_action: Some(TacticalAction::DefeatEnemy(threat)),
                    status: SupportRecommendationStatus::Blocked,
                    reason: SupportRecommendationReason::ItemDefinitionMissing,
                    evidence: vec![SupportEvidence {
                        description: "La municion equipada no tiene una definicion registrada".to_string(),
                        confirmed: true,
                    }],
                    missing_requirements: vec!["Definicion de objeto valida".to_string()],
                });
                continue;
            };
            let Some(receiver) = self.inventory(*needy_player) else {
                continue;
            };
            if needed > definition.max_stack
                || receiver.quantity(ammunition.item).checked_add(needed).is_none_or(|quantity| quantity > definition.max_stack)
            {
                recommendations.push(PlayerSupportRecommendation {
                    needy_player: *needy_player,
                    provider: None,
                    item: Some(ammunition.item),
                    amount: needed,
                    blocked_action: Some(TacticalAction::DefeatEnemy(threat)),
                    status: SupportRecommendationStatus::Blocked,
                    reason: SupportRecommendationReason::StackExceeded,
                    evidence: vec![SupportEvidence {
                        description: "La cantidad requerida excede el limite de pila del objeto".to_string(),
                        confirmed: true,
                    }],
                    missing_requirements: vec!["Espacio de pila suficiente".to_string()],
                });
                continue;
            }
            if receiver.remaining_capacity() < needed {
                recommendations.push(PlayerSupportRecommendation {
                    needy_player: *needy_player,
                    provider: None,
                    item: Some(ammunition.item),
                    amount: needed,
                    blocked_action: Some(TacticalAction::DefeatEnemy(threat)),
                    status: SupportRecommendationStatus::Blocked,
                    reason: SupportRecommendationReason::ReceiverCapacityExceeded,
                    evidence: vec![SupportEvidence {
                        description: "La capacidad libre del receptor no alcanza la transferencia".to_string(),
                        confirmed: true,
                    }],
                    missing_requirements: vec!["Capacidad libre en el inventario receptor".to_string()],
                });
                continue;
            }

            let mut providers = active_players
                .iter()
                .filter(|provider| **provider != *needy_player)
                .filter_map(|provider| {
                    let inventory = self.inventory(*provider)?;
                    let quantity = inventory.quantity(ammunition.item);
                    if quantity < needed || needed > definition.max_stack {
                        return None;
                    }
                    let provider_can_remain_ready = self
                        .equipped_weapon(*provider)
                        .and_then(|weapon| weapon.ammunition)
                        .is_none_or(|provider_ammunition| {
                            provider_ammunition.item != ammunition.item
                                || quantity - needed >= provider_ammunition.amount_per_attack
                        });
                    if !provider_can_remain_ready {
                        return None;
                    }
                    Some((*provider, quantity))
                })
                .collect::<Vec<_>>();
            providers.sort_by(|left, right| {
                right.1.cmp(&left.1).then_with(|| left.0.value().cmp(&right.0.value()))
            });
            let Some((provider, quantity)) = providers.first().copied() else {
                let has_quantity = active_players.iter().any(|candidate| {
                    *candidate != *needy_player
                        && self.inventory(*candidate).is_some_and(|inventory| {
                            inventory.quantity(ammunition.item) > 0
                        })
                });
                recommendations.push(PlayerSupportRecommendation {
                    needy_player: *needy_player,
                    provider: None,
                    item: Some(ammunition.item),
                    amount: needed,
                    blocked_action: Some(TacticalAction::DefeatEnemy(threat)),
                    status: SupportRecommendationStatus::Blocked,
                    reason: if has_quantity {
                        SupportRecommendationReason::ProviderInsufficientQuantity
                    } else {
                        SupportRecommendationReason::NoProvider
                    },
                    evidence: vec![SupportEvidence {
                        description: if has_quantity {
                            "Hay companeros activos, pero ninguno puede cubrir la cantidad requerida".to_string()
                        } else {
                            "No hay un companero activo con la municion requerida".to_string()
                        },
                        confirmed: true,
                    }],
                    missing_requirements: vec!["Transferencia completa de municion".to_string()],
                });
                continue;
            };
            recommendations.push(PlayerSupportRecommendation {
                needy_player: *needy_player,
                provider: Some(provider),
                item: Some(ammunition.item),
                amount: needed,
                blocked_action: Some(TacticalAction::DefeatEnemy(threat)),
                status: SupportRecommendationStatus::Actionable,
                reason: SupportRecommendationReason::ProviderAvailable,
                evidence: vec![
                    SupportEvidence {
                        description: format!("El proveedor tiene {quantity} unidades disponibles"),
                        confirmed: true,
                    },
                    SupportEvidence {
                        description: "La cantidad respeta capacidad, pila y definicion del objeto".to_string(),
                        confirmed: true,
                    },
                ],
                missing_requirements: Vec::new(),
            });
        }
        let status = if recommendations
            .iter()
            .any(|recommendation| recommendation.status == SupportRecommendationStatus::Actionable)
        {
            SupportRecommendationStatus::Actionable
        } else if recommendations.is_empty() {
            SupportRecommendationStatus::Informational
        } else {
            SupportRecommendationStatus::Blocked
        };
        Ok(Some(CooperativeSupportAssessment {
            companion: entity,
            followed_player: companion.followed_player,
            active_players,
            threats,
            status,
            conclusive: true,
            recommendations,
        }))
    }
    pub fn companion_cooperative_encounter_readiness(
        &self,
        entity: EntityId,
    ) -> Result<Option<CooperativeEncounterReadinessAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active
            || !self.entities.contains(player.entity_id)
            || !self
                .combatant(player.entity_id)
                .is_some_and(|combatant| combatant.is_player && combatant.active && !combatant.is_defeated())
        {
            return Ok(None);
        }
        let Some(tactical) = self.companion_cooperative_tactical_assessment(entity)? else {
            return Ok(None);
        };
        if tactical.threats.is_empty() {
            return Ok(Some(CooperativeEncounterReadinessAssessment {
                companion: entity,
                followed_player: companion.followed_player,
                active_players: tactical.active_players,
                threats: Vec::new(),
                status: CooperativeEncounterReadinessStatus::NotEvaluable,
                satisfied_requirements: Vec::new(),
                missing_requirements: Vec::new(),
                blockers: Vec::new(),
                support_opportunities: Vec::new(),
                actions: Vec::new(),
                evidence: vec![SupportEvidence {
                    description: "No hay amenazas activas que evaluar".to_string(),
                    confirmed: true,
                }],
                conclusive: false,
            }));
        }

        let preparation = self.companion_preparation_assessment(entity)?;
        let support = self.companion_cooperative_support_assessment(entity)?;
        let mut satisfied_requirements = Vec::new();
        let mut missing_requirements = Vec::new();
        let mut blockers = Vec::new();
        let mut support_opportunities = Vec::new();
        let mut actions = Vec::new();
        let mut evidence = Vec::new();
        if let Some(preparation) = preparation {
            let mut has_real_missing_requirement = false;
            for requirement in preparation.satisfied_requirements {
                satisfied_requirements.push(ReadinessRequirement {
                    player: preparation.player,
                    requirement,
                    satisfied: true,
                });
            }
            for requirement in preparation.missing_requirements {
                if matches!(
                    requirement,
                    PreparationRequirement::Ammunition { item: None, .. }
                ) {
                    continue;
                }
                has_real_missing_requirement = true;
                missing_requirements.push(ReadinessRequirement {
                    player: preparation.player,
                    requirement,
                    satisfied: false,
                });
                blockers.push(ReadinessBlocker {
                    player: preparation.player,
                    threat: match preparation.action {
                        Some(PreparationAction::PrepareCombat(enemy)) => Some(enemy),
                        _ => None,
                    },
                    requirement: Some(requirement),
                    description: format!("Requisito de preparacion faltante: {requirement:?}"),
                    support: None,
                    evidence: vec![SupportEvidence {
                        description: format!("La evaluacion de preparacion informo {:?}", preparation.reason),
                        confirmed: true,
                    }],
                });
            }
            if has_real_missing_requirement {
                if let Some(action) = preparation.action {
                    actions.push(CooperativePreparationAction::Prepare(action));
                }
            }
        }

        for recommendation in &tactical.recommendations {
            match recommendation.status {
                TacticalAssignmentStatus::Actionable => {
                    if let Some(action) = recommendation.action {
                        actions.push(CooperativePreparationAction::ExecuteTactical(action));
                    }
                    evidence.push(SupportEvidence {
                        description: format!(
                            "El jugador {:?} puede ejecutar la tarea tactica {:?}",
                            recommendation.player, recommendation.action
                        ),
                        confirmed: true,
                    });
                }
                TacticalAssignmentStatus::Blocked => {
                    let requirement = if recommendation.reason == TacticalAssignmentReason::MissingAmmunition {
                        self.equipped_weapon(recommendation.player)
                            .and_then(|weapon| weapon.ammunition)
                            .map(|ammunition| PreparationRequirement::Ammunition {
                                item: Some(ammunition.item),
                                required: ammunition.amount_per_attack,
                                available: self
                                    .inventory(recommendation.player)
                                    .map_or(0, |inventory| inventory.quantity(ammunition.item)),
                            })
                    } else {
                        None
                    };
                    blockers.push(ReadinessBlocker {
                        player: recommendation.player,
                        threat: recommendation.threat,
                        requirement,
                        description: format!("La tarea tactica esta bloqueada por {:?}", recommendation.reason),
                        support: None,
                        evidence: recommendation
                            .evidence
                            .iter()
                            .map(|item| SupportEvidence {
                                description: item.description.clone(),
                                confirmed: item.confirmed,
                            })
                            .collect(),
                    });
                    if let Some(requirement) = requirement {
                        missing_requirements.push(ReadinessRequirement {
                            player: recommendation.player,
                            requirement,
                            satisfied: false,
                        });
                    }
                }
                TacticalAssignmentStatus::NotEvaluable => {
                    evidence.push(SupportEvidence {
                        description: format!(
                            "No se asigno una tarea adicional a {:?}: {:?}",
                            recommendation.player, recommendation.reason
                        ),
                        confirmed: false,
                    });
                }
            }
        }

        if let Some(support) = support {
            for recommendation in support.recommendations {
                if recommendation.status == SupportRecommendationStatus::Actionable {
                    if let (Some(provider), Some(item)) = (recommendation.provider, recommendation.item) {
                        actions.push(CooperativePreparationAction::TransferInventory {
                            from: provider,
                            to: recommendation.needy_player,
                            item,
                            amount: recommendation.amount,
                        });
                    }
                    support_opportunities.push(recommendation.clone());
                    if let Some(blocker) = blockers.iter_mut().find(|blocker| {
                        blocker.player == recommendation.needy_player
                            && blocker.threat == recommendation.blocked_action.and_then(|action| match action {
                                TacticalAction::DefeatEnemy(enemy) => Some(enemy),
                                _ => None,
                            })
                    }) {
                        blocker.support = Some(recommendation);
                    }
                } else if recommendation.status == SupportRecommendationStatus::Blocked {
                    if !blockers.iter().any(|blocker| {
                        blocker.player == recommendation.needy_player
                            && blocker.requirement.is_some_and(|requirement| {
                                requirement == PreparationRequirement::Ammunition {
                                    item: recommendation.item,
                                    required: recommendation.amount,
                                    available: self
                                        .inventory(recommendation.needy_player)
                                        .map_or(0, |inventory| {
                                            recommendation.item.map_or(0, |item| inventory.quantity(item))
                                        }),
                                }
                            })
                    }) {
                        blockers.push(ReadinessBlocker {
                            player: recommendation.needy_player,
                            threat: recommendation.blocked_action.and_then(|action| match action {
                                TacticalAction::DefeatEnemy(enemy) => Some(enemy),
                                _ => None,
                            }),
                            requirement: recommendation.item.map(|item| PreparationRequirement::Ammunition {
                                item: Some(item),
                                required: recommendation.amount,
                                available: self
                                    .inventory(recommendation.needy_player)
                                    .map_or(0, |inventory| inventory.quantity(item)),
                            }),
                            description: format!("El apoyo no puede cubrir el bloqueo: {:?}", recommendation.reason),
                            support: None,
                            evidence: recommendation
                                .evidence
                                .iter()
                                .map(|item| SupportEvidence {
                                    description: item.description.clone(),
                                    confirmed: item.confirmed,
                                })
                                .collect(),
                        });
                    }
                }
            }
        }
        let status = if blockers.is_empty() {
            if tactical
                .recommendations
                .iter()
                .any(|recommendation| recommendation.status == TacticalAssignmentStatus::NotEvaluable)
            {
                CooperativeEncounterReadinessStatus::PreparedWithReservations
            } else {
                CooperativeEncounterReadinessStatus::Prepared
            }
        } else {
            CooperativeEncounterReadinessStatus::Blocked
        };
        Ok(Some(CooperativeEncounterReadinessAssessment {
            companion: entity,
            followed_player: companion.followed_player,
            active_players: tactical.active_players,
            threats: tactical.threats,
            status,
            satisfied_requirements,
            missing_requirements,
            blockers,
            support_opportunities,
            actions,
            evidence,
            conclusive: true,
        }))
    }
    pub fn companion_navigation_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<NavigationAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }

        let autopilot = self.autopilot_state().clone();
        let travel = self.travel_state();
        let (destination, route, eta, route_start) = match &autopilot {
            crate::navigation::AutopilotState::Planned {
                destination,
                route,
                eta,
                ..
            } => (Some(*destination), route.clone(), Some(*eta), 0),
            crate::navigation::AutopilotState::InTransit {
                destination,
                route,
                next_leg,
                eta,
                ..
            } => (Some(*destination), route.clone(), Some(*eta), *next_leg),
            crate::navigation::AutopilotState::Completed { destination }
            | crate::navigation::AutopilotState::Interrupted { destination }
            | crate::navigation::AutopilotState::Failed { destination } => {
                (Some(*destination), Vec::new(), None, 0)
            }
            crate::navigation::AutopilotState::Inactive => (None, Vec::new(), None, 0),
        };
        let fuel_available = self
            .navigation()
            .ship_id()
            .and_then(|ship| self.ship_fuel(ship))
            .map(|fuel| fuel.current());
        let fuel_required = if route.is_empty() {
            None
        } else {
            route_fuel_requirement(self, &route, route_start)
        };
        let mut risks = Vec::new();
        let current_position = self.ship_position();
        let destination_position = destination.and_then(|id| self.destination(id)).map(|value| value.position);
        for hazard in self.active_environmental_hazards() {
            if [current_position, destination_position]
                .into_iter()
                .flatten()
                .any(|position| within_radius(position, hazard.position, hazard.radius))
            {
                risks.push(NavigationRisk::KnownHazard(hazard.id));
            }
        }
        for encounter in self.spatial_encounters() {
            if encounter.state == crate::encounter::SpatialEncounterState::Resolved {
                continue;
            }
            if [current_position, destination_position]
                .into_iter()
                .flatten()
                .any(|position| within_radius(position, encounter.position, encounter.activation_radius))
            {
                risks.push(NavigationRisk::KnownEncounter(encounter.id));
            }
        }
        risks.sort_by_key(|risk| match risk {
            NavigationRisk::InsufficientFuel { .. } => (0_u8, 0_u64),
            NavigationRisk::KnownHazard(id) => (1_u8, u64::from(id.value())),
            NavigationRisk::KnownEncounter(id) => (2_u8, u64::from(id.value())),
        });
        if let (Some(required), Some(available)) = (fuel_required, fuel_available) {
            if required > available {
                risks.insert(
                    0,
                    NavigationRisk::InsufficientFuel {
                        required,
                        available,
                    },
                );
            }
        }
        let mut route_risks = Vec::new();
        if !route.is_empty() {
            for hazard in self.environmental_hazards() {
                route_risks.push(classify_hazard_on_route(self, &route, hazard));
            }
            for encounter in self.spatial_encounters() {
                route_risks.push(classify_encounter_on_route(self, &route, encounter));
            }
        }
        route_risks.sort_by_key(navigation_route_risk_sort_key);
        for route_risk in &route_risks {
            let is_near = matches!(
                route_risk.classification,
                NavigationRouteRiskClassification::NearSegment { .. }
            );
            if !is_near {
                continue;
            }
            match route_risk.source {
                NavigationRouteRiskSource::Hazard(id)
                    if !risks.iter().any(|risk| *risk == NavigationRisk::KnownHazard(id)) =>
                {
                    risks.push(NavigationRisk::KnownHazard(id));
                }
                NavigationRouteRiskSource::Encounter(id)
                    if !risks.iter().any(|risk| *risk == NavigationRisk::KnownEncounter(id)) =>
                {
                    risks.push(NavigationRisk::KnownEncounter(id));
                }
                _ => {}
            }
        }
        risks.sort_by_key(|risk| match risk {
            NavigationRisk::InsufficientFuel { .. } => (0_u8, 0_u64),
            NavigationRisk::KnownHazard(id) => (1_u8, u64::from(id.value())),
            NavigationRisk::KnownEncounter(id) => (2_u8, u64::from(id.value())),
        });
        let recommendation_segment = risks.first().and_then(|risk| {
            let source = match risk {
                NavigationRisk::KnownHazard(id) => NavigationRouteRiskSource::Hazard(*id),
                NavigationRisk::KnownEncounter(id) => NavigationRouteRiskSource::Encounter(*id),
                NavigationRisk::InsufficientFuel { .. } => return None,
            };
            route_risks.iter().find_map(|value| {
                if value.source != source {
                    return None;
                }
                match value.classification {
                    NavigationRouteRiskClassification::NearSegment { segment, .. } => Some(segment),
                    _ => None,
                }
            })
        });
        let recommendation = match autopilot {
            crate::navigation::AutopilotState::Planned { destination, .. } => {
                if fuel_required.zip(fuel_available).is_some_and(|(required, available)| required > available) {
                    Some(NavigationAdvice::ReplanAutopilot { destination })
                } else if let Some(NavigationRisk::KnownHazard(id)) = risks.first() {
                    Some(NavigationAdvice::InspectHazard(*id))
                } else if let Some(NavigationRisk::KnownEncounter(id)) = risks.first() {
                    Some(NavigationAdvice::AddressEncounter(*id))
                } else if fuel_required.zip(fuel_available).is_some() {
                    Some(NavigationAdvice::StartPlannedAutopilot { destination })
                } else {
                    None
                }
            }
            crate::navigation::AutopilotState::InTransit { destination, .. } => {
                if fuel_required.zip(fuel_available).is_some_and(|(required, available)| required > available) {
                    Some(NavigationAdvice::ReplanAutopilot { destination })
                } else if let Some(NavigationRisk::KnownHazard(id)) = risks.first() {
                    Some(NavigationAdvice::InspectHazard(*id))
                } else if let Some(NavigationRisk::KnownEncounter(id)) = risks.first() {
                    Some(NavigationAdvice::AddressEncounter(*id))
                } else {
                    Some(NavigationAdvice::MonitorAutopilot { destination })
                }
            }
            crate::navigation::AutopilotState::Interrupted { destination }
            | crate::navigation::AutopilotState::Failed { destination } => {
                Some(NavigationAdvice::ReplanAutopilot { destination })
            }
            crate::navigation::AutopilotState::Inactive
            | crate::navigation::AutopilotState::Completed { .. } => None,
        };
        Ok(Some(NavigationAssessment {
            companion: entity,
            player: companion.followed_player,
            autopilot,
            travel,
            destination,
            route,
            eta,
            fuel_available,
            fuel_required,
            risks,
            route_risks,
            recommendation_segment,
            recommendation,
        }))
    }
    pub fn companion_navigation_comparison(
        &self,
        entity: EntityId,
    ) -> Result<Option<NavigationRouteComparison>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let (origin, destination, current_route, in_transit) = match self.autopilot_state() {
            crate::navigation::AutopilotState::Planned {
                origin,
                destination,
                route,
                ..
            } => (*origin, *destination, route.clone(), false),
            crate::navigation::AutopilotState::InTransit {
                origin,
                destination,
                route,
                ..
            } => (*origin, *destination, route.clone(), true),
            crate::navigation::AutopilotState::Inactive
            | crate::navigation::AutopilotState::Completed { .. }
            | crate::navigation::AutopilotState::Interrupted { .. }
            | crate::navigation::AutopilotState::Failed { .. } => return Ok(None),
        };
        let fuel_available = self
            .navigation()
            .ship_id()
            .and_then(|ship| self.ship_fuel(ship))
            .map(|fuel| fuel.current());
        let ship_operational = self
            .navigation()
            .ship_id()
            .and_then(|ship| self.ship_is_operational(ship))
            .unwrap_or(false);
        let current = summarize_route(self, &current_route, fuel_available, ship_operational)
            .ok_or(CompanionError::GuidanceUnavailable)?;
        let mut paths = Vec::new();
        let mut path = vec![origin];
        let mut visited = BTreeSet::from([origin]);
        enumerate_navigation_paths(
            self,
            destination,
            &mut path,
            &mut visited,
            self.destinations().count(),
            &mut paths,
        );
        let mut alternatives = paths
            .into_iter()
            .filter(|route| route.as_slice() != current_route.as_slice())
            .filter_map(|route| summarize_route(self, &route, fuel_available, ship_operational))
            .collect::<Vec<_>>();
        alternatives.sort_by(compare_route_options);
        let recommendation = if in_transit || current.active_risks == 0 || !current.viable {
            None
        } else {
            alternatives
                .iter()
                .find(|candidate| candidate.viable && route_is_better(candidate, &current))
                .and_then(|alternative| build_alternative_recommendation(&current, alternative))
        };
        Ok(Some(NavigationRouteComparison {
            current,
            alternatives,
            recommendation,
        }))
    }
    pub fn companion_operational_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<OperationalAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let ship = self.navigation().ship_id();
        let Some(ship) = ship else {
            return Ok(None);
        };
        let Some(integrity) = self.ship_integrity(ship) else {
            return Ok(None);
        };
        let Some(fuel) = self.ship_fuel(ship) else {
            return Ok(None);
        };
        let navigation = self.companion_navigation_assessment(entity)?;
        let route_comparison = self.companion_navigation_comparison(entity)?;
        let current_integrity = integrity.current();
        let maximum_integrity = integrity.maximum();
        let hull = if integrity.is_destroyed() {
            HullCondition::Destroyed
        } else if integrity.is_critical() {
            HullCondition::Critical
        } else if current_integrity < maximum_integrity {
            HullCondition::Reduced
        } else {
            HullCondition::Normal
        };
        let fuel_required = route_comparison
            .as_ref()
            .map(|comparison| comparison.current.fuel_required)
            .or_else(|| navigation.as_ref().and_then(|assessment| assessment.fuel_required));
        let fuel_available = fuel.current();
        let fuel_condition = match fuel_required {
            Some(required) if fuel_available < required => FuelCondition::Insufficient,
            Some(required) if fuel_available == required => FuelCondition::Tight,
            Some(_) => FuelCondition::Sufficient,
            None => FuelCondition::Unknown,
        };
        let recommendation = if hull == HullCondition::Destroyed {
            Some(MaintenanceRecommendation::AvoidShipActivity { ship })
        } else if fuel_condition == FuelCondition::Insufficient {
            Some(MaintenanceRecommendation::ManageFuel {
                ship,
                required: fuel_required.unwrap_or_default(),
                available: fuel_available,
            })
        } else if hull == HullCondition::Critical {
            Some(MaintenanceRecommendation::RepairShip { ship })
        } else if hull == HullCondition::Reduced
            && route_comparison
                .as_ref()
                .is_some_and(|comparison| comparison.current.critical_risks > 0)
        {
            Some(MaintenanceRecommendation::RepairShip { ship })
        } else if fuel_condition == FuelCondition::Tight {
            Some(MaintenanceRecommendation::ManageFuel {
                ship,
                required: fuel_required.unwrap_or_default(),
                available: fuel_available,
            })
        } else {
            None
        };
        let priority = if hull == HullCondition::Destroyed
            || fuel_condition == FuelCondition::Insufficient
            || hull == HullCondition::Critical
        {
            MaintenancePriority::Critical
        } else if recommendation.is_some() {
            MaintenancePriority::High
        } else {
            MaintenancePriority::Normal
        };
        Ok(Some(OperationalAssessment {
            companion: entity,
            player: companion.followed_player,
            ship,
            hull,
            integrity: current_integrity,
            maximum_integrity,
            critical_threshold: integrity.critical_threshold(),
            fuel: fuel_condition,
            fuel_available: Some(fuel_available),
            fuel_required,
            navigation,
            route_comparison,
            priority,
            recommendation,
        }))
    }
    pub fn companion_exploration_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<ExplorationAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let ship = self.navigation().ship_id();
        let ship_operational = ship
            .and_then(|value| self.ship_is_operational(value))
            .unwrap_or(false);
        let fuel = ship.and_then(|value| self.ship_fuel(value)).map(|value| value.current());
        let current_position = self.navigation().current_position();
        let route_is_viable = |destination: DestinationId| {
            let Some(origin) = self
                .destinations()
                .find(|value| value.position == current_position)
                .map(|value| value.id)
            else {
                return false;
            };
            let Some(route) = self.navigation().route(origin, destination) else {
                return false;
            };
            route.state == crate::navigation::NavigationRouteState::Available
                && ship_operational
                && fuel.is_some_and(|available| available >= route.fuel_cost)
        };
        let player_can_reach = |position: engine::Vec3, range: f32| {
            finite_vec3(player.position)
                && finite_vec3(position)
                && range.is_finite()
                && range > 0.0
                && crate::encounter::distance_squared(player.position, position)
                    .is_some_and(|distance| distance <= f64::from(range) * f64::from(range))
        };
        let guidance = self
            .guidance_for_player(companion.followed_player)
            .map_err(|_| CompanionError::GuidanceUnavailable)?;
        let mut recommendations = Vec::new();
        if let Some(current_id) = guidance.current_objective {
            if let Some(objective) = guidance.objectives.iter().find(|value| value.id == current_id) {
                if let Some(action) = objective.recommended_action {
                    let destination = match action {
                        GuidanceAction::TravelTo(value)
                        | GuidanceAction::IdentifySignal(Some(value))
                        | GuidanceAction::DiscoverDestination(value) => Some(value),
                        _ => None,
                    };
                    let accessible = match action {
                        GuidanceAction::TravelTo(value) => {
                            self.destination(value).is_some_and(|destination| {
                                destination.discovery == crate::navigation::DiscoveryState::Discovered
                            }) && route_is_viable(value)
                        }
                        GuidanceAction::DiscoverDestination(value) => self.destination(value).is_some(),
                        GuidanceAction::IdentifySignal(Some(value)) => {
                            self.destination(value).is_some()
                                && self.guidance_action_is_available(companion.followed_player, action)
                        }
                        GuidanceAction::RepairShip => ship_operational,
                        GuidanceAction::CompleteObjective(_) => false,
                        _ => self.guidance_action_is_available(companion.followed_player, action),
                    };
                    if accessible && action != GuidanceAction::CompleteObjective(current_id) {
                        let mut fulfilled = vec![GuidanceRequirement::MissionActive(objective.mission)];
                        if let Some(destination) = destination {
                            if self.destination(destination).is_some_and(|value| {
                                value.discovery == crate::navigation::DiscoveryState::Discovered
                            }) {
                                fulfilled.push(GuidanceRequirement::DestinationDiscovered(destination));
                            }
                        }
                        recommendations.push(ExplorationRecommendation {
                            companion: entity,
                            player: companion.followed_player,
                            category: if matches!(action, GuidanceAction::StudyKnowledge(_)) {
                                ExplorationTargetCategory::Knowledge
                            } else {
                                ExplorationTargetCategory::MissionObjective
                            },
                            mission: Some(objective.mission),
                            objective: Some(objective.id),
                            destination,
                            entity: match action {
                                GuidanceAction::Interact(value)
                                | GuidanceAction::DefeatEnemy(Some(value)) => Some(value),
                                _ => None,
                            },
                            reason: ExplorationRecommendationReason::MissionProgress,
                            priority: ExplorationPriority::Critical,
                            fulfilled_requirements: fulfilled,
                            missing_requirements: objective.missing_requirements.clone(),
                            accessible: true,
                            action: ExplorationAction::Guidance(action),
                        });
                    }
                }
            }
        }
        for encounter in self.spatial_encounters() {
            if encounter.state != crate::encounter::SpatialEncounterState::Active
                || !player_can_reach(encounter.position, encounter.activation_radius)
            {
                continue;
            }
            recommendations.push(ExplorationRecommendation {
                companion: entity,
                player: companion.followed_player,
                category: ExplorationTargetCategory::Encounter,
                mission: None,
                objective: None,
                destination: None,
                entity: None,
                reason: ExplorationRecommendationReason::PendingEncounter,
                priority: ExplorationPriority::High,
                fulfilled_requirements: Vec::new(),
                missing_requirements: Vec::new(),
                accessible: true,
                action: ExplorationAction::ResolveEncounter(encounter.id),
            });
        }
        for derelict in self.derelicts() {
            if !matches!(derelict.state, DerelictState::Discovered | DerelictState::Exploring)
                || !ship_operational
                || !player_can_reach(derelict.position, derelict.interaction_range)
            {
                continue;
            }
            recommendations.push(ExplorationRecommendation {
                companion: entity,
                player: companion.followed_player,
                category: ExplorationTargetCategory::Derelict,
                mission: None,
                objective: None,
                destination: None,
                entity: None,
                reason: ExplorationRecommendationReason::DerelictContent,
                priority: ExplorationPriority::Normal,
                fulfilled_requirements: Vec::new(),
                missing_requirements: Vec::new(),
                accessible: true,
                action: ExplorationAction::ExploreDerelict(derelict.id),
            });
        }
        for world in self.functional_worlds() {
            let Some(destination) = self.destination(world.destination) else {
                continue;
            };
            if destination.discovery != crate::navigation::DiscoveryState::Discovered
                || !ship_operational
                || !player_can_reach(destination.position, world.interaction_range)
            {
                continue;
            }
            let available = world.activities.iter().any(|activity| match activity {
                WorldActivity::Trade(market) => market.offers.iter().any(|offer| offer.stock > 0),
                WorldActivity::Resource(site) => site.available > 0,
                WorldActivity::Combat(zone) => !zone.resolved,
                WorldActivity::Story(story) => !story.interacted,
            });
            if !available {
                continue;
            }
            let activity = world
                .activities
                .iter()
                .find(|activity| match activity {
                    WorldActivity::Trade(market) => market.offers.iter().any(|offer| offer.stock > 0),
                    WorldActivity::Resource(site) => site.available > 0,
                    WorldActivity::Combat(zone) => !zone.resolved,
                    WorldActivity::Story(story) => !story.interacted,
                })
                .map(WorldActivity::category);
            let Some(category) = activity else {
                continue;
            };
            recommendations.push(ExplorationRecommendation {
                companion: entity,
                player: companion.followed_player,
                category: ExplorationTargetCategory::WorldActivity,
                mission: None,
                objective: None,
                destination: Some(world.destination),
                entity: None,
                reason: ExplorationRecommendationReason::WorldActivity,
                priority: ExplorationPriority::Low,
                fulfilled_requirements: Vec::new(),
                missing_requirements: Vec::new(),
                accessible: true,
                action: ExplorationAction::PerformWorldActivity {
                    destination: world.destination,
                    category,
                },
            });
        }
        recommendations.sort_by_key(|recommendation| {
            let priority = match recommendation.priority {
                ExplorationPriority::Critical => 0,
                ExplorationPriority::High => 1,
                ExplorationPriority::Normal => 2,
                ExplorationPriority::Low => 3,
            };
            let category = match recommendation.category {
                ExplorationTargetCategory::MissionObjective => 0,
                ExplorationTargetCategory::Knowledge => 1,
                ExplorationTargetCategory::Encounter => 2,
                ExplorationTargetCategory::Derelict => 3,
                ExplorationTargetCategory::WorldActivity => 4,
            };
            (
                priority,
                category,
                recommendation.destination.map_or(0, DestinationId::value),
                recommendation.entity.map_or(0, EntityId::value),
                recommendation.objective.map_or(0, |value| value.0),
            )
        });
        let primary = recommendations.first().cloned();
        let alternatives = recommendations.into_iter().skip(1).take(2).collect();
        Ok(Some(ExplorationAssessment {
            companion: entity,
            player: companion.followed_player,
            primary,
            alternatives,
        }))
    }
    pub fn companion_preparation_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<PreparationAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let exploration = self.companion_exploration_assessment(entity)?;
        let exploration_target = exploration.as_ref().and_then(|value| value.primary.clone());
        let guidance = self
            .guidance_for_player(companion.followed_player)
            .map_err(|_| CompanionError::GuidanceUnavailable)?;
        let guided_action = guidance.current_objective.and_then(|id| {
            guidance
                .objectives
                .iter()
                .find(|objective| objective.id == id)
                .and_then(|objective| objective.recommended_action)
        });
        let ship = self.navigation().ship_id();
        let fuel_available = ship.and_then(|value| self.ship_fuel(value)).map(|value| value.current());
        let current_position = self.navigation().current_position();
        let route_requirement = |destination: DestinationId| {
            let origin = self
                .destinations()
                .find(|value| value.position == current_position)
                .map(|value| value.id)?;
            let route = self.navigation().route(origin, destination)?;
            Some((route.fuel_cost, fuel_available.unwrap_or_default()))
        };
        let mut satisfied = Vec::new();
        let mut missing = Vec::new();
        let mut action = None;
        let mut category = PreparationCategory::Ready;
        let mut reason = PreparationReason::Ready;
        let mut priority = PreparationPriority::Normal;
        let target_destination = exploration_target
            .as_ref()
            .and_then(|target| target.destination)
            .or_else(|| match guided_action {
                Some(GuidanceAction::TravelTo(destination)) => Some(destination),
                _ => None,
            });
        if let Some(destination) = target_destination {
            if let Some((required, available)) = route_requirement(destination) {
                let requirement = PreparationRequirement::Fuel { required, available };
                if available >= required {
                    satisfied.push(requirement);
                } else {
                    missing.push(requirement);
                    category = PreparationCategory::Fuel;
                    reason = PreparationReason::FuelShortage;
                    priority = PreparationPriority::Critical;
                    action = Some(PreparationAction::PlayerRequired);
                }
            }
        }
        if let Some(operational) = self.companion_operational_assessment(entity)? {
            if operational.hull == HullCondition::Critical {
                let configuration = self.ship_repair_configuration();
                let available = self
                    .inventory(companion.followed_player)
                    .map_or(0, |inventory| inventory.quantity(configuration.material));
                let requirement = PreparationRequirement::RepairMaterial {
                    item: configuration.material,
                    required: configuration.material_cost,
                    available,
                };
                if available >= configuration.material_cost {
                    satisfied.push(requirement);
                    if missing.is_empty() {
                        category = PreparationCategory::Repair;
                        action = ship.map(PreparationAction::RepairShip);
                    }
                } else {
                    missing.push(requirement);
                    category = PreparationCategory::Repair;
                    reason = PreparationReason::RepairMaterialShortage;
                    priority = PreparationPriority::Critical;
                    action = Some(PreparationAction::PlayerRequired);
                }
            }
        }
        let tactical = self.companion_tactical_recommendation(entity)?;
        if let Some(TacticalRecommendation {
            action: TacticalAction::DefeatEnemy(enemy),
            ..
        }) = tactical
        {
            let (item, required, available) = self
                .equipped_weapon(companion.followed_player)
                .and_then(|weapon| weapon.ammunition)
                .map(|ammunition| {
                    (
                        Some(ammunition.item),
                        ammunition.amount_per_attack,
                        self.inventory(companion.followed_player)
                            .map_or(0, |inventory| inventory.quantity(ammunition.item)),
                    )
                })
                .unwrap_or((None, 1, 0));
            let requirement = PreparationRequirement::Ammunition {
                item,
                required,
                available,
            };
            if available >= required {
                satisfied.push(requirement);
            } else {
                missing.push(requirement);
                category = PreparationCategory::Ammunition;
                reason = PreparationReason::AmmunitionShortage;
                if priority != PreparationPriority::Critical {
                    priority = PreparationPriority::High;
                    action = Some(PreparationAction::PrepareCombat(enemy));
                }
            }
        }
        if let Some(inventory) = self.inventory(companion.followed_player) {
            for collectible in self.collectibles() {
                if !collectible.active
                    || collectible.owner.is_some_and(|owner| owner != companion.followed_player)
                {
                    continue;
                }
                let requirement = PreparationRequirement::InventoryCapacity {
                    required: collectible.amount,
                    available: inventory.remaining_capacity(),
                };
                if inventory.can_add(collectible.amount) {
                    satisfied.push(requirement);
                } else {
                    missing.push(requirement);
                    category = PreparationCategory::Inventory;
                    reason = PreparationReason::InventoryCapacity;
                    if priority != PreparationPriority::Critical {
                        priority = PreparationPriority::High;
                        action = Some(PreparationAction::FreeInventory);
                    }
                }
            }
        }
        if let Some(target) = exploration_target.as_ref() {
            if target.category == ExplorationTargetCategory::WorldActivity {
                if let Some(destination) = target.destination {
                    if let Some(world) = self.functional_world(destination) {
                        if let Some(WorldActivity::Resource(site)) = world.activities.iter().find(|activity| {
                            matches!(activity, WorldActivity::Resource(site) if site.available > 0)
                        }) {
                            let available_capacity = self
                                .inventory(companion.followed_player)
                                .map_or(0, |inventory| inventory.remaining_capacity());
                            let requirement = PreparationRequirement::ActivityResource {
                                item: site.item,
                                required: site.extraction_amount,
                                available: site.available,
                            };
                            if site.available >= site.extraction_amount {
                                satisfied.push(requirement);
                            } else {
                                missing.push(requirement);
                                category = PreparationCategory::Activity;
                                reason = PreparationReason::ActivityResourceShortage;
                                if priority != PreparationPriority::Critical {
                                    priority = PreparationPriority::High;
                                    action = Some(PreparationAction::PrepareActivity(destination));
                                }
                            }
                            if available_capacity >= site.extraction_amount {
                                satisfied.push(PreparationRequirement::InventoryCapacity {
                                    required: site.extraction_amount,
                                    available: available_capacity,
                                });
                            } else {
                                missing.push(PreparationRequirement::InventoryCapacity {
                                    required: site.extraction_amount,
                                    available: available_capacity,
                                });
                                category = PreparationCategory::Inventory;
                                reason = PreparationReason::InventoryCapacity;
                                if priority != PreparationPriority::Critical {
                                    priority = PreparationPriority::High;
                                    action = Some(PreparationAction::FreeInventory);
                                }
                            }
                        }
                    }
                }
            }
        }
        let prepared = missing.is_empty();
        if prepared {
            category = PreparationCategory::Ready;
            reason = PreparationReason::Ready;
            priority = PreparationPriority::Normal;
            action = None;
        }
        Ok(Some(PreparationAssessment {
            companion: entity,
            player: companion.followed_player,
            exploration: exploration_target,
            category,
            priority,
            reason,
            prepared,
            satisfied_requirements: satisfied,
            missing_requirements: missing,
            action,
        }))
    }
    pub fn companion_coordination_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<CoordinationAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let exploration = self.companion_exploration_assessment(entity)?;
        let preparation = self.companion_preparation_assessment(entity)?;
        let operational = self.companion_operational_assessment(entity)?;
        let tactical = self.companion_tactical_recommendation(entity)?;
        let guidance = self
            .guidance_for_player(companion.followed_player)
            .map_err(|_| CompanionError::GuidanceUnavailable)?;
        let primary_exploration = exploration.as_ref().and_then(|value| value.primary.clone());
        let objective = primary_exploration
            .as_ref()
            .and_then(|value| value.objective)
            .or(guidance.current_objective);
        let mission = primary_exploration.as_ref().and_then(|value| value.mission).or_else(|| {
            objective.and_then(|id| guidance.objectives.iter().find(|value| value.id == id).map(|value| value.mission))
        });
        let mut steps = Vec::new();
        let mut add_step = |category: CoordinationCategory,
                            action: CoordinationAction,
                            objective: Option<crate::types::ObjectiveId>,
                            destination: Option<DestinationId>,
                            entity: Option<EntityId>,
                            status: CoordinationStepStatus,
                            requirements: Vec<PreparationRequirement>,
                            reason: CoordinationReason,
                            priority: CoordinationPriority,
                            description: &'static str| {
            steps.push(CoordinationStep {
                order: 0,
                category,
                action,
                objective,
                destination,
                entity,
                status,
                requirements,
                reason,
                priority,
                description: description.to_string(),
            });
        };
        if let Some(preparation) = preparation.as_ref().filter(|value| !value.prepared) {
            add_step(
                CoordinationCategory::Preparation,
                CoordinationAction::Preparation(preparation.action.unwrap_or(PreparationAction::PlayerRequired)),
                objective,
                primary_exploration.as_ref().and_then(|value| value.destination),
                primary_exploration.as_ref().and_then(|value| value.entity),
                CoordinationStepStatus::Blocked,
                preparation.missing_requirements.clone(),
                CoordinationReason::PreparationRequired,
                match preparation.priority {
                    PreparationPriority::Critical => CoordinationPriority::Critical,
                    PreparationPriority::High => CoordinationPriority::High,
                    PreparationPriority::Normal => CoordinationPriority::Normal,
                },
                "Preparar los recursos necesarios antes de continuar",
            );
        } else if operational.as_ref().is_some_and(|value| value.hull == HullCondition::Destroyed) {
            add_step(
                CoordinationCategory::Preparation,
                CoordinationAction::PlayerRequired,
                objective,
                None,
                operational.as_ref().map(|value| value.ship),
                CoordinationStepStatus::Blocked,
                Vec::new(),
                CoordinationReason::CriticalRisk,
                CoordinationPriority::Critical,
                "La nave no puede continuar en su estado actual",
            );
        } else if let Some(tactical) = tactical.as_ref().filter(|value| value.priority == TacticalPriority::Critical) {
            let (action, category, entity, description) = match tactical.action {
                TacticalAction::DefeatEnemy(enemy) => (
                    CoordinationAction::Tactical(tactical.action),
                    CoordinationCategory::Combat,
                    Some(enemy),
                    "Resolver la amenaza enemiga inmediata",
                ),
                TacticalAction::AddressEncounter(_encounter) => (
                    CoordinationAction::Tactical(tactical.action),
                    CoordinationCategory::Risk,
                    None,
                    "Resolver el encuentro activo antes de continuar",
                ),
                TacticalAction::WithdrawFromHazard(_hazard) => (
                    CoordinationAction::Tactical(tactical.action),
                    CoordinationCategory::Risk,
                    None,
                    "Salir del peligro activo antes de continuar",
                ),
                TacticalAction::PlayerAction(action) => (
                    CoordinationAction::Guidance(action),
                    CoordinationCategory::Risk,
                    None,
                    "Ejecutar la acción requerida por el riesgo actual",
                ),
            };
            add_step(
                category,
                action,
                objective,
                None,
                entity,
                CoordinationStepStatus::Recommended,
                Vec::new(),
                CoordinationReason::CriticalRisk,
                CoordinationPriority::Critical,
                description,
            );
        } else if let Some(target) = primary_exploration.as_ref() {
            match target.action {
                ExplorationAction::Guidance(GuidanceAction::TravelTo(destination)) => {
                    let action = match self.autopilot_state() {
                        crate::navigation::AutopilotState::InTransit { destination: current, .. }
                        | crate::navigation::AutopilotState::Planned { destination: current, .. }
                            if *current == destination => CoordinationAction::MonitorAutopilot(destination),
                        _ => CoordinationAction::StartAutopilot(destination),
                    };
                    add_step(
                        CoordinationCategory::Navigation,
                        action,
                        target.objective,
                        Some(destination),
                        target.entity,
                        CoordinationStepStatus::Recommended,
                        preparation.as_ref().map_or_else(Vec::new, |value| value.missing_requirements.clone()),
                        CoordinationReason::RouteToObjective,
                        CoordinationPriority::Normal,
                        "Continuar la ruta hacia el objetivo activo",
                    );
                }
                ExplorationAction::Guidance(action) => add_step(
                    CoordinationCategory::Mission,
                    CoordinationAction::Guidance(action),
                    target.objective,
                    target.destination,
                    target.entity,
                    CoordinationStepStatus::Recommended,
                    Vec::new(),
                    CoordinationReason::ObjectiveAction,
                    CoordinationPriority::Normal,
                    "Ejecutar la acción del objetivo activo",
                ),
                action => add_step(
                    match target.category {
                        ExplorationTargetCategory::WorldActivity => CoordinationCategory::Activity,
                        ExplorationTargetCategory::Encounter | ExplorationTargetCategory::Derelict => {
                            CoordinationCategory::Exploration
                        }
                        _ => CoordinationCategory::Mission,
                    },
                    CoordinationAction::Exploration(action),
                    target.objective,
                    target.destination,
                    target.entity,
                    CoordinationStepStatus::Recommended,
                    Vec::new(),
                    CoordinationReason::ObjectiveAction,
                    CoordinationPriority::Normal,
                    "Continuar la exploración requerida por el objetivo",
                ),
            }
        } else if let Some(action) = guidance.recommended_action {
            add_step(
                CoordinationCategory::Mission,
                CoordinationAction::Guidance(action),
                objective,
                None,
                None,
                CoordinationStepStatus::Recommended,
                Vec::new(),
                CoordinationReason::ObjectiveAction,
                CoordinationPriority::Normal,
                "Ejecutar la siguiente acción guiada",
            );
        }
        if let Some(assessment) = exploration.as_ref() {
            for target in assessment.alternatives.iter().take(2) {
                steps.push(CoordinationStep {
                    order: 0,
                    category: match target.category {
                        ExplorationTargetCategory::WorldActivity => CoordinationCategory::Activity,
                        ExplorationTargetCategory::Encounter | ExplorationTargetCategory::Derelict => {
                            CoordinationCategory::Exploration
                        }
                        _ => CoordinationCategory::Mission,
                    },
                    action: match target.action {
                        ExplorationAction::Guidance(action) => CoordinationAction::Guidance(action),
                        action => CoordinationAction::Exploration(action),
                    },
                    objective: target.objective,
                    destination: target.destination,
                    entity: target.entity,
                    status: CoordinationStepStatus::Future,
                    requirements: Vec::new(),
                    reason: CoordinationReason::FutureDependency,
                    priority: CoordinationPriority::Low,
                    description: "Paso posterior disponible cuando corresponda".to_string(),
                });
            }
        }
        if let Some(current_mission) = mission {
            for guided in guidance.objectives.iter().filter(|value| {
                value.mission == current_mission
                    && Some(value.id) != objective
                    && matches!(
                        value.status,
                        crate::guidance::GuidanceStatus::Available
                            | crate::guidance::GuidanceStatus::Active
                    )
            }) {
                let Some(action) = guided.recommended_action else {
                    continue;
                };
                let destination = match action {
                    GuidanceAction::TravelTo(value)
                    | GuidanceAction::IdentifySignal(Some(value))
                    | GuidanceAction::DiscoverDestination(value) => Some(value),
                    _ => None,
                };
                steps.push(CoordinationStep {
                    order: 0,
                    category: if destination.is_some() {
                        CoordinationCategory::Navigation
                    } else {
                        CoordinationCategory::Mission
                    },
                    action: CoordinationAction::Guidance(action),
                    objective: Some(guided.id),
                    destination,
                    entity: match action {
                        GuidanceAction::Interact(value)
                        | GuidanceAction::DefeatEnemy(Some(value)) => Some(value),
                        _ => None,
                    },
                    status: CoordinationStepStatus::Future,
                    requirements: Vec::new(),
                    reason: CoordinationReason::FutureDependency,
                    priority: CoordinationPriority::Low,
                    description: "Siguiente objetivo disponible segun la guia actual".to_string(),
                });
            }
        }
        steps.sort_by_key(|step| {
            let priority = match step.priority {
                CoordinationPriority::Critical => 0_u8,
                CoordinationPriority::High => 1,
                CoordinationPriority::Normal => 2,
                CoordinationPriority::Low => 3,
            };
            (
                priority,
                step.objective.map_or(0, |value| value.0),
                step.destination.map_or(0, DestinationId::value),
                step.entity.map_or(0, EntityId::value),
            )
        });
        for (index, step) in steps.iter_mut().enumerate() {
            step.order = (index + 1) as u32;
        }
        let next_step = steps.first().cloned();
        let missing_requirements = preparation
            .as_ref()
            .map_or_else(Vec::new, |value| value.missing_requirements.clone());
        Ok(Some(CoordinationAssessment {
            companion: entity,
            player: companion.followed_player,
            objective,
            mission,
            blocked: next_step.as_ref().is_some_and(|step| step.status == CoordinationStepStatus::Blocked),
            ready: next_step.is_none(),
            next_step,
            later_steps: steps.into_iter().skip(1).collect(),
            missing_requirements,
        }))
    }
    pub fn companion_recovery_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<RecoveryAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let coordination = self.companion_coordination_assessment(entity)?;
        let Some(coordination) = coordination else {
            return Ok(None);
        };
        let preparation = self.companion_preparation_assessment(entity)?;
        let exploration = self.companion_exploration_assessment(entity)?;
        let operational = self.companion_operational_assessment(entity)?;
        let tactical = self.companion_tactical_recommendation(entity)?;
        let attempted = coordination.next_step.clone();
        let mut alternatives = Vec::new();
        let coordination_category = |category| match category {
            CoordinationCategory::Preparation => RecoveryCategory::Preparation,
            CoordinationCategory::Risk => RecoveryCategory::Encounter,
            CoordinationCategory::Navigation => RecoveryCategory::Navigation,
            CoordinationCategory::Exploration => RecoveryCategory::Exploration,
            CoordinationCategory::Combat => RecoveryCategory::Combat,
            CoordinationCategory::Activity => RecoveryCategory::Exploration,
            CoordinationCategory::Mission => RecoveryCategory::Mission,
        };
        for step in &coordination.later_steps {
            alternatives.push(RecoveryAlternative {
                category: coordination_category(step.category),
                action: RecoveryAction::Coordination(step.action),
                priority: step.priority,
                destination: step.destination,
                entity: step.entity,
                viable: step.status != CoordinationStepStatus::Blocked,
                reason: step.description.clone(),
            });
        }
        if let Some(operational) = operational.as_ref() {
            if !matches!(
                operational.navigation.as_ref().map(|value| &value.autopilot),
                Some(crate::navigation::AutopilotState::InTransit { .. })
            ) {
                if let Some(comparison) = operational.route_comparison.as_ref() {
                    for route in comparison.alternatives.iter().filter(|value| value.viable) {
                        let Some(destination) = route.route.last().copied() else {
                            continue;
                        };
                        alternatives.push(RecoveryAlternative {
                            category: RecoveryCategory::Navigation,
                            action: RecoveryAction::Coordination(
                                CoordinationAction::StartAutopilot(destination),
                            ),
                            priority: CoordinationPriority::Normal,
                            destination: Some(destination),
                            entity: None,
                            viable: true,
                            reason: "Ruta alternativa registrada y viable".to_string(),
                        });
                    }
                }
            }
        }
        alternatives.sort_by_key(|alternative| {
            let priority = match alternative.priority {
                CoordinationPriority::Critical => 0_u8,
                CoordinationPriority::High => 1,
                CoordinationPriority::Normal => 2,
                CoordinationPriority::Low => 3,
            };
            (
                priority,
                alternative.destination.map_or(0, DestinationId::value),
                alternative.entity.map_or(0, EntityId::value),
                alternative.category as u8,
            )
        });
        alternatives.dedup();
        let mut blocker = None;
        if let Some(preparation) = preparation.as_ref().filter(|value| !value.prepared) {
            let reason = match preparation.reason {
                PreparationReason::FuelShortage => RecoveryReason::FuelInsufficient,
                PreparationReason::RepairMaterialShortage => RecoveryReason::PreparationRequired,
                PreparationReason::InventoryCapacity => RecoveryReason::InventoryCapacity,
                PreparationReason::AmmunitionShortage => RecoveryReason::PreparationRequired,
                PreparationReason::ActivityResourceShortage => RecoveryReason::PreparationRequired,
                PreparationReason::Ready => RecoveryReason::PreparationRequired,
            };
            blocker = Some(RecoveryBlocker {
                category: RecoveryCategory::Preparation,
                reason,
                action: attempted
                    .as_ref()
                    .map(|step| RecoveryAction::Coordination(step.action)),
                objective: coordination.objective,
                destination: attempted.as_ref().and_then(|step| step.destination),
                entity: attempted.as_ref().and_then(|step| step.entity),
                missing_requirements: preparation.missing_requirements.clone(),
                evidence: "La evaluacion de preparacion reporta requisitos faltantes".to_string(),
            });
        } else if operational
            .as_ref()
            .is_some_and(|value| value.hull == HullCondition::Destroyed)
        {
            blocker = Some(RecoveryBlocker {
                category: RecoveryCategory::Preparation,
                reason: RecoveryReason::ShipDestroyed,
                action: Some(RecoveryAction::PlayerRequired),
                objective: coordination.objective,
                destination: None,
                entity: operational.as_ref().map(|value| value.ship),
                missing_requirements: Vec::new(),
                evidence: "La integridad autoritativa de la nave esta destruida".to_string(),
            });
        } else if let Some(tactical) = tactical.as_ref().filter(|value| {
            value.priority == TacticalPriority::Critical
        }) {
            let (category, reason, entity) = match tactical.action {
                TacticalAction::DefeatEnemy(enemy) => {
                    (RecoveryCategory::Combat, RecoveryReason::ActiveThreat, Some(enemy))
                }
                TacticalAction::AddressEncounter(_) => {
                    (RecoveryCategory::Encounter, RecoveryReason::ActiveEncounter, None)
                }
                TacticalAction::WithdrawFromHazard(_) => {
                    (RecoveryCategory::Navigation, RecoveryReason::ActiveEncounter, None)
                }
                TacticalAction::PlayerAction(_) => {
                    (RecoveryCategory::Mission, RecoveryReason::PlayerDecisionRequired, None)
                }
            };
            blocker = Some(RecoveryBlocker {
                category,
                reason,
                action: Some(RecoveryAction::Coordination(
                    CoordinationAction::Tactical(tactical.action),
                )),
                objective: coordination.objective,
                destination: None,
                entity,
                missing_requirements: Vec::new(),
                evidence: "Existe una amenaza activa con prioridad critica".to_string(),
            });
        } else if let Some(step) = attempted.as_ref().filter(|value| {
            value.status == CoordinationStepStatus::Blocked
        }) {
            blocker = Some(RecoveryBlocker {
                category: coordination_category(step.category),
                reason: RecoveryReason::PlayerDecisionRequired,
                action: Some(RecoveryAction::Coordination(step.action)),
                objective: step.objective,
                destination: step.destination,
                entity: step.entity,
                missing_requirements: step.requirements.clone(),
                evidence: "El siguiente paso existe pero sus requisitos no estan satisfechos".to_string(),
            });
        } else if let Some(target) = exploration.as_ref().and_then(|value| value.primary.as_ref()) {
            if target.missing_requirements.iter().any(|requirement| {
                matches!(requirement, GuidanceRequirement::KnowledgeAtLeast { .. })
            }) {
                blocker = Some(RecoveryBlocker {
                    category: RecoveryCategory::Knowledge,
                    reason: RecoveryReason::KnowledgeMissing,
                    action: Some(RecoveryAction::Coordination(
                        CoordinationAction::Exploration(target.action),
                    )),
                    objective: target.objective,
                    destination: target.destination,
                    entity: target.entity,
                    missing_requirements: Vec::new(),
                    evidence: "La guia exige conocimiento que aun no esta registrado".to_string(),
                });
            }
        }
        let (status, actionable, explanation) = match blocker.as_ref() {
            Some(blocker)
                if alternatives.iter().any(|value| value.viable)
                    || blocker.action.is_some_and(|action| {
                        !matches!(
                            action,
                            RecoveryAction::PlayerRequired
                                | RecoveryAction::Coordination(
                                    CoordinationAction::PlayerRequired
                                )
                                | RecoveryAction::Coordination(
                                    CoordinationAction::Preparation(
                                        PreparationAction::PlayerRequired
                                    )
                                )
                        )
                    }) =>
            (
                RecoveryStatus::Recoverable,
                true,
                "Existe una accion real conocida para recuperar la progresion".to_string(),
            ),
            Some(_) => (
                RecoveryStatus::Blocked,
                false,
                "El bloqueo requiere una decision o intervencion del jugador".to_string(),
            ),
            None if attempted.is_some() => (
                RecoveryStatus::Actionable,
                true,
                "El siguiente paso esta respaldado por el estado actual".to_string(),
            ),
            None => (
                RecoveryStatus::Satisfied,
                false,
                "No existe un objetivo pendiente en la guia actual".to_string(),
            ),
        };
        let missing_requirements = preparation
            .as_ref()
            .map_or_else(Vec::new, |value| value.missing_requirements.clone());
        Ok(Some(RecoveryAssessment {
            companion: entity,
            player: companion.followed_player,
            objective: coordination.objective,
            mission: coordination.mission,
            attempted,
            status,
            blocker,
            alternatives,
            missing_requirements,
            actionable,
            explanation,
        }))
    }
    pub fn companion_recommendation_explanation(
        &self,
        entity: EntityId,
    ) -> Result<Option<RecommendationExplanation>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let coordination = self.companion_coordination_assessment(entity)?;
        let recovery = self.companion_recovery_assessment(entity)?;
        let preparation = self.companion_preparation_assessment(entity)?;
        let operational = self.companion_operational_assessment(entity)?;
        let exploration = self.companion_exploration_assessment(entity)?;
        let Some(coordination) = coordination else {
            return Ok(None);
        };
        let Some(recovery) = recovery else {
            return Ok(None);
        };
        let status = match recovery.status {
            RecoveryStatus::Actionable => ExplanationStatus::Actionable,
            RecoveryStatus::Recoverable => ExplanationStatus::Recoverable,
            RecoveryStatus::Blocked => ExplanationStatus::Blocked,
            RecoveryStatus::Satisfied => ExplanationStatus::Satisfied,
        };
        let reason = match recovery.blocker.as_ref().map(|value| value.reason) {
            Some(RecoveryReason::FuelInsufficient) => ExplanationReason::FuelInsufficient,
            Some(RecoveryReason::ShipDestroyed) => ExplanationReason::ShipDestroyed,
            Some(RecoveryReason::ActiveThreat) => ExplanationReason::ActiveThreat,
            Some(RecoveryReason::KnowledgeMissing) => ExplanationReason::KnowledgeMissing,
            Some(RecoveryReason::PreparationRequired)
            | Some(RecoveryReason::InventoryCapacity)
            | Some(RecoveryReason::ShipNotOperational)
            | Some(RecoveryReason::ActiveEncounter)
            | Some(RecoveryReason::NoKnownRoute)
            | Some(RecoveryReason::PlayerDecisionRequired) => ExplanationReason::PreparationRequired,
            Some(RecoveryReason::ObjectiveSatisfied) => ExplanationReason::ObjectiveCompleted,
            None if status == ExplanationStatus::Satisfied => ExplanationReason::ObjectiveCompleted,
            None => ExplanationReason::ObjectiveAction,
        };
        let action = recovery
            .blocker
            .as_ref()
            .and_then(|value| value.action)
            .or_else(|| {
                coordination
                    .next_step
                    .as_ref()
                    .map(|step| RecoveryAction::Coordination(step.action))
            });
        let mut evidence = Vec::new();
        if let Some(objective) = coordination.objective {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::Objective,
                description: "Existe un objetivo real asociado a la recomendacion".to_string(),
                objective: Some(objective),
                destination: coordination.next_step.as_ref().and_then(|step| step.destination),
                entity: coordination.next_step.as_ref().and_then(|step| step.entity),
                satisfied: status == ExplanationStatus::Satisfied,
            });
        }
        if let Some(preparation) = preparation.as_ref() {
            for requirement in &preparation.satisfied_requirements {
                evidence.push(ExplanationEvidence {
                    kind: if matches!(requirement, PreparationRequirement::Fuel { .. }) {
                        ExplanationEvidenceKind::Fuel
                    } else {
                        ExplanationEvidenceKind::Preparation
                    },
                    description: format!("Requisito satisfecho: {requirement:?}"),
                    objective: coordination.objective,
                    destination: None,
                    entity: None,
                    satisfied: true,
                });
            }
            for requirement in &preparation.missing_requirements {
                evidence.push(ExplanationEvidence {
                    kind: if matches!(requirement, PreparationRequirement::Fuel { .. }) {
                        ExplanationEvidenceKind::Fuel
                    } else {
                        ExplanationEvidenceKind::Preparation
                    },
                    description: format!("Requisito faltante: {requirement:?}"),
                    objective: coordination.objective,
                    destination: None,
                    entity: None,
                    satisfied: false,
                });
            }
        }
        if let Some(target) = exploration.as_ref().and_then(|value| value.primary.as_ref()) {
            if matches!(target.action, ExplorationAction::Guidance(GuidanceAction::TravelTo(_)))
                && target.accessible
            {
                evidence.push(ExplanationEvidence {
                    kind: ExplanationEvidenceKind::Route,
                    description: "Existe una ruta registrada y accesible hacia el objetivo".to_string(),
                    objective: target.objective,
                    destination: target.destination,
                    entity: target.entity,
                    satisfied: true,
                });
            }
        }
        if let Some(operational) = operational.as_ref() {
            if let Some(navigation) = operational.navigation.as_ref() {
                if let Some(destination) = navigation.destination {
                    evidence.push(ExplanationEvidence {
                        kind: ExplanationEvidenceKind::Route,
                        description: if navigation.route.is_empty() {
                            "El destino existe, pero no hay una ruta activa en el autopilot".to_string()
                        } else {
                            "Existe una ruta registrada para el destino recomendado".to_string()
                        },
                        objective: coordination.objective,
                        destination: Some(destination),
                        entity: None,
                        satisfied: !navigation.route.is_empty(),
                    });
                }
                if let (Some(required), Some(available)) =
                    (navigation.fuel_required, navigation.fuel_available)
                {
                    evidence.push(ExplanationEvidence {
                        kind: ExplanationEvidenceKind::Fuel,
                        description: format!("Combustible disponible {available}, requerido {required}"),
                        objective: coordination.objective,
                        destination: navigation.destination,
                        entity: Some(operational.ship),
                        satisfied: available >= required,
                    });
                }
            }
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::Integrity,
                description: format!(
                    "Integridad de nave {}/{}",
                    operational.integrity, operational.maximum_integrity
                ),
                objective: coordination.objective,
                destination: None,
                entity: Some(operational.ship),
                satisfied: operational.hull != HullCondition::Destroyed,
            });
        }
        if let Some(blocker) = recovery.blocker.as_ref() {
            let (kind, description) = match blocker.reason {
                RecoveryReason::ActiveThreat => (
                    ExplanationEvidenceKind::Threat,
                    "Existe un enemigo activo que requiere atencion".to_string(),
                ),
                RecoveryReason::KnowledgeMissing => (
                    ExplanationEvidenceKind::Knowledge,
                    "Falta conocimiento requerido por el objetivo".to_string(),
                ),
                _ => (
                    ExplanationEvidenceKind::StateChange,
                    blocker.evidence.clone(),
                ),
            };
            evidence.push(ExplanationEvidence {
                kind,
                description,
                objective: blocker.objective,
                destination: blocker.destination,
                entity: blocker.entity,
                satisfied: false,
            });
        }
        if let Some(target) = exploration.as_ref().and_then(|value| value.primary.as_ref()) {
            for requirement in &target.missing_requirements {
                if matches!(requirement, GuidanceRequirement::KnowledgeAtLeast { .. }) {
                    evidence.push(ExplanationEvidence {
                        kind: ExplanationEvidenceKind::Knowledge,
                        description: format!("Requisito de conocimiento: {requirement:?}"),
                        objective: target.objective,
                        destination: target.destination,
                        entity: target.entity,
                        satisfied: false,
                    });
                }
            }
        }
        let alternatives = recovery
            .alternatives
            .iter()
            .map(|alternative| ExplainedAlternative {
                alternative: alternative.clone(),
                selected: false,
                explanation: if alternative.viable {
                    format!(
                        "Alternativa viable considerada; conserva prioridad {:?}",
                        alternative.priority
                    )
                } else {
                    "Alternativa descartada porque no es viable".to_string()
                },
            })
            .collect::<Vec<_>>();
        for alternative in &alternatives {
            evidence.push(ExplanationEvidence {
                kind: ExplanationEvidenceKind::Alternative,
                description: alternative.explanation.clone(),
                objective: coordination.objective,
                destination: alternative.alternative.destination,
                entity: alternative.alternative.entity,
                satisfied: alternative.alternative.viable,
            });
        }
        Ok(Some(RecommendationExplanation {
            companion: entity,
            player: companion.followed_player,
            objective: coordination.objective,
            mission: coordination.mission,
            action,
            status,
            reason,
            evidence,
            satisfied_requirements: preparation
                .as_ref()
                .map_or_else(Vec::new, |value| value.satisfied_requirements.clone()),
            missing_requirements: recovery.missing_requirements.clone(),
            blocker: recovery.blocker.clone(),
            alternatives,
            explanation: recovery.explanation,
        }))
    }
    pub fn companion_consistency_assessment(
        &self,
        entity: EntityId,
    ) -> Result<Option<crate::companion::ConsistencyAssessment>, CompanionError> {
        let companion = self
            .companion(entity)
            .ok_or(CompanionError::CompanionNotFound)?;
        if !companion.active {
            return Err(CompanionError::CompanionInactive);
        }
        if companion.behavior == CompanionBehavior::Unavailable {
            return Ok(None);
        }
        let Some(player) = self.player(companion.followed_player) else {
            return Ok(None);
        };
        if !player.is_active || !self.entities.contains(player.entity_id) {
            return Ok(None);
        }
        let coordination = self.companion_coordination_assessment(entity)?;
        let recovery = self.companion_recovery_assessment(entity)?;
        let explanation = self.companion_recommendation_explanation(entity)?;
        let (Some(coordination), Some(recovery), Some(explanation)) =
            (coordination, recovery, explanation)
        else {
            return Ok(None);
        };
        let mut contradictions = Vec::new();
        let mut informational = Vec::new();
        let travel_action = coordination.next_step.as_ref().and_then(|step| {
            matches!(
                step.action,
                CoordinationAction::StartAutopilot(_) | CoordinationAction::MonitorAutopilot(_)
            )
            .then_some(step)
        });
        let fuel_blocked = recovery.blocker.as_ref().is_some_and(|blocker| {
            matches!(blocker.reason, RecoveryReason::FuelInsufficient)
        });
        if fuel_blocked
            && travel_action.is_some_and(|step| step.status == CoordinationStepStatus::Recommended)
        {
            if let Some(step) = travel_action {
                contradictions.push(Contradiction {
                    severity: ContradictionSeverity::Critical,
                    category: ContradictionCategory::PreparationNavigation,
                    reason: ContradictionReason::CriticalPreparationConflict,
                    explanation: "La coordinacion recomienda viajar aunque la preparacion confirma combustible insuficiente".to_string(),
                    evidence: vec![ConsistencyEvidence {
                        category: ContradictionCategory::PreparationNavigation,
                        description: "El bloqueo de combustible y la recomendacion de viaje se refieren al mismo estado".to_string(),
                        objective: step.objective,
                        destination: step.destination,
                        entity: recovery.blocker.as_ref().and_then(|value| value.entity),
                        confirmed: true,
                    }],
                });
            }
        }
        if let Some(blocker) = recovery.blocker.as_ref() {
            let threat_conflict = blocker.reason == RecoveryReason::ActiveThreat
                && !matches!(
                    coordination.next_step.as_ref().map(|step| step.action),
                    Some(CoordinationAction::Tactical(TacticalAction::DefeatEnemy(enemy)))
                        if Some(enemy) == blocker.entity
                );
            if threat_conflict {
                contradictions.push(Contradiction {
                    severity: ContradictionSeverity::Important,
                    category: ContradictionCategory::TacticalCoordination,
                    reason: ContradictionReason::ActiveThreatConflict,
                    explanation: "La recuperacion confirma una amenaza activa, pero la coordinacion no recomienda resolverla".to_string(),
                    evidence: vec![ConsistencyEvidence {
                        category: ContradictionCategory::TacticalCoordination,
                        description: blocker.evidence.clone(),
                        objective: blocker.objective,
                        destination: blocker.destination,
                        entity: blocker.entity,
                        confirmed: true,
                    }],
                });
            }
        }
        if matches!(recovery.status, RecoveryStatus::Blocked | RecoveryStatus::Recoverable)
            && matches!(
                explanation.status,
                ExplanationStatus::Actionable | ExplanationStatus::Satisfied
            )
            && explanation.blocker.is_none()
        {
            contradictions.push(Contradiction {
                severity: ContradictionSeverity::Important,
                category: ContradictionCategory::ExplanationRecovery,
                reason: ContradictionReason::BlockerStateConflict,
                explanation: "La explicacion presenta el estado como resuelto aunque la recuperacion conserva un bloqueo".to_string(),
                evidence: vec![ConsistencyEvidence {
                    category: ContradictionCategory::ExplanationRecovery,
                    description: recovery.explanation.clone(),
                    objective: recovery.objective,
                    destination: recovery.blocker.as_ref().and_then(|value| value.destination),
                    entity: recovery.blocker.as_ref().and_then(|value| value.entity),
                    confirmed: true,
                }],
            });
        }
        if let Some(objective) = coordination.objective {
            let completed_recommendation = self.objective(objective).is_some_and(|value| {
                value.state() == crate::domains::objective::ObjectiveState::Completed
            }) && coordination.next_step.as_ref().is_some_and(|step| {
                step.objective == Some(objective)
                    && step.status != CoordinationStepStatus::Satisfied
            });
            if completed_recommendation {
                contradictions.push(Contradiction {
                    severity: ContradictionSeverity::Important,
                    category: ContradictionCategory::ObjectiveGuidance,
                    reason: ContradictionReason::CompletedObjectiveStillRecommended,
                    explanation: "La guia conserva como pendiente un objetivo que el mundo ya marca completado".to_string(),
                    evidence: vec![ConsistencyEvidence {
                        category: ContradictionCategory::ObjectiveGuidance,
                        description: "El estado del objetivo y el siguiente paso no coinciden".to_string(),
                        objective: Some(objective),
                        destination: coordination.next_step.as_ref().and_then(|step| step.destination),
                        entity: coordination.next_step.as_ref().and_then(|step| step.entity),
                        confirmed: true,
                    }],
                });
            }
        }
        if let Some(operational) = self.companion_operational_assessment(entity)? {
            if let Some(comparison) = operational.route_comparison.as_ref() {
                let route_conflict = !comparison.current.viable
                    && travel_action
                        .is_some_and(|step| step.status == CoordinationStepStatus::Recommended);
                if route_conflict {
                    if let Some(step) = travel_action {
                        contradictions.push(Contradiction {
                            severity: ContradictionSeverity::Important,
                            category: ContradictionCategory::RouteViability,
                            reason: ContradictionReason::InfeasibleRoutePresentedAsViable,
                            explanation: "La evaluacion de ruta marca el trayecto como inviable, pero la coordinacion lo recomienda".to_string(),
                            evidence: vec![ConsistencyEvidence {
                                category: ContradictionCategory::RouteViability,
                                description: "La comparacion de rutas no considera viable el trayecto actual".to_string(),
                                objective: step.objective,
                                destination: step.destination,
                                entity: Some(operational.ship),
                                confirmed: true,
                            }],
                        });
                    }
                }
            }
            if operational.route_comparison.is_none() && travel_action.is_some() {
                informational.push(ConsistencyEvidence {
                    category: ContradictionCategory::InsufficientEvidence,
                    description: "No existe comparacion de rutas suficiente para evaluar alternativas".to_string(),
                    objective: coordination.objective,
                    destination: travel_action.and_then(|step| step.destination),
                    entity: Some(operational.ship),
                    confirmed: false,
                });
            }
        } else {
            informational.push(ConsistencyEvidence {
                category: ContradictionCategory::InsufficientEvidence,
                description: "No existe evaluacion operativa suficiente para contrastar navegacion".to_string(),
                objective: coordination.objective,
                destination: travel_action.and_then(|step| step.destination),
                entity: None,
                confirmed: false,
            });
        }
        let state = if contradictions.iter().any(|value| {
            value.severity != ContradictionSeverity::Informational
        }) {
            ConsistencyAuditState::Contradictory
        } else if !contradictions.is_empty() || !informational.is_empty() {
            ConsistencyAuditState::Informational
        } else {
            ConsistencyAuditState::Consistent
        };
        Ok(Some(crate::companion::ConsistencyAssessment {
            companion: entity,
            player: companion.followed_player,
            state,
            coordination: Some(coordination),
            recovery: Some(recovery),
            explanation: Some(explanation),
            contradictions,
            informational,
        }))
    }
    fn guidance_action_is_available(
        &self,
        player: EntityId,
        action: crate::guidance::GuidanceAction,
    ) -> bool {
        match action {
            crate::guidance::GuidanceAction::Interact(target) => {
                let Some(actor) = self.player(player) else {
                    return false;
                };
                let Some(interactable) = self.interactable(target) else {
                    return false;
                };
                if !interactable.enabled
                    || !finite_vec3(actor.position)
                    || !finite_vec3(interactable.position)
                    || !interactable.interaction_range.is_finite()
                    || interactable.interaction_range <= 0.0
                {
                    return false;
                }
                match interactable.behavior {
                    InteractionBehavior::Door { door } => {
                        if !self.access_requirement_satisfied(door.access_requirement) {
                            return false;
                        }
                    }
                    InteractionBehavior::Station { station }
                        if station.state(interactable.enabled)
                            != crate::station::StationState::Available =>
                    {
                        return false;
                    }
                    _ => {}
                }
                let dx = f64::from(actor.position.x) - f64::from(interactable.position.x);
                let dy = f64::from(actor.position.y) - f64::from(interactable.position.y);
                let dz = f64::from(actor.position.z) - f64::from(interactable.position.z);
                let distance_squared = dx.mul_add(dx, dy.mul_add(dy, dz * dz));
                let range = f64::from(interactable.interaction_range);
                distance_squared.is_finite() && distance_squared <= range * range
            }
            crate::guidance::GuidanceAction::CollectItem { item, amount } => self
                .collectibles
                .values()
                .any(|collectible| {
                    let Some(actor) = self.player(player) else {
                        return false;
                    };
                    if collectible.item != item
                        || !collectible.active
                        || collectible.owner.is_some_and(|owner| owner != player)
                        || collectible.amount < amount
                        || !self.can_player_collect(player, collectible.entity, amount)
                        || !finite_vec3(actor.position)
                        || !finite_vec3(collectible.position)
                        || !collectible.interaction_range.is_finite()
                        || collectible.interaction_range <= 0.0
                    {
                        return false;
                    }
                    let dx = f64::from(actor.position.x) - f64::from(collectible.position.x);
                    let dy = f64::from(actor.position.y) - f64::from(collectible.position.y);
                    let dz = f64::from(actor.position.z) - f64::from(collectible.position.z);
                    let distance_squared = dx.mul_add(dx, dy.mul_add(dy, dz * dz));
                    let range = f64::from(collectible.interaction_range);
                    distance_squared.is_finite() && distance_squared <= range * range
                }),
            crate::guidance::GuidanceAction::StudyKnowledge(target) => {
                self.knowledge(target).can_study() && !self.knowledge(target).is_studied()
            }
            crate::guidance::GuidanceAction::IdentifySignal(Some(destination)) => {
                self.destination(destination).is_some()
                    && self.knowledge(KnowledgeTarget::Destination(destination))
                        == crate::knowledge::KnowledgeState::Detected
            }
            crate::guidance::GuidanceAction::DefeatEnemy(expected) => self.enemies().any(|enemy| {
                enemy.active
                    && !enemy.is_defeated()
                    && expected.is_none_or(|target| target == enemy.entity)
            }),
            _ => false,
        }
    }
    pub(crate) fn update_companions(&mut self, delta: Duration) {
        if delta.0 == 0 {
            return;
        }
        let companions: Vec<_> = self.companions.values().copied().collect();
        let seconds = delta.0 as f64 / 1_000.0;
        let max_step = f64::from(crate::companion::FOLLOW_SPEED) * seconds;
        if !max_step.is_finite() || max_step <= 0.0 {
            return;
        }
        for companion in companions {
            if !companion.active {
                continue;
            }
            let Some(player) = self.players.get(&companion.followed_player).copied() else {
                self.transition_companion(companion.entity, CompanionBehavior::Unavailable);
                continue;
            };
            if !player.is_active
                || !self.entities.contains(player.entity_id)
                || !finite_vec3(player.position)
                || !finite_vec3(companion.position)
            {
                self.transition_companion(companion.entity, CompanionBehavior::Unavailable);
                continue;
            }
            match companion.behavior {
                CompanionBehavior::Accompany => {
                    self.move_companion_towards_player(companion.entity, player.position, max_step);
                }
                CompanionBehavior::Investigate(target) => {
                    if !self.knowledge_target_exists(target) || !self.knowledge(target).is_detected() {
                        self.transition_companion(companion.entity, CompanionBehavior::Accompany);
                    }
                }
                CompanionBehavior::CombatSupport => {
                    let combat_exists = self.enemies().any(|enemy| enemy.active && !enemy.is_defeated());
                    if !combat_exists {
                        self.transition_companion(companion.entity, CompanionBehavior::Accompany);
                    }
                }
                CompanionBehavior::Waiting | CompanionBehavior::Unavailable => {}
            }
        }
    }
    fn move_companion_towards_player(
        &mut self,
        entity: EntityId,
        player_position: engine::Vec3,
        max_step: f64,
    ) {
        let Some(companion) = self.companions.get(&entity).copied() else {
            return;
        };
        let dx = f64::from(companion.position.x) - f64::from(player_position.x);
        let dy = f64::from(companion.position.y) - f64::from(player_position.y);
        let dz = f64::from(companion.position.z) - f64::from(player_position.z);
        let distance = dx.mul_add(dx, dy.mul_add(dy, dz * dz)).sqrt();
        if !distance.is_finite() {
            self.transition_companion(entity, CompanionBehavior::Unavailable);
            return;
        }
        let desired = f64::from(crate::companion::FOLLOW_DISTANCE);
        if distance > 0.0 && distance <= desired {
            return;
        }
        let (direction_x, direction_y, direction_z, remaining) = if distance == 0.0 {
            (1.0, 0.0, 0.0, desired)
        } else {
            (-dx / distance, -dy / distance, -dz / distance, distance - desired)
        };
        let step = remaining.min(max_step);
        let position = engine::Vec3 {
            x: (f64::from(companion.position.x) + direction_x * step) as f32,
            y: (f64::from(companion.position.y) + direction_y * step) as f32,
            z: (f64::from(companion.position.z) + direction_z * step) as f32,
        };
        if !finite_vec3(position) {
            self.transition_companion(entity, CompanionBehavior::Unavailable);
            return;
        }
        if let Some(updated) = self.companions.get_mut(&entity) {
            updated.position = position;
        }
        self.publish(crate::events::GameplayEvent::CompanionMoved { companion: entity, position });
    }
    fn transition_companion(&mut self, entity: EntityId, behavior: CompanionBehavior) {
        if let Some(companion) = self.companions.get_mut(&entity) {
            if companion.behavior == behavior {
                return;
            }
            companion.behavior = behavior;
            self.publish(crate::events::GameplayEvent::CompanionBehaviorChanged { companion: entity, behavior });
        }
    }
    pub fn guidance_for_player(
        &self,
        player: EntityId,
    ) -> Result<crate::guidance::GuidanceState, crate::guidance::GuidanceError> {
        crate::guidance::GuidanceSystem::query(self, player)
    }
    pub fn players(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
    }
    pub fn group_size(&self) -> GroupSize {
        let active_players = self
            .players
            .values()
            .filter(|player| {
                player.is_active
                    && self.entities.contains(player.entity_id)
                    && self
                        .combatant(player.entity_id)
                        .is_some_and(|combatant| combatant.active && !combatant.is_defeated())
            })
            .count();
        GroupSize::from_active_players(active_players)
    }
    pub fn current_group_scaling(&self) -> GameplayScaling {
        GameplayScaling::for_group_size(self.group_size())
    }
    pub fn scaling_for_player(
        &self,
        player: EntityId,
    ) -> Result<GameplayScaling, ScalingError> {
        let Some(player_state) = self.player(player) else {
            return Err(ScalingError::PlayerNotFound);
        };
        if !player_state.is_active
            || !self.entities.contains(player)
            || !self
                .combatant(player)
                .is_some_and(|combatant| combatant.active && !combatant.is_defeated())
        {
            return Err(ScalingError::PlayerInactive);
        }
        Ok(self.current_group_scaling())
    }
    pub(crate) fn scaled_enemy_configuration(
        &self,
        configuration: EnemyConfiguration,
    ) -> Result<EnemyConfiguration, ScalingError> {
        self.current_group_scaling()
            .scale_enemy_configuration(configuration)
    }
    pub fn combatant(&self, entity: EntityId) -> Option<&Combatant> {
        self.combatants.get(&entity)
    }
    pub fn combatants(&self) -> impl Iterator<Item = &Combatant> {
        self.combatants.values()
    }
    pub fn equipped_weapon(&self, player: EntityId) -> Option<crate::combat::WeaponConfiguration> {
        self.combatant(player)?.equipped_weapon
    }
    pub fn player_weapon_ammunition(
        &self,
        player: EntityId,
    ) -> Option<(crate::types::ItemId, u32)> {
        let weapon = self.equipped_weapon(player)?;
        let ammunition = weapon.ammunition?;
        Some((
            ammunition.item,
            self.inventory(player)
                .map_or(0, |inventory| inventory.quantity(ammunition.item)),
        ))
    }
    pub fn player_weapon_ammunition_cost(&self, player: EntityId) -> Option<u32> {
        self.equipped_weapon(player)?
            .ammunition
            .map(|ammunition| ammunition.amount_per_attack)
    }
    pub fn player_weapon_requires_ammunition(&self, player: EntityId) -> Option<bool> {
        Some(self.equipped_weapon(player)?.ammunition.is_some())
    }
    pub fn player_weapon_ready(&self, player: EntityId) -> bool {
        let Some(player_state) = self.player(player) else {
            return false;
        };
        if !self.entities.contains(player) || !player_state.is_active {
            return false;
        }
        let Some(weapon) = self.equipped_weapon(player) else {
            return false;
        };
        if weapon.validate().is_err() {
            return false;
        }
        let Some(ammunition) = weapon.ammunition else {
            return true;
        };
        self.item_definition(ammunition.item)
            .is_some_and(|definition| {
                definition.category == crate::collectible::ItemCategory::CombatAmmunition
                    && self.inventory(player).is_some_and(|inventory| {
                        inventory.quantity(ammunition.item) >= ammunition.amount_per_attack
                    })
            })
    }
    pub fn enemies(&self) -> impl Iterator<Item = &Combatant> {
        self.combatants
            .values()
            .filter(|combatant| !combatant.is_player)
    }
    pub fn player_health(&self, entity: EntityId) -> Option<(u32, u32, bool)> {
        let combatant = self.combatant(entity)?;
        combatant.is_player.then_some((
            combatant.current_health,
            combatant.maximum_health,
            !combatant.is_defeated(),
        ))
    }
    pub fn inventory(&self, player: EntityId) -> Option<&Inventory> {
        self.inventories.get(&player)
    }
    pub fn item_definition(&self, item: crate::types::ItemId) -> Option<&ItemDefinition> {
        self.item_definitions.get(&item)
    }
    pub fn healing_effect(
        &self,
        item: crate::types::ItemId,
    ) -> Option<crate::healing::HealingEffect> {
        self.item_definition(item)?.healing
    }
    pub fn healing_amount(
        &self,
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> Option<u32> {
        let effect = self.healing_effect(item)?;
        let combatant = self.combatant(player)?;
        if !combatant.is_player || combatant.current_health >= combatant.maximum_health {
            return None;
        }
        let requested = effect.health_per_unit.checked_mul(amount)?;
        Some(requested.min(combatant.maximum_health - combatant.current_health))
    }
    pub fn revival_effect(
        &self,
        item: crate::types::ItemId,
    ) -> Option<crate::revival::ReviveEffect> {
        self.item_definition(item)?.revival
    }
    pub fn can_player_revive(
        &self,
        helper: EntityId,
        target: EntityId,
        item: crate::types::ItemId,
    ) -> bool {
        self.revival_preview(helper, target, item).is_ok()
    }
    pub fn can_player_use_healing(
        &self,
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> bool {
        self.phase() == SimulationPhase::Running
            && self
                .player(player)
                .is_some_and(|value| self.entities.contains(player) && value.is_active)
            && self.combatant(player).is_some_and(|combatant| {
                combatant.is_player
                    && !combatant.is_defeated()
                    && combatant.current_health <= combatant.maximum_health
            })
            && amount > 0
            && self
                .inventory(player)
                .is_some_and(|inventory| inventory.quantity(item) >= amount)
            && self
                .healing_amount(player, item, amount)
                .is_some_and(|value| value > 0)
    }
    pub fn knowledge(&self, target: KnowledgeTarget) -> KnowledgeState {
        self.knowledge
            .get(&target)
            .copied()
            .unwrap_or(KnowledgeState::Unknown)
    }
    pub fn knowledge_entries(
        &self,
    ) -> impl Iterator<Item = (KnowledgeTarget, KnowledgeState)> + '_ {
        self.knowledge
            .iter()
            .map(|(target, state)| (*target, *state))
    }
    pub fn known_targets(&self) -> impl Iterator<Item = KnowledgeTarget> + '_ {
        self.knowledge
            .iter()
            .filter(|(_, state)| state.is_detected())
            .map(|(target, _)| *target)
    }
    pub fn identified_targets(&self) -> impl Iterator<Item = KnowledgeTarget> + '_ {
        self.knowledge
            .iter()
            .filter(|(_, state)| state.is_identified())
            .map(|(target, _)| *target)
    }
    pub fn studied_targets(&self) -> impl Iterator<Item = KnowledgeTarget> + '_ {
        self.knowledge
            .iter()
            .filter(|(_, state)| state.is_studied())
            .map(|(target, _)| *target)
    }
    pub fn knowledge_count(&self, state: KnowledgeState) -> u32 {
        self.knowledge
            .values()
            .filter(|current| **current == state)
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }
    pub fn knowledge_count_by_category(
        &self,
        category: crate::knowledge::KnowledgeCategory,
        state: KnowledgeState,
    ) -> u32 {
        self.knowledge
            .iter()
            .filter(|(target, current)| **current == state && category.matches(target))
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }
    pub fn can_player_study_knowledge(&self, player: EntityId, target: KnowledgeTarget) -> bool {
        self.player(player)
            .is_some_and(|value| self.entities.contains(player) && value.is_active)
            && self.knowledge(target).can_study()
    }
    pub fn collectible(&self, entity: EntityId) -> Option<&Collectible> {
        self.collectibles.get(&entity)
    }
    pub fn collectibles(&self) -> impl Iterator<Item = &Collectible> {
        self.collectibles.values()
    }
    pub fn enemy_reward(&self, archetype: crate::combat::EnemyArchetype) -> Option<&EnemyReward> {
        self.enemy_rewards.get(&archetype)
    }
    pub fn enemy_rewards(&self) -> impl Iterator<Item = &EnemyReward> {
        self.enemy_rewards.values()
    }
    pub fn can_player_collect(&self, player: EntityId, collectible: EntityId, amount: u32) -> bool {
        let Some(player_state) = self.players.get(&player) else {
            return false;
        };
        let Some(item) = self.collectibles.get(&collectible) else {
            return false;
        };
        player_state.is_active
            && item.active
            && item.owner.is_none_or(|owner| owner == player)
            && amount > 0
            && amount <= item.amount
            && self
                .inventories
                .get(&player)
                .is_some_and(|inventory| inventory.can_add(amount))
    }
    pub fn interactable(&self, entity: EntityId) -> Option<&Interactable> {
        self.interactables.get(&entity)
    }
    pub fn interactables(&self) -> impl Iterator<Item = &Interactable> {
        self.interactables.values()
    }
    pub fn door(&self, entity: EntityId) -> Option<Door> {
        match self.interactables.get(&entity)?.behavior {
            InteractionBehavior::Door { door } => Some(door),
            _ => None,
        }
    }
    pub fn station(&self, entity: EntityId) -> Option<Station> {
        match self.interactables.get(&entity)?.behavior {
            InteractionBehavior::Station { station } => Some(station),
            _ => None,
        }
    }
    pub fn station_state(&self, entity: EntityId) -> Option<crate::station::StationState> {
        let interactable = self.interactables.get(&entity)?;
        match interactable.behavior {
            InteractionBehavior::Station { station } => Some(station.state(interactable.enabled)),
            _ => None,
        }
    }
    pub fn electrical_panel(&self, entity: EntityId) -> Option<&ElectricalPanel> {
        self.electrical_panels.get(&entity)
    }
    pub fn electrical_panels(&self) -> impl Iterator<Item = &ElectricalPanel> {
        self.electrical_panels.values()
    }
    pub fn life_support_panel(&self, entity: EntityId) -> Option<&LifeSupportPanel> {
        self.life_support_panels.get(&entity)
    }
    pub fn life_support_panels(&self) -> impl Iterator<Item = &LifeSupportPanel> {
        self.life_support_panels.values()
    }
    pub fn navigation(&self) -> &NavigationState {
        &self.navigation
    }
    pub fn destination(&self, destination: DestinationId) -> Option<&Destination> {
        self.navigation.destination(destination)
    }
    pub fn destinations(&self) -> impl Iterator<Item = &Destination> {
        self.navigation.destinations()
    }
    pub fn navigation_routes(&self) -> impl Iterator<Item = &crate::navigation::NavigationRoute> {
        self.navigation.routes()
    }
    pub fn navigation_route(
        &self,
        from: DestinationId,
        to: DestinationId,
    ) -> Option<&crate::navigation::NavigationRoute> {
        self.navigation.route(from, to)
    }
    pub fn autopilot_state(&self) -> &crate::navigation::AutopilotState {
        self.navigation.autopilot_state()
    }
    pub fn travel_progress(&self) -> Option<TravelProgress> {
        self.navigation.travel_progress(self.time().elapsed)
    }
    pub fn travel_state(&self) -> TravelState {
        self.navigation.travel_state()
    }
    pub fn ship_integrity(&self, ship: EntityId) -> Option<ShipIntegrity> {
        self.ship_integrity.get(&ship).copied()
    }
    pub fn ship_structural_state(&self, ship: EntityId) -> Option<StructuralState> {
        self.ship_integrity(ship).map(ShipIntegrity::state)
    }
    pub fn ship_is_operational(&self, ship: EntityId) -> Option<bool> {
        self.ship_integrity(ship).map(ShipIntegrity::is_operational)
    }
    pub fn ship_is_critical(&self, ship: EntityId) -> Option<bool> {
        self.ship_integrity(ship).map(ShipIntegrity::is_critical)
    }
    pub fn ship_is_destroyed(&self, ship: EntityId) -> Option<bool> {
        self.ship_integrity(ship).map(ShipIntegrity::is_destroyed)
    }
    pub fn ship_position(&self) -> Option<engine::Vec3> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.current_position())
    }
    pub fn ship_orientation(&self) -> Option<crate::navigation::ShipOrientation> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.orientation())
    }
    pub fn ship_velocity(&self) -> Option<engine::Vec3> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.velocity())
    }
    pub fn ship_movement_parameters(&self) -> Option<crate::navigation::ShipMovementParameters> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.movement_parameters())
    }
    pub fn ship_control_active(&self) -> Option<bool> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.control_active())
    }
    pub fn ship_at_rest(&self) -> Option<bool> {
        self.navigation
            .ship_id()
            .map(|_| self.navigation.is_at_rest())
    }
    pub fn can_player_control_ship(&self, player: EntityId, station: EntityId) -> bool {
        let Some(player_state) = self.player(player) else {
            return false;
        };
        if !self.entities.contains(player) || !player_state.is_active {
            return false;
        }
        let Some(ship) = self.navigation.ship_id() else {
            return false;
        };
        if !self.entities.contains(ship) {
            return false;
        }
        let Some(interactable) = self.interactable(station) else {
            return false;
        };
        if !interactable.enabled {
            return false;
        }
        matches!(interactable.behavior, InteractionBehavior::Station { station: value }
            if value.station_type == StationType::Piloting && value.occupant == Some(player))
    }
    pub fn ship_repair_configuration(&self) -> ShipRepairConfiguration {
        ShipRepairConfiguration::default()
    }
    pub fn ship_repair_amount(
        &self,
        player: EntityId,
        ship: EntityId,
        station: EntityId,
    ) -> Option<u32> {
        let configuration = self.ship_repair_configuration();
        if !configuration.validate() {
            return None;
        }
        let player_state = self.player(player)?;
        if !self.entities.contains(player) || !player_state.is_active {
            return None;
        }
        if self.navigation.ship_id() != Some(ship) || !self.entities.contains(ship) {
            return None;
        }
        let integrity = self.ship_integrity(ship)?;
        if integrity.is_destroyed() {
            return None;
        }
        let station_state = self.interactables.get(&station)?;
        if !station_state.enabled {
            return None;
        }
        let InteractionBehavior::Station {
            station: station_data,
        } = station_state.behavior
        else {
            return None;
        };
        if station_data.station_type != StationType::Engineering
            || station_data.occupant != Some(player)
        {
            return None;
        }
        let Some(definition) = self.item_definitions.get(&configuration.material) else {
            return None;
        };
        if definition.category != crate::collectible::ItemCategory::RepairMaterial {
            return None;
        }
        if self.inventory(player)?.quantity(configuration.material) < configuration.material_cost {
            return None;
        }
        let missing = integrity.maximum().saturating_sub(integrity.current());
        let amount = configuration.integrity_restored.min(missing);
        (amount > 0).then_some(amount)
    }
    pub fn can_player_repair_ship(
        &self,
        player: EntityId,
        ship: EntityId,
        station: EntityId,
    ) -> bool {
        self.ship_repair_amount(player, ship, station).is_some()
    }
    pub fn environmental_hazard(&self, hazard: HazardId) -> Option<EnvironmentalHazard> {
        self.environmental_hazards.get(&hazard).copied()
    }
    pub fn environmental_hazards(&self) -> impl Iterator<Item = &EnvironmentalHazard> {
        self.environmental_hazards.values()
    }
    pub fn spatial_encounter(&self, encounter: EncounterId) -> Option<&SpatialEncounter> {
        self.spatial_encounters.get(&encounter)
    }
    pub fn spatial_encounters(&self) -> impl Iterator<Item = &SpatialEncounter> {
        self.spatial_encounters.values()
    }
    pub fn derelict(&self, id: DerelictId) -> Option<&Derelict> {
        self.derelicts.get(&id)
    }
    pub fn derelicts(&self) -> impl Iterator<Item = &Derelict> {
        self.derelicts.values()
    }
    pub fn functional_world(&self, destination: DestinationId) -> Option<&FunctionalWorld> {
        self.functional_worlds.get(&destination)
    }
    pub fn functional_worlds(&self) -> impl Iterator<Item = &FunctionalWorld> {
        self.functional_worlds.values()
    }
    pub fn player_currency(&self, player: EntityId) -> Option<u64> {
        self.currencies.get(&player).copied()
    }
    pub fn world_trade_offer(
        &self,
        destination: DestinationId,
        item: crate::types::ItemId,
    ) -> Option<crate::world_activity::TradeOffer> {
        self.functional_world(destination)?.activities.iter().find_map(|activity| {
            if let WorldActivity::Trade(market) = activity {
                market.offers.iter().find(|offer| offer.item == item).cloned()
            } else {
                None
            }
        })
    }
    pub fn register_functional_world(
        &mut self,
        functional_world: FunctionalWorld,
    ) -> Result<(), FunctionalWorldError> {
        functional_world.validate()?;
        if self.functional_worlds.contains_key(&functional_world.destination) {
            return Err(FunctionalWorldError::DuplicateDestination);
        }
        let Some(destination) = self.destination(functional_world.destination) else {
            return Err(FunctionalWorldError::DestinationNotFound);
        };
        if destination.discovery != crate::navigation::DiscoveryState::Discovered {
            return Err(FunctionalWorldError::DestinationNotDiscovered);
        }
        for activity in &functional_world.activities {
            match activity {
                WorldActivity::Trade(market) => {
                    for offer in &market.offers {
                        let Some(definition) = self.item_definition(offer.item) else {
                            return Err(FunctionalWorldError::ItemDefinitionNotFound);
                        };
                        if offer.item.0 == 0 || definition.max_stack < 1 {
                            return Err(FunctionalWorldError::ItemDefinitionNotFound);
                        }
                    }
                }
                WorldActivity::Resource(site) => {
                    if self.item_definition(site.item).is_none() {
                        return Err(FunctionalWorldError::ItemDefinitionNotFound);
                    }
                }
                WorldActivity::Combat(zone) => {
                    if zone.configuration.validate().is_err() {
                        return Err(FunctionalWorldError::InvalidCombat);
                    }
                }
                WorldActivity::Story(story) => {
                    if !self.knowledge_target_exists(story.target) {
                        return Err(FunctionalWorldError::KnowledgeTargetNotFound);
                    }
                }
            }
        }
        let destination_id = functional_world.destination;
        self.functional_worlds
            .insert(destination_id, functional_world);
        self.publish(crate::events::GameplayEvent::FunctionalWorldRegistered {
            destination: destination_id,
        });
        Ok(())
    }
    pub fn register_derelict(&mut self, derelict: Derelict) -> Result<(), DerelictError> {
        derelict.validate()?;
        if self.derelicts.contains_key(&derelict.id) {
            return Err(DerelictError::DuplicateId);
        }
        if let Some(encounter) = derelict.associated_encounter {
            if self.spatial_encounter(encounter).is_none() {
                return Err(DerelictError::EncounterNotFound);
            }
        }
        for reward in &derelict.rewards {
            let Some(definition) = self.item_definition(reward.item) else {
                return Err(DerelictError::ItemDefinitionNotFound);
            };
            if reward.amount > definition.max_stack {
                return Err(DerelictError::InvalidReward);
            }
        }
        let id = derelict.id;
        let position = derelict.position;
        self.derelicts.insert(id, derelict);
        self.publish(crate::events::GameplayEvent::DerelictRegistered {
            derelict: id,
            position,
        });
        Ok(())
    }
    pub fn register_spatial_encounter(
        &mut self,
        encounter: SpatialEncounter,
    ) -> Result<(), SpatialEncounterError> {
        encounter.validate()?;
        if self.spatial_encounters.contains_key(&encounter.id) {
            return Err(SpatialEncounterError::DuplicateId);
        }
        match encounter.kind {
            SpatialEncounterKind::Signal { destination }
                if self.destination(destination).is_none() =>
            {
                return Err(SpatialEncounterError::DestinationNotFound)
            }
            SpatialEncounterKind::Hazard { hazard } if self.environmental_hazard(hazard).is_none() => {
                return Err(SpatialEncounterError::HazardNotFound)
            }
            SpatialEncounterKind::Resource { item, .. }
                if self.item_definition(item).is_none() =>
            {
                return Err(SpatialEncounterError::ItemDefinitionNotFound)
            }
            _ => {}
        }
        self.spatial_encounters.insert(encounter.id, encounter);
        self.publish(crate::events::GameplayEvent::SpatialEncounterRegistered {
            encounter: encounter.id,
            kind: encounter.kind,
        });
        Ok(())
    }
    pub fn active_environmental_hazards(&self) -> impl Iterator<Item = &EnvironmentalHazard> {
        self.environmental_hazards
            .values()
            .filter(|hazard| hazard.active)
    }
    pub fn environmental_hazards_near(&self, position: engine::Vec3) -> Vec<EnvironmentalHazard> {
        self.environmental_hazards
            .values()
            .filter(|hazard| {
                hazard.active
                    && crate::hazard::distance_squared(position, hazard.position).is_some_and(
                        |distance| distance <= hazard.radius as f64 * hazard.radius as f64,
                    )
            })
            .copied()
            .collect()
    }
    pub fn ship_environmental_hazards(&self) -> Vec<EnvironmentalHazard> {
        self.ship_position().map_or_else(Vec::new, |position| {
            self.environmental_hazards_near(position)
        })
    }
    pub fn register_environmental_hazard(
        &mut self,
        hazard: EnvironmentalHazard,
    ) -> Result<(), EnvironmentalHazardError> {
        hazard.validate()?;
        if self.environmental_hazards.contains_key(&hazard.id) {
            return Err(EnvironmentalHazardError::DuplicateId);
        }
        self.environmental_hazards.insert(hazard.id, hazard);
        self.publish(
            crate::events::GameplayEvent::EnvironmentalHazardRegistered {
                hazard: hazard.id,
                hazard_type: hazard.hazard_type,
                position: hazard.position,
            },
        );
        Ok(())
    }
    pub fn initialize_navigation_ship(
        &mut self,
        ship: EntityId,
    ) -> Result<(), NavigationInitializationError> {
        if !self.entities.contains(ship) {
            return Err(NavigationInitializationError::ShipNotFound);
        }
        self.navigation.initialize_ship(ship)
    }
    pub fn initialize_ship_integrity(
        &mut self,
        ship: EntityId,
        maximum: u32,
        initial: u32,
        critical_threshold: u32,
    ) -> Result<(), IntegrityError> {
        if self.navigation.ship_id() != Some(ship) || !self.entities.contains(ship) {
            return Err(IntegrityError::ShipNotFound);
        }
        if self.ship_integrity.contains_key(&ship) {
            return Err(IntegrityError::AlreadyInitialized);
        }
        let integrity = ShipIntegrity::new(ship, maximum, initial, critical_threshold)?;
        self.ship_integrity.insert(ship, integrity);
        self.events
            .push(crate::events::GameplayEvent::ShipIntegrityInitialized {
                ship,
                current: initial,
                maximum,
            });
        Ok(())
    }
    pub fn register_destination(
        &mut self,
        destination: Destination,
    ) -> Result<(), NavigationRegistrationError> {
        self.navigation.register_destination(destination)
    }
    pub fn register_navigation_route(
        &mut self,
        route: crate::navigation::NavigationRoute,
    ) -> Result<(), crate::navigation::NavigationRouteError> {
        self.navigation.register_route(route)
    }
    pub fn ship_resources(&self, ship: EntityId) -> Option<ShipResources> {
        self.ship_resources.get(&ship).copied()
    }
    pub fn ship_fuel(&self, ship: EntityId) -> Option<FuelTank> {
        self.ship_resources
            .get(&ship)
            .map(|resources| resources.fuel())
    }
    pub fn destination_fuel_cost(&self, destination: DestinationId) -> Option<u32> {
        self.navigation
            .destination(destination)
            .map(|destination| destination.fuel_cost)
    }
    pub fn can_afford_destination(&self, destination: DestinationId) -> Option<bool> {
        let ship = self.navigation.ship_id()?;
        let fuel = self.ship_fuel(ship)?;
        Some(
            self.destination_fuel_cost(destination)
                .is_some_and(|cost| fuel.can_consume(cost)),
        )
    }
    pub fn initialize_ship_resources(
        &mut self,
        ship: EntityId,
        capacity: u32,
        initial_fuel: u32,
    ) -> Result<(), FuelError> {
        if self.ship_resources.contains_key(&ship) {
            return Err(FuelError::AlreadyInitialized);
        }
        if self.navigation.ship_id() != Some(ship) || !self.entities.contains(ship) {
            return Err(FuelError::ShipNotFound);
        }
        let resources = ShipResources::new(ship, capacity, initial_fuel)?;
        self.ship_resources.insert(ship, resources);
        self.events
            .push(crate::events::GameplayEvent::ShipResourcesInitialized {
                ship,
                fuel: initial_fuel,
                capacity,
            });
        Ok(())
    }
    pub fn mission(&self, mission: crate::types::MissionId) -> Option<&Mission> {
        self.missions.get(&mission)
    }
    pub fn access_requirement_satisfied(&self, requirement: DoorAccessRequirement) -> bool {
        match requirement {
            DoorAccessRequirement::None => true,
            DoorAccessRequirement::MissionObjectives { mission, required } => self
                .mission(mission)
                .map(|mission| {
                    required.iter().all(|objective_id| {
                        mission
                            .objectives()
                            .iter()
                            .find(|objective| objective.id == *objective_id)
                            .is_some_and(|objective| {
                                objective.state()
                                    == crate::domains::objective::ObjectiveState::Completed
                            })
                    })
                })
                .unwrap_or(false),
        }
    }
    pub fn missions(&self) -> impl Iterator<Item = &Mission> {
        self.missions.values()
    }
    pub fn objective(
        &self,
        objective: crate::types::ObjectiveId,
    ) -> Option<&crate::objective::Objective> {
        self.missions.values().find_map(|mission| {
            mission
                .objectives()
                .iter()
                .find(|value| value.id == objective)
        })
    }
    pub fn objectives_for_mission(
        &self,
        mission: crate::types::MissionId,
    ) -> Option<&[crate::objective::Objective]> {
        self.mission(mission).map(Mission::objectives)
    }
    pub fn events(&self) -> &EventLog {
        &self.events
    }
    pub fn create_entity(&mut self) -> EntityId {
        self.entities.create()
    }
    pub fn register_mission(&mut self, mission: Mission) -> Result<(), MissionRegistrationError> {
        if self.missions.contains_key(&mission.id) {
            return Err(MissionRegistrationError::DuplicateMission);
        }
        mission
            .validate_for_registration()
            .map_err(MissionRegistrationError::Definition)?;
        if mission
            .objectives()
            .iter()
            .any(|objective| self.objective(objective.id).is_some())
        {
            return Err(MissionRegistrationError::DuplicateObjective);
        }
        self.missions.insert(mission.id, mission);
        Ok(())
    }
    pub fn register_interactable(
        &mut self,
        interactable: Interactable,
    ) -> Result<(), InteractionRegistrationError> {
        if !interactable.position.x.is_finite()
            || !interactable.position.y.is_finite()
            || !interactable.position.z.is_finite()
        {
            return Err(InteractionRegistrationError::InvalidPosition);
        }
        if !interactable.interaction_range.is_finite() || interactable.interaction_range <= 0.0 {
            return Err(InteractionRegistrationError::InvalidRange);
        }
        if !self.entities.contains(interactable.entity_id) {
            return Err(InteractionRegistrationError::EntityNotFound);
        }
        if self.interactables.contains_key(&interactable.entity_id) {
            return Err(InteractionRegistrationError::DuplicateEntity);
        }
        self.interactables
            .insert(interactable.entity_id, interactable);
        Ok(())
    }
    pub fn create_console(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<EntityId, InteractionRegistrationError> {
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(InteractionRegistrationError::InvalidPosition);
        }
        if !interaction_range.is_finite() || interaction_range <= 0.0 {
            return Err(InteractionRegistrationError::InvalidRange);
        }
        let entity = self.entities.create();
        let interactable = Interactable::console(entity, position, interaction_range);
        self.interactables.insert(entity, interactable);
        Ok(entity)
    }
    pub fn create_ship_status_console(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<EntityId, InteractionRegistrationError> {
        validate_interactable_geometry(position, interaction_range)?;
        let entity = self.entities.create();
        self.interactables.insert(
            entity,
            Interactable::ship_status_console(entity, position, interaction_range),
        );
        Ok(entity)
    }
    pub fn create_door(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<EntityId, InteractionRegistrationError> {
        validate_interactable_geometry(position, interaction_range)?;
        let entity = self.entities.create();
        self.interactables.insert(
            entity,
            Interactable::door(entity, position, interaction_range),
        );
        Ok(entity)
    }
    pub fn create_bridge_door(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<EntityId, InteractionRegistrationError> {
        validate_interactable_geometry(position, interaction_range)?;
        let entity = self.entities.create();
        self.interactables.insert(
            entity,
            Interactable::bridge_door(entity, position, interaction_range),
        );
        Ok(entity)
    }
    pub fn create_bridge_door_with_requirement(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        requirement: DoorAccessRequirement,
    ) -> Result<EntityId, InteractionRegistrationError> {
        validate_interactable_geometry(position, interaction_range)?;
        let entity = self.entities.create();
        self.interactables.insert(
            entity,
            Interactable::bridge_door_with_requirement(
                entity,
                position,
                interaction_range,
                requirement,
            ),
        );
        Ok(entity)
    }
    pub fn create_station(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        station_type: StationType,
    ) -> Result<EntityId, InteractionRegistrationError> {
        validate_interactable_geometry(position, interaction_range)?;
        let entity = self.entities.create();
        self.interactables.insert(
            entity,
            Interactable::station(
                entity,
                position,
                interaction_range,
                Station::new(station_type),
            ),
        );
        Ok(entity)
    }
    pub fn create_electrical_panel(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        configuration: ElectricalPanelConfiguration,
    ) -> Result<EntityId, ElectricalPanelRegistrationError> {
        ElectricalPanel::validate_configuration(position, interaction_range, &configuration)?;
        let entity = self.entities.create();
        let panel = match ElectricalPanel::new(entity, position, interaction_range, configuration) {
            Ok(panel) => panel,
            Err(error) => {
                self.entities.deactivate(entity);
                return Err(error);
            }
        };
        self.interactables.insert(
            entity,
            Interactable::electrical_panel(entity, position, interaction_range),
        );
        self.electrical_panels.insert(entity, panel);
        Ok(entity)
    }
    pub fn create_life_support_panel(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        configuration: LifeSupportConfiguration,
    ) -> Result<EntityId, LifeSupportRegistrationError> {
        LifeSupportPanel::validate_configuration(position, interaction_range, &configuration)?;
        let entity = self.entities.create();
        let panel = match LifeSupportPanel::new(entity, position, interaction_range, configuration)
        {
            Ok(panel) => panel,
            Err(error) => {
                self.entities.deactivate(entity);
                return Err(error);
            }
        };
        self.interactables.insert(
            entity,
            Interactable::life_support_panel(entity, position, interaction_range),
        );
        self.life_support_panels.insert(entity, panel);
        Ok(entity)
    }
    pub fn create_player(&mut self) -> Result<EntityId, PlayerCreationError> {
        if self.players.len() >= MAX_LOCAL_PLAYERS {
            return Err(PlayerCreationError::MaximumPlayersReached);
        }
        let entity = self.entities.create();
        let player = Player::new(entity);
        self.players.insert(entity, player);
        self.combatants.insert(entity, Combatant::player(entity));
        self.currencies.insert(entity, 0);
        self.events
            .push(crate::events::GameplayEvent::PlayerCreated {
                player: player.id(),
                position: player.position,
            });
        Ok(entity)
    }
    pub fn initialize_inventory(
        &mut self,
        player: EntityId,
        capacity: u32,
    ) -> Result<(), InventoryError> {
        if !self.players.contains_key(&player) {
            return Err(InventoryError::PlayerNotFound);
        }
        if self.inventories.contains_key(&player) {
            return Err(InventoryError::AlreadyInitialized);
        }
        self.inventories.insert(player, Inventory::new(capacity)?);
        self.events
            .push(crate::events::GameplayEvent::InventoryInitialized { player, capacity });
        Ok(())
    }
    pub(crate) fn transfer_inventory_item(
        &mut self,
        from: EntityId,
        to: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> Result<(), InventoryTransferError> {
        let sender = self.player(from).ok_or(InventoryTransferError::PlayerNotFound)?;
        let receiver = self.player(to).ok_or(InventoryTransferError::PlayerNotFound)?;
        if from == to {
            return Err(InventoryTransferError::SamePlayer);
        }
        if !self.entities.contains(from)
            || !sender.is_active
            || !self.entities.contains(to)
            || !receiver.is_active
        {
            return Err(InventoryTransferError::PlayerInactive);
        }
        if amount == 0 {
            return Err(InventoryTransferError::InvalidQuantity);
        }
        let definition = self
            .item_definition(item)
            .ok_or(InventoryTransferError::ItemDefinitionNotFound)?;
        if amount > definition.max_stack {
            return Err(InventoryTransferError::InvalidQuantity);
        }
        let source = self
            .inventories
            .get(&from)
            .cloned()
            .ok_or(InventoryTransferError::InventoryNotInitialized)?;
        let target = self
            .inventories
            .get(&to)
            .cloned()
            .ok_or(InventoryTransferError::InventoryNotInitialized)?;
        if source.quantity(item) < amount {
            return Err(InventoryTransferError::InsufficientQuantity);
        }
        let target_quantity = target
            .quantity(item)
            .checked_add(amount)
            .ok_or(InventoryTransferError::ArithmeticOverflow)?;
        if target_quantity > definition.max_stack {
            return Err(InventoryTransferError::StackExceeded);
        }
        if !target.can_add(amount) {
            return Err(InventoryTransferError::CapacityExceeded);
        }

        let mut updated_source = source;
        let mut updated_target = target;
        updated_source
            .try_remove(item, amount)
            .map_err(|error| match error {
                InventoryError::InsufficientQuantity => InventoryTransferError::InsufficientQuantity,
                InventoryError::InvalidQuantity => InventoryTransferError::InvalidQuantity,
                _ => InventoryTransferError::ArithmeticOverflow,
            })?;
        updated_target
            .try_add(item, amount)
            .map_err(|error| match error {
                InventoryError::CapacityExceeded => InventoryTransferError::CapacityExceeded,
                InventoryError::InvalidQuantity => InventoryTransferError::InvalidQuantity,
                InventoryError::ArithmeticOverflow => InventoryTransferError::ArithmeticOverflow,
                InventoryError::InsufficientQuantity | InventoryError::PlayerNotFound
                | InventoryError::AlreadyInitialized
                | InventoryError::InvalidCapacity => InventoryTransferError::ArithmeticOverflow,
            })?;
        self.inventories.insert(from, updated_source);
        self.inventories.insert(to, updated_target);
        self.publish(crate::events::GameplayEvent::ItemRemoved {
            player: from,
            item,
            amount,
        });
        self.publish(crate::events::GameplayEvent::ItemAdded {
            player: crate::types::PlayerId(to),
            item,
            amount,
        });
        Ok(())
    }
    pub fn register_item_definition(
        &mut self,
        definition: ItemDefinition,
    ) -> Result<(), ItemDefinitionError> {
        definition.validate()?;
        if self.item_definitions.contains_key(&definition.id) {
            return Err(ItemDefinitionError::DuplicateItem);
        }
        self.item_definitions.insert(definition.id, definition);
        Ok(())
    }
    pub fn register_enemy_reward(&mut self, reward: EnemyReward) -> Result<(), RewardError> {
        reward
            .validate()
            .map_err(|_| RewardError::InvalidConfiguration)?;
        if self.enemy_rewards.contains_key(&reward.archetype) {
            return Err(RewardError::DuplicateReward);
        }
        for entry in &reward.entries {
            let Some(definition) = self.item_definitions.get(&entry.item) else {
                return Err(RewardError::ItemDefinitionNotFound);
            };
            if entry.amount > definition.max_stack {
                return Err(RewardError::AmountExceedsStack);
            }
        }
        self.enemy_rewards.insert(reward.archetype, reward);
        Ok(())
    }
    pub fn configure_enemy_reward_distribution(
        &mut self,
        archetype: crate::combat::EnemyArchetype,
        distribution: RewardDistributionPolicy,
    ) -> Result<(), RewardError> {
        let reward = self
            .enemy_rewards
            .get_mut(&archetype)
            .ok_or(RewardError::RewardNotRegistered)?;
        reward.distribution = distribution;
        Ok(())
    }

    pub fn create_collectible(
        &mut self,
        item: crate::types::ItemId,
        amount: u32,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<EntityId, CollectibleError> {
        let Some(definition) = self.item_definitions.get(&item) else {
            return Err(CollectibleError::ItemNotFound);
        };
        if amount > definition.max_stack {
            return Err(CollectibleError::InvalidAmount);
        }
        Collectible::validate_configuration(amount, position, interaction_range)?;
        let entity = self.entities.create();
        let collectible = Collectible {
            entity,
            item,
            amount,
            position,
            interaction_range,
            owner: None,
            active: true,
        };
        if let Err(error) = collectible.validate() {
            self.entities.deactivate(entity);
            return Err(error);
        }
        self.collectibles.insert(entity, collectible);
        Ok(entity)
    }
    pub fn deactivate_entity(&mut self, entity: EntityId) -> bool {
        let deactivated = self.entities.deactivate(entity);
        if deactivated {
            self.interactables.remove(&entity);
            self.electrical_panels.remove(&entity);
            self.life_support_panels.remove(&entity);
            self.ship_resources.remove(&entity);
            self.ship_integrity.remove(&entity);
            if let Some(combatant) = self.combatants.get_mut(&entity) {
                combatant.active = false;
            }
            if let Some(collectible) = self.collectibles.get_mut(&entity) {
                collectible.active = false;
            }
            if let Some(player) = self.players.get_mut(&entity) {
                player.is_active = false;
                self.events
                    .push(crate::events::GameplayEvent::PlayerDeactivated {
                        player: player.id(),
                    });
            }
        }
        deactivated
    }
    pub fn take_events(&mut self) -> Vec<crate::events::GameplayEvent> {
        self.events.take()
    }

    pub(crate) fn clear_events(&mut self) {
        self.events.clear();
    }
    pub(crate) fn set_time(&mut self, time: GameTime) {
        self.state.time = time;
    }
    pub(crate) fn publish(&mut self, event: crate::events::GameplayEvent) {
        self.events.push(event);
    }
    pub(crate) fn knowledge_target_exists(&self, target: KnowledgeTarget) -> bool {
        match target {
            KnowledgeTarget::Destination(destination) => self.destination(destination).is_some(),
            KnowledgeTarget::Enemy(enemy) => self
                .combatant(enemy)
                .is_some_and(|combatant| !combatant.is_player),
            KnowledgeTarget::Item(item) => self.item_definition(item).is_some(),
        }
    }
    pub(crate) fn register_knowledge(
        &mut self,
        target: KnowledgeTarget,
    ) -> Result<bool, crate::knowledge::KnowledgeError> {
        if !self.knowledge_target_exists(target) {
            return Err(crate::knowledge::KnowledgeError::TargetNotFound);
        }
        if self.knowledge.contains_key(&target) {
            return Err(crate::knowledge::KnowledgeError::AlreadyRegistered);
        }
        self.knowledge.insert(target, KnowledgeState::Unknown);
        Ok(true)
    }
    pub(crate) fn set_knowledge(
        &mut self,
        target: KnowledgeTarget,
        state: KnowledgeState,
    ) -> Result<bool, crate::knowledge::KnowledgeError> {
        if !self.knowledge_target_exists(target) {
            return Err(crate::knowledge::KnowledgeError::TargetNotFound);
        }
        let current = self.knowledge(target);
        if state < current {
            return Err(crate::knowledge::KnowledgeError::TransitionNotAllowed);
        }
        if current == state {
            return Ok(false);
        }
        self.knowledge.insert(target, state);
        Ok(true)
    }
    pub(crate) fn player_mut(&mut self, entity: EntityId) -> Option<&mut Player> {
        self.players.get_mut(&entity)
    }
    pub(crate) fn combatant_mut(&mut self, entity: EntityId) -> Option<&mut Combatant> {
        self.combatants.get_mut(&entity)
    }
    pub(crate) fn inventory_mut(&mut self, player: EntityId) -> Option<&mut Inventory> {
        self.inventories.get_mut(&player)
    }
    pub(crate) fn use_healing_consumable(
        &mut self,
        player: EntityId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> Result<HealingResult, HealingError> {
        let player_state = self.player(player).ok_or(HealingError::PlayerNotFound)?;
        if !self.entities.contains(player) || !player_state.is_active {
            return Err(HealingError::PlayerInactive);
        }
        let combatant = self
            .combatant(player)
            .ok_or(HealingError::CombatantInvalid)?;
        if !combatant.is_player {
            return Err(HealingError::CombatantInvalid);
        }
        if combatant.is_defeated() {
            return Err(HealingError::CombatantDefeated);
        }
        if combatant.current_health > combatant.maximum_health || combatant.maximum_health == 0 {
            return Err(HealingError::InvalidHealth);
        }
        if combatant.current_health == combatant.maximum_health {
            return Err(HealingError::HealthAtMaximum);
        }
        if amount == 0 {
            return Err(HealingError::InvalidAmount);
        }
        let definition = self
            .item_definition(item)
            .ok_or(HealingError::ItemDefinitionNotFound)?;
        if definition.category != crate::collectible::ItemCategory::HealingConsumable {
            return Err(HealingError::NotHealingConsumable);
        }
        let effect = definition
            .healing
            .ok_or(HealingError::NotHealingConsumable)?;
        if !effect.is_valid() {
            return Err(HealingError::NotHealingConsumable);
        }
        let inventory = self
            .inventory(player)
            .ok_or(HealingError::InventoryNotInitialized)?;
        if inventory.quantity(item) < amount {
            return Err(HealingError::InsufficientQuantity);
        }
        let requested = effect
            .health_per_unit
            .checked_mul(amount)
            .ok_or(HealingError::ArithmeticOverflow)?;
        let health_before = combatant.current_health;
        let restored = requested.min(combatant.maximum_health - health_before);
        if restored == 0 {
            return Err(HealingError::HealthAtMaximum);
        }

        let mut inventory_after = inventory.clone();
        inventory_after
            .try_remove(item, amount)
            .map_err(|_| HealingError::InsufficientQuantity)?;
        let mut combatant_after = *combatant;
        combatant_after.current_health = health_before.saturating_add(restored);
        if combatant_after.current_health > combatant_after.maximum_health {
            return Err(HealingError::InvalidHealth);
        }
        self.inventories.insert(player, inventory_after);
        self.combatants.insert(player, combatant_after);
        self.publish(crate::events::GameplayEvent::ItemRemoved {
            player,
            item,
            amount,
        });
        self.publish(crate::events::GameplayEvent::PlayerHealed {
            player,
            item,
            consumed: amount,
            health_before,
            restored,
            health_after: combatant_after.current_health,
        });
        Ok(HealingResult {
            player,
            item,
            consumed: amount,
            health_before,
            restored,
            health_after: combatant_after.current_health,
        })
    }
    pub(crate) fn revive_player(
        &mut self,
        helper: EntityId,
        target: EntityId,
        item: crate::types::ItemId,
    ) -> Result<ReviveResult, RevivalError> {
        let helper_player = self.player(helper).ok_or(RevivalError::HelperNotFound)?;
        if !self.entities.contains(helper) || !helper_player.is_active {
            return Err(RevivalError::HelperInactive);
        }
        let helper_combatant = self.combatant(helper).ok_or(RevivalError::HelperInvalid)?;
        if !helper_combatant.is_player
            || !helper_combatant.active
            || helper_combatant.is_defeated()
            || helper_combatant.current_health > helper_combatant.maximum_health
        {
            return Err(RevivalError::HelperInvalid);
        }
        let target_player = self.player(target).ok_or(RevivalError::TargetNotFound)?;
        let target_combatant = self.combatant(target).ok_or(RevivalError::TargetInvalid)?;
        if !target_combatant.is_player || helper == target {
            return Err(RevivalError::TargetInvalid);
        }
        if target_combatant.current_health != 0
            || target_combatant.active
            || target_player.is_active
            || self.entities.contains(target)
        {
            return Err(RevivalError::TargetNotDefeated);
        }
        if target_combatant.maximum_health == 0 {
            return Err(RevivalError::TargetInvalid);
        }
        let helper_position = helper_player.position;
        let target_position = target_player.position;
        if !finite_vec3(helper_position) || !finite_vec3(target_position) {
            return Err(RevivalError::InvalidPosition);
        }
        let effect = self
            .revival_effect(item)
            .ok_or(RevivalError::NotRevivalConsumable)?;
        let dx = (helper_position.x - target_position.x) as f64;
        let dy = (helper_position.y - target_position.y) as f64;
        let dz = (helper_position.z - target_position.z) as f64;
        let distance_squared = dx * dx + dy * dy + dz * dz;
        let range_squared = effect.range as f64 * effect.range as f64;
        if !distance_squared.is_finite() || distance_squared > range_squared {
            return Err(RevivalError::OutOfRange);
        }
        let definition = self
            .item_definition(item)
            .ok_or(RevivalError::ItemDefinitionNotFound)?;
        if definition.category != crate::collectible::ItemCategory::RevivalConsumable
            || definition.revival != Some(effect)
            || !effect.is_valid()
        {
            return Err(RevivalError::InvalidConfiguration);
        }
        let inventory = self
            .inventory(helper)
            .ok_or(RevivalError::InventoryNotInitialized)?;
        if inventory.quantity(item) < effect.units_per_revival {
            return Err(RevivalError::InsufficientQuantity);
        }
        let health_after = effect.health_restored.min(target_combatant.maximum_health);
        if health_after == 0 {
            return Err(RevivalError::InvalidConfiguration);
        }
        let mut inventory_after = inventory.clone();
        inventory_after
            .try_remove(item, effect.units_per_revival)
            .map_err(|_| RevivalError::InsufficientQuantity)?;
        let mut target_after = *target_combatant;
        target_after.current_health = health_after;
        target_after.active = true;
        let mut target_player_after = *target_player;
        target_player_after.is_active = true;

        self.inventories.insert(helper, inventory_after);
        self.combatants.insert(target, target_after);
        self.players.insert(target, target_player_after);
        self.entities.activate(target);
        self.publish(crate::events::GameplayEvent::ItemRemoved {
            player: helper,
            item,
            amount: effect.units_per_revival,
        });
        self.publish(crate::events::GameplayEvent::PlayerRevived {
            helper,
            target,
            item,
            consumed: effect.units_per_revival,
            health_before: 0,
            restored: health_after,
            health_after,
        });
        Ok(ReviveResult {
            helper,
            target,
            item,
            consumed: effect.units_per_revival,
            health_before: 0,
            restored: health_after,
            health_after,
        })
    }
    fn revival_preview(
        &self,
        helper: EntityId,
        target: EntityId,
        item: crate::types::ItemId,
    ) -> Result<(), RevivalError> {
        if self.phase() != SimulationPhase::Running {
            return Err(RevivalError::HelperInactive);
        }
        let helper_player = self.player(helper).ok_or(RevivalError::HelperNotFound)?;
        let target_player = self.player(target).ok_or(RevivalError::TargetNotFound)?;
        let helper_combatant = self.combatant(helper).ok_or(RevivalError::HelperInvalid)?;
        let target_combatant = self.combatant(target).ok_or(RevivalError::TargetInvalid)?;
        if !self.entities.contains(helper)
            || !helper_player.is_active
            || !helper_combatant.is_player
            || !helper_combatant.active
            || helper_combatant.is_defeated()
        {
            return Err(RevivalError::HelperInvalid);
        }
        if helper == target
            || !target_combatant.is_player
            || target_combatant.current_health != 0
            || target_combatant.active
            || target_player.is_active
            || self.entities.contains(target)
            || target_combatant.maximum_health == 0
        {
            return Err(RevivalError::TargetNotDefeated);
        }
        if !finite_vec3(helper_player.position) || !finite_vec3(target_player.position) {
            return Err(RevivalError::InvalidPosition);
        }
        let effect = self
            .revival_effect(item)
            .ok_or(RevivalError::NotRevivalConsumable)?;
        if !effect.is_valid() {
            return Err(RevivalError::InvalidConfiguration);
        }
        let dx = (helper_player.position.x - target_player.position.x) as f64;
        let dy = (helper_player.position.y - target_player.position.y) as f64;
        let dz = (helper_player.position.z - target_player.position.z) as f64;
        let distance_squared = dx * dx + dy * dy + dz * dz;
        let range_squared = effect.range as f64 * effect.range as f64;
        if !distance_squared.is_finite() || distance_squared > range_squared {
            return Err(RevivalError::OutOfRange);
        }
        self.inventory(helper)
            .filter(|inventory| inventory.quantity(item) >= effect.units_per_revival)
            .ok_or(RevivalError::InventoryNotInitialized)
            .map(|_| ())
    }
    pub(crate) fn collectible_mut(&mut self, entity: EntityId) -> Option<&mut Collectible> {
        self.collectibles.get_mut(&entity)
    }
    pub(crate) fn register_enemy(
        &mut self,
        configuration: EnemyConfiguration,
    ) -> Result<EntityId, crate::combat::CombatConfigurationError> {
        configuration.validate()?;
        let entity = self.entities.create();
        self.combatants
            .insert(entity, Combatant::enemy(entity, configuration));
        Ok(entity)
    }
    pub(crate) fn create_enemy_reward_collectibles(
        &mut self,
        enemy: EntityId,
        archetype: crate::combat::EnemyArchetype,
        position: engine::Vec3,
    ) -> Result<Vec<Collectible>, RewardError> {
        if self.processed_enemy_rewards.contains(&enemy) {
            return Err(RewardError::RewardAlreadyProcessed);
        }
        let Some(enemy_state) = self.combatant(enemy) else {
            return Err(RewardError::EnemyNotFound);
        };
        if enemy_state.is_player || enemy_state.archetype != Some(archetype) {
            return Err(RewardError::EnemyNotFound);
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(RewardError::InvalidPosition);
        }
        let Some(reward) = self.enemy_rewards.get(&archetype).cloned() else {
            self.processed_enemy_rewards.insert(enemy);
            return Ok(Vec::new());
        };
        for entry in &reward.entries {
            let Some(definition) = self.item_definitions.get(&entry.item) else {
                return Err(RewardError::ItemDefinitionNotFound);
            };
            if entry.amount > definition.max_stack {
                return Err(RewardError::AmountExceedsStack);
            }
        }
        let eligible_players: Vec<_> = match reward.distribution {
            RewardDistributionPolicy::Shared => vec![None],
            RewardDistributionPolicy::Personal => self
                .players
                .iter()
                .filter(|(player, state)| self.entities.contains(**player) && state.is_active)
                .map(|(player, _)| Some(*player))
                .collect(),
        };
        let collectible_specs: Vec<_> = reward
            .entries
            .iter()
            .flat_map(|entry| {
                eligible_players
                    .iter()
                    .map(move |owner| (entry.item, entry.amount, *owner))
            })
            .collect();
        let collectibles: Vec<_> = collectible_specs
            .into_iter()
            .map(|(item, amount, owner)| Collectible {
                entity: self.entities.create(),
                item,
                amount,
                position,
                interaction_range: 2.0,
                owner,
                active: true,
            })
            .collect();
        self.collectibles
            .extend(collectibles.iter().copied().map(|item| (item.entity, item)));
        self.processed_enemy_rewards.insert(enemy);
        Ok(collectibles)
    }
    pub(crate) fn interactable_mut(&mut self, entity: EntityId) -> Option<&mut Interactable> {
        self.interactables.get_mut(&entity)
    }
    pub(crate) fn electrical_panel_mut(
        &mut self,
        entity: EntityId,
    ) -> Option<&mut ElectricalPanel> {
        self.electrical_panels.get_mut(&entity)
    }
    pub(crate) fn life_support_panel_mut(
        &mut self,
        entity: EntityId,
    ) -> Option<&mut LifeSupportPanel> {
        self.life_support_panels.get_mut(&entity)
    }
    pub(crate) fn navigation_mut(&mut self) -> &mut NavigationState {
        &mut self.navigation
    }
    pub(crate) fn spatial_encounter_mut(
        &mut self,
        encounter: EncounterId,
    ) -> Option<&mut SpatialEncounter> {
        self.spatial_encounters.get_mut(&encounter)
    }
    pub(crate) fn derelict_mut(&mut self, id: DerelictId) -> Option<&mut Derelict> {
        self.derelicts.get_mut(&id)
    }
    pub(crate) fn functional_world_mut(
        &mut self,
        destination: DestinationId,
    ) -> Option<&mut FunctionalWorld> {
        self.functional_worlds.get_mut(&destination)
    }
    fn validate_world_access(
        &self,
        player: EntityId,
        destination: DestinationId,
    ) -> Result<FunctionalWorld, WorldActivityError> {
        let Some(player_state) = self.player(player) else {
            return Err(WorldActivityError::PlayerNotFound);
        };
        let Some(combatant) = self.combatant(player) else {
            return Err(WorldActivityError::PlayerInactive);
        };
        if !self.entities.contains(player) || !player_state.is_active || !combatant.is_player {
            return Err(WorldActivityError::PlayerInactive);
        }
        let Some(ship) = self.navigation.ship_id() else {
            return Err(WorldActivityError::ShipNotOperational);
        };
        if self.ship_is_operational(ship) != Some(true) {
            return Err(WorldActivityError::ShipNotOperational);
        }
        let Some(world) = self.functional_world(destination).cloned() else {
            return Err(WorldActivityError::WorldNotFound);
        };
        let Some(destination_data) = self.destination(destination) else {
            return Err(WorldActivityError::WorldNotFound);
        };
        if destination_data.discovery != crate::navigation::DiscoveryState::Discovered {
            return Err(WorldActivityError::DestinationNotDiscovered);
        }
        let Some(position) = self.ship_position() else {
            return Err(WorldActivityError::OutOfRange);
        };
        let Some(distance) = crate::encounter::distance_squared(position, destination_data.position)
        else {
            return Err(WorldActivityError::OutOfRange);
        };
        if distance
            > f64::from(world.interaction_range) * f64::from(world.interaction_range)
        {
            return Err(WorldActivityError::OutOfRange);
        }
        Ok(world)
    }
    pub(crate) fn buy_world_item(
        &mut self,
        player: EntityId,
        destination: DestinationId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> Result<u64, WorldActivityError> {
        let world = self.validate_world_access(player, destination)?;
        if amount == 0 {
            return Err(WorldActivityError::InvalidQuantity);
        }
        let offer = world
            .activities
            .iter()
            .find_map(|activity| match activity {
                WorldActivity::Trade(market) => market.offers.iter().find(|offer| offer.item == item),
                _ => None,
            })
            .cloned()
            .ok_or(WorldActivityError::ActivityNotFound)?;
        if offer.stock < amount {
            return Err(WorldActivityError::ResourceUnavailable);
        }
        if self.item_definition(item).is_none() {
            return Err(WorldActivityError::InvalidItem);
        }
        let total = offer
            .buy_price
            .checked_mul(u64::from(amount))
            .ok_or(WorldActivityError::PriceOverflow)?;
        let balance = self
            .player_currency(player)
            .ok_or(WorldActivityError::PlayerNotFound)?;
        if balance < total {
            return Err(WorldActivityError::InsufficientFunds);
        }
        let inventory = self
            .inventory(player)
            .ok_or(WorldActivityError::InventoryCapacity)?;
        let mut inventory_after = inventory.clone();
        inventory_after
            .try_add(item, amount)
            .map_err(|error| match error {
                InventoryError::CapacityExceeded => WorldActivityError::InventoryCapacity,
                _ => WorldActivityError::InvalidQuantity,
            })?;
        let Some(world_mut) = self.functional_worlds.get_mut(&destination) else {
            return Err(WorldActivityError::WorldNotFound);
        };
        let Some(WorldActivity::Trade(market)) = world_mut
            .activities
            .iter_mut()
            .find(|activity| activity.category() == crate::world_activity::WorldCategory::Trade)
        else {
            return Err(WorldActivityError::ActivityNotFound);
        };
        let Some(offer_mut) = market.offers.iter_mut().find(|offer| offer.item == item) else {
            return Err(WorldActivityError::ActivityNotFound);
        };
        offer_mut.stock -= amount;
        self.inventories.insert(player, inventory_after);
        self.currencies.insert(player, balance - total);
        self.publish(crate::events::GameplayEvent::WorldPurchaseCompleted {
            player,
            destination,
            item,
            amount,
            total_price: total,
        });
        Ok(total)
    }
    pub(crate) fn sell_world_item(
        &mut self,
        player: EntityId,
        destination: DestinationId,
        item: crate::types::ItemId,
        amount: u32,
    ) -> Result<u64, WorldActivityError> {
        let world = self.validate_world_access(player, destination)?;
        if amount == 0 {
            return Err(WorldActivityError::InvalidQuantity);
        }
        let offer = world
            .activities
            .iter()
            .find_map(|activity| match activity {
                WorldActivity::Trade(market) => market.offers.iter().find(|offer| offer.item == item),
                _ => None,
            })
            .cloned()
            .ok_or(WorldActivityError::ActivityNotFound)?;
        let total = offer
            .sell_price
            .checked_mul(u64::from(amount))
            .ok_or(WorldActivityError::PriceOverflow)?;
        let inventory = self
            .inventory(player)
            .ok_or(WorldActivityError::InventoryInsufficient)?;
        let mut inventory_after = inventory.clone();
        inventory_after
            .try_remove(item, amount)
            .map_err(|_| WorldActivityError::InventoryInsufficient)?;
        let balance = self
            .player_currency(player)
            .ok_or(WorldActivityError::PlayerNotFound)?;
        let balance_after = balance
            .checked_add(total)
            .ok_or(WorldActivityError::PriceOverflow)?;
        self.inventories.insert(player, inventory_after);
        self.currencies.insert(player, balance_after);
        self.publish(crate::events::GameplayEvent::WorldSaleCompleted {
            player,
            destination,
            item,
            amount,
            total_value: total,
        });
        Ok(total)
    }
    pub(crate) fn extract_world_resource(
        &mut self,
        player: EntityId,
        destination: DestinationId,
    ) -> Result<(crate::types::ItemId, u32, u32), WorldActivityError> {
        let world = self.validate_world_access(player, destination)?;
        let site = world
            .activities
            .iter()
            .find_map(|activity| match activity {
                WorldActivity::Resource(site) => Some(*site),
                _ => None,
            })
            .ok_or(WorldActivityError::ActivityNotFound)?;
        let scaling = self.current_group_scaling();
        let scaled_extraction = scaling
            .scale_amount(site.extraction_amount, scaling.resource_yield_percent)
            .map_err(|_| WorldActivityError::InvalidQuantity)?;
        let amount = scaled_extraction.min(site.available);
        if amount == 0 {
            return Err(WorldActivityError::ResourceUnavailable);
        }
        let inventory = self
            .inventory(player)
            .ok_or(WorldActivityError::InventoryCapacity)?;
        let mut inventory_after = inventory.clone();
        inventory_after
            .try_add(site.item, amount)
            .map_err(|error| match error {
                InventoryError::CapacityExceeded => WorldActivityError::InventoryCapacity,
                _ => WorldActivityError::InvalidQuantity,
            })?;
        let Some(world_mut) = self.functional_worlds.get_mut(&destination) else {
            return Err(WorldActivityError::WorldNotFound);
        };
        let Some(WorldActivity::Resource(site_mut)) = world_mut
            .activities
            .iter_mut()
            .find(|activity| activity.category() == crate::world_activity::WorldCategory::Resource)
        else {
            return Err(WorldActivityError::ActivityNotFound);
        };
        site_mut.available -= amount;
        self.inventories.insert(player, inventory_after);
        self.publish(crate::events::GameplayEvent::WorldResourceExtracted {
            player,
            destination,
            item: site.item,
            amount,
            remaining: site.available - amount,
        });
        Ok((site.item, amount, site.available - amount))
    }
    pub(crate) fn activate_world_combat(
        &mut self,
        player: EntityId,
        destination: DestinationId,
    ) -> Result<EntityId, WorldActivityError> {
        let world = self.validate_world_access(player, destination)?;
        let zone = world
            .activities
            .iter()
            .find_map(|activity| match activity {
                WorldActivity::Combat(zone) => Some(*zone),
                _ => None,
            })
            .ok_or(WorldActivityError::ActivityNotFound)?;
        if zone.resolved {
            return Err(WorldActivityError::CombatResolved);
        }
        if zone.active_enemy.is_some() {
            return Err(WorldActivityError::CombatAlreadyActivated);
        }
        let configuration = self
            .scaled_enemy_configuration(zone.configuration)
            .map_err(|_| WorldActivityError::EnemyCreationFailed)?;
        let enemy = self
            .register_enemy(configuration)
            .map_err(|_| WorldActivityError::EnemyCreationFailed)?;
        let Some(world_mut) = self.functional_worlds.get_mut(&destination) else {
            return Err(WorldActivityError::WorldNotFound);
        };
        let Some(WorldActivity::Combat(zone_mut)) = world_mut
            .activities
            .iter_mut()
            .find(|activity| activity.category() == crate::world_activity::WorldCategory::Combat)
        else {
            return Err(WorldActivityError::ActivityNotFound);
        };
        zone_mut.active_enemy = Some(enemy);
        self.publish(crate::events::GameplayEvent::EnemyCreated {
            enemy,
            archetype: configuration.archetype,
            position: configuration.position,
        });
        self.publish(crate::events::GameplayEvent::WorldCombatActivated {
            player,
            destination,
            enemy,
        });
        Ok(enemy)
    }
    pub(crate) fn interact_story_world(
        &mut self,
        player: EntityId,
        destination: DestinationId,
    ) -> Result<crate::knowledge::KnowledgeTarget, WorldActivityError> {
        let world = self.validate_world_access(player, destination)?;
        let story = world
            .activities
            .iter()
            .find_map(|activity| match activity {
                WorldActivity::Story(story) => Some(*story),
                _ => None,
            })
            .ok_or(WorldActivityError::ActivityNotFound)?;
        if story.interacted {
            return Err(WorldActivityError::StoryAlreadyInteracted);
        }
        if !self.knowledge_target_exists(story.target) {
            return Err(WorldActivityError::StoryKnowledgeUnavailable);
        }
        let current = self.knowledge(story.target);
        if current == crate::knowledge::KnowledgeState::Unknown {
            let _ = self.register_knowledge(story.target);
        }
        if self
            .set_knowledge(story.target, crate::knowledge::KnowledgeState::Identified)
            .is_err()
        {
            return Err(WorldActivityError::StoryKnowledgeUnavailable);
        }
        let Some(world_mut) = self.functional_worlds.get_mut(&destination) else {
            return Err(WorldActivityError::WorldNotFound);
        };
        let Some(WorldActivity::Story(story_mut)) = world_mut
            .activities
            .iter_mut()
            .find(|activity| activity.category() == crate::world_activity::WorldCategory::Story)
        else {
            return Err(WorldActivityError::ActivityNotFound);
        };
        story_mut.interacted = true;
        self.publish(crate::events::GameplayEvent::WorldStoryInteracted {
            player,
            destination,
            target: story.target,
        });
        Ok(story.target)
    }
    pub(crate) fn create_owned_collectible(
        &mut self,
        item: crate::types::ItemId,
        amount: u32,
        position: engine::Vec3,
        interaction_range: f32,
        owner: Option<EntityId>,
    ) -> Result<EntityId, CollectibleError> {
        let Some(definition) = self.item_definitions.get(&item) else {
            return Err(CollectibleError::ItemNotFound);
        };
        if amount > definition.max_stack {
            return Err(CollectibleError::InvalidAmount);
        }
        if let Some(owner) = owner {
            if !self.players.contains_key(&owner) {
                return Err(CollectibleError::EntityNotFound);
            }
        }
        Collectible::validate_configuration(amount, position, interaction_range)?;
        let entity = self.entities.create();
        self.collectibles.insert(
            entity,
            Collectible {
                entity,
                item,
                amount,
                position,
                interaction_range,
                owner,
                active: true,
            },
        );
        Ok(entity)
    }
    pub(crate) fn create_derelict_collectibles(
        &mut self,
        derelict: DerelictId,
    ) -> Result<Vec<(EntityId, crate::types::ItemId, u32, Option<EntityId>)>, DerelictError> {
        let definition = self
            .derelict(derelict)
            .cloned()
            .ok_or(DerelictError::DuplicateId)?;
        let owners: Vec<Option<EntityId>> = match definition.distribution {
            crate::reward::RewardDistributionPolicy::Shared => vec![None],
            crate::reward::RewardDistributionPolicy::Personal => self
                .players
                .iter()
                .filter(|(player, value)| self.entities.contains(**player) && value.is_active)
                .map(|(player, _)| Some(*player))
                .collect(),
        };
        for reward in &definition.rewards {
            let Some(item) = self.item_definition(reward.item) else {
                return Err(DerelictError::ItemDefinitionNotFound);
            };
            if reward.amount == 0 || reward.amount > item.max_stack {
                return Err(DerelictError::InvalidReward);
            }
        }
        let mut created = Vec::new();
        for reward in &definition.rewards {
            for owner in &owners {
                let entity = self
                    .create_owned_collectible(
                        reward.item,
                        reward.amount,
                        definition.position,
                        definition.interaction_range,
                        *owner,
                    )
                    .map_err(|_| DerelictError::InvalidReward)?;
                created.push((entity, reward.item, reward.amount, *owner));
            }
        }
        Ok(created)
    }
    pub(crate) fn repair_ship_at_station(
        &mut self,
        player: EntityId,
        ship: EntityId,
        station: EntityId,
    ) -> Result<u32, ShipRepairError> {
        let configuration = self.ship_repair_configuration();
        if !configuration.validate() {
            return Err(ShipRepairError::InvalidConfiguration);
        }
        let player_state = self.player(player).ok_or(ShipRepairError::PlayerNotFound)?;
        if !self.entities.contains(player) || !player_state.is_active {
            return Err(ShipRepairError::PlayerInactive);
        }
        if self.navigation.ship_id() != Some(ship) || !self.entities.contains(ship) {
            return Err(ShipRepairError::ShipNotFound);
        }
        let integrity = self
            .ship_integrity(ship)
            .ok_or(ShipRepairError::ShipIntegrityNotInitialized)?;
        if integrity.is_destroyed() {
            return Err(ShipRepairError::ShipDestroyed);
        }
        let station_data = self
            .interactables
            .get(&station)
            .ok_or(ShipRepairError::StationNotFound)?;
        if !station_data.enabled {
            return Err(ShipRepairError::StationDisabled);
        }
        let InteractionBehavior::Station {
            station: station_state,
        } = station_data.behavior
        else {
            return Err(ShipRepairError::WrongStationType);
        };
        if station_state.station_type != StationType::Engineering {
            return Err(ShipRepairError::WrongStationType);
        }
        if station_state.occupant != Some(player) {
            return Err(ShipRepairError::StationNotOccupiedByPlayer);
        }
        let definition = self
            .item_definitions
            .get(&configuration.material)
            .ok_or(ShipRepairError::RepairMaterialNotFound)?;
        if definition.category != crate::collectible::ItemCategory::RepairMaterial {
            return Err(ShipRepairError::RepairMaterialNotFound);
        }
        let inventory = self
            .inventories
            .get(&player)
            .ok_or(ShipRepairError::InventoryNotInitialized)?;
        if inventory.quantity(configuration.material) < configuration.material_cost {
            return Err(ShipRepairError::InsufficientMaterial);
        }
        let missing = integrity.maximum().saturating_sub(integrity.current());
        let amount = configuration.integrity_restored.min(missing);
        if amount == 0 {
            return Err(ShipRepairError::ShipAtMaximum);
        }

        let mut inventory_after = inventory.clone();
        inventory_after
            .try_remove(configuration.material, configuration.material_cost)
            .map_err(|_| ShipRepairError::InsufficientMaterial)?;
        let mut integrity_after = integrity;
        let (current, _before, state) = integrity_after
            .repair(amount)
            .map_err(|_| ShipRepairError::InvalidConfiguration)?;

        self.inventories.insert(player, inventory_after);
        self.ship_integrity.insert(ship, integrity_after);
        self.publish(crate::events::GameplayEvent::ItemRemoved {
            player,
            item: configuration.material,
            amount: configuration.material_cost,
        });
        self.publish(crate::events::GameplayEvent::ShipRepairApplied {
            ship,
            amount,
            current,
            state,
        });
        self.publish(crate::events::GameplayEvent::ShipRepaired {
            ship,
            player,
            station,
            amount,
            current,
            material: configuration.material,
            material_consumed: configuration.material_cost,
        });
        Ok(amount)
    }
    pub(crate) fn environmental_hazard_mut(
        &mut self,
        hazard: HazardId,
    ) -> Option<&mut EnvironmentalHazard> {
        self.environmental_hazards.get_mut(&hazard)
    }
    pub(crate) fn apply_ship_damage(
        &mut self,
        ship: EntityId,
        amount: u32,
    ) -> Result<(u32, StructuralState, StructuralState), IntegrityError> {
        let outcome = {
            let Some(integrity) = self.ship_integrity.get_mut(&ship) else {
                return Err(IntegrityError::NotInitialized);
            };
            integrity.apply_damage(amount)?
        };
        let (remaining, before, after) = outcome;
        self.publish(crate::events::GameplayEvent::ShipDamageApplied {
            ship,
            amount,
            remaining,
            state: after,
        });
        if before != StructuralState::Critical && after == StructuralState::Critical {
            self.publish(crate::events::GameplayEvent::ShipEnteredCritical { ship, remaining });
        }
        if after == StructuralState::Destroyed {
            self.publish(crate::events::GameplayEvent::ShipDestroyed { ship });
            if let Some(destination) = self.navigation.cancel_travel() {
                self.publish(crate::events::GameplayEvent::TravelCancelledByDestruction {
                    ship,
                    destination,
                });
            }
        }
        Ok(outcome)
    }
    pub(crate) fn start_navigation_travel(
        &mut self,
        now: engine::Duration,
    ) -> Result<
        (DestinationId, engine::Duration, engine::Duration, u32, u32),
        crate::navigation::NavigationStartError,
    > {
        let Some(ship) = self.navigation.ship_id() else {
            return Err(crate::navigation::NavigationStartError::ShipNotFound);
        };
        let Some(resources) = self.ship_resources.get_mut(&ship) else {
            return Err(crate::navigation::NavigationStartError::ResourcesNotInitialized);
        };
        self.navigation.start_travel(now, resources.fuel_mut())
    }
    pub(crate) fn plan_autopilot(
        &mut self,
        origin: DestinationId,
        destination: DestinationId,
    ) -> Result<engine::Duration, crate::navigation::AutopilotError> {
        self.navigation.plan_autopilot(origin, destination)
    }
    pub(crate) fn start_autopilot(
        &mut self,
        now: engine::Duration,
        player: EntityId,
        pilot_station: EntityId,
    ) -> Result<
        (DestinationId, engine::Duration, engine::Duration, u32, u32),
        crate::navigation::AutopilotError,
    > {
        let Some(ship) = self.navigation.ship_id() else {
            return Err(crate::navigation::AutopilotError::DestinationNotFound);
        };
        let Some(resources) = self.ship_resources.get_mut(&ship) else {
            return Err(crate::navigation::AutopilotError::DestinationNotFound);
        };
        self.navigation
            .start_autopilot(now, resources.fuel_mut(), player, pilot_station)
    }
    pub(crate) fn advance_autopilot_leg(
        &mut self,
        now: engine::Duration,
    ) -> Result<
        Option<(EntityId, EntityId, DestinationId, engine::Duration, u32, u32)>,
        crate::navigation::AutopilotError,
    > {
        let Some(ship) = self.navigation.ship_id() else {
            return Err(crate::navigation::AutopilotError::DestinationNotFound);
        };
        let Some(resources) = self.ship_resources.get_mut(&ship) else {
            return Err(crate::navigation::AutopilotError::DestinationNotFound);
        };
        self.navigation
            .advance_autopilot_leg(now, resources.fuel_mut())
    }
    pub(crate) fn ship_resources_mut(&mut self, ship: EntityId) -> Option<&mut ShipResources> {
        self.ship_resources.get_mut(&ship)
    }
    pub(crate) fn set_interactable_enabled(&mut self, entity: EntityId, enabled: bool) -> bool {
        let Some(interactable) = self.interactables.get_mut(&entity) else {
            return false;
        };
        interactable.enabled = enabled;
        true
    }
    pub(crate) fn mission_mut(&mut self, mission: crate::types::MissionId) -> Option<&mut Mission> {
        self.missions.get_mut(&mission)
    }
}

fn finite_vec3(value: engine::Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn route_fuel_requirement(
    world: &GameplayWorld,
    route: &[crate::navigation::DestinationId],
    start: usize,
) -> Option<u32> {
    if route.len() < 2 || start >= route.len().saturating_sub(1) {
        return Some(0);
    }
    let mut total = 0_u32;
    for index in start..route.len() - 1 {
        let leg = world.navigation_route(route[index], route[index + 1])?;
        total = total.checked_add(leg.fuel_cost)?;
    }
    Some(total)
}

fn within_radius(a: engine::Vec3, b: engine::Vec3, radius: f32) -> bool {
    crate::encounter::distance_squared(a, b)
        .is_some_and(|distance| distance <= f64::from(radius) * f64::from(radius))
}

fn classify_hazard_on_route(
    world: &GameplayWorld,
    route: &[DestinationId],
    hazard: &EnvironmentalHazard,
) -> NavigationRouteRisk {
    let source = NavigationRouteRiskSource::Hazard(hazard.id);
    if !hazard.active {
        return NavigationRouteRisk {
            source,
            classification: NavigationRouteRiskClassification::Resolved,
        };
    }
    classify_position_on_route(world, route, hazard.position, hazard.radius, source)
}

fn classify_encounter_on_route(
    world: &GameplayWorld,
    route: &[DestinationId],
    encounter: &SpatialEncounter,
) -> NavigationRouteRisk {
    let source = NavigationRouteRiskSource::Encounter(encounter.id);
    if encounter.state == crate::encounter::SpatialEncounterState::Resolved {
        return NavigationRouteRisk {
            source,
            classification: NavigationRouteRiskClassification::Resolved,
        };
    }
    classify_position_on_route(
        world,
        route,
        encounter.position,
        encounter.activation_radius,
        source,
    )
}

fn classify_position_on_route(
    world: &GameplayWorld,
    route: &[DestinationId],
    position: engine::Vec3,
    margin: f32,
    source: NavigationRouteRiskSource,
) -> NavigationRouteRisk {
    if route.len() < 2 || !finite_vec3(position) || !margin.is_finite() || margin < 0.0 {
        return NavigationRouteRisk {
            source,
            classification: NavigationRouteRiskClassification::NotEvaluable,
        };
    }
    let mut nearest: Option<(usize, f64)> = None;
    for segment in 0..route.len() - 1 {
        let Some(from) = world.destination(route[segment]).map(|value| value.position) else {
            return NavigationRouteRisk {
                source,
                classification: NavigationRouteRiskClassification::NotEvaluable,
            };
        };
        let Some(to) = world.destination(route[segment + 1]).map(|value| value.position) else {
            return NavigationRouteRisk {
                source,
                classification: NavigationRouteRiskClassification::NotEvaluable,
            };
        };
        let Some(distance_squared) = point_segment_distance_squared(position, from, to) else {
            return NavigationRouteRisk {
                source,
                classification: NavigationRouteRiskClassification::NotEvaluable,
            };
        };
        if nearest.is_none_or(|(_, current)| distance_squared < current) {
            nearest = Some((segment, distance_squared));
        }
    }
    let Some((segment, distance_squared)) = nearest else {
        return NavigationRouteRisk {
            source,
            classification: NavigationRouteRiskClassification::NotEvaluable,
        };
    };
    let distance = distance_squared.sqrt();
    if !distance.is_finite() {
        return NavigationRouteRisk {
            source,
            classification: NavigationRouteRiskClassification::NotEvaluable,
        };
    }
    let margin = f64::from(margin);
    let classification = if distance_squared <= margin * margin {
        NavigationRouteRiskClassification::NearSegment { segment, distance }
    } else {
        NavigationRouteRiskClassification::OutsideMargin { segment, distance }
    };
    NavigationRouteRisk { source, classification }
}

fn point_segment_distance_squared(
    point: engine::Vec3,
    from: engine::Vec3,
    to: engine::Vec3,
) -> Option<f64> {
    if !finite_vec3(point) || !finite_vec3(from) || !finite_vec3(to) {
        return None;
    }
    let px = f64::from(point.x) - f64::from(from.x);
    let py = f64::from(point.y) - f64::from(from.y);
    let pz = f64::from(point.z) - f64::from(from.z);
    let dx = f64::from(to.x) - f64::from(from.x);
    let dy = f64::from(to.y) - f64::from(from.y);
    let dz = f64::from(to.z) - f64::from(from.z);
    let length_squared = dx * dx + dy * dy + dz * dz;
    if !length_squared.is_finite() || length_squared <= f64::EPSILON {
        return None;
    }
    let projection = (px * dx + py * dy + pz * dz) / length_squared;
    if !projection.is_finite() {
        return None;
    }
    let clamped = projection.clamp(0.0, 1.0);
    let offset_x = px - clamped * dx;
    let offset_y = py - clamped * dy;
    let offset_z = pz - clamped * dz;
    let distance_squared = offset_x * offset_x + offset_y * offset_y + offset_z * offset_z;
    distance_squared.is_finite().then_some(distance_squared)
}

fn navigation_route_risk_sort_key(
    risk: &NavigationRouteRisk,
) -> (u8, u64, u8, usize) {
    let (source_kind, source_id) = match risk.source {
        NavigationRouteRiskSource::Hazard(id) => (0_u8, u64::from(id.value())),
        NavigationRouteRiskSource::Encounter(id) => (1_u8, u64::from(id.value())),
    };
    let classification_kind = match risk.classification {
        NavigationRouteRiskClassification::NearSegment { segment, .. } => (0_u8, segment),
        NavigationRouteRiskClassification::OutsideMargin { segment, .. } => (1_u8, segment),
        NavigationRouteRiskClassification::NotEvaluable => (2_u8, usize::MAX),
        NavigationRouteRiskClassification::Resolved => (3_u8, usize::MAX),
    };
    (source_kind, source_id, classification_kind.0, classification_kind.1)
}

fn summarize_route(
    world: &GameplayWorld,
    route: &[DestinationId],
    fuel_available: Option<u32>,
    ship_operational: bool,
) -> Option<NavigationRouteOption> {
    if route.len() < 2 {
        return None;
    }
    let mut distance = 0.0_f64;
    let mut duration = 0_u64;
    let mut fuel_required = 0_u32;
    for pair in route.windows(2) {
        let leg = world.navigation_route(pair[0], pair[1])?;
        if leg.state != crate::navigation::NavigationRouteState::Available
            || !leg.distance.is_finite()
            || leg.distance <= 0.0
        {
            return None;
        }
        distance += f64::from(leg.distance);
        if !distance.is_finite() {
            return None;
        }
        duration = duration.checked_add(leg.duration.0)?;
        fuel_required = fuel_required.checked_add(leg.fuel_cost)?;
    }
    let mut risks = Vec::new();
    for hazard in world.environmental_hazards() {
        risks.push(classify_hazard_on_route(world, route, hazard));
    }
    for encounter in world.spatial_encounters() {
        risks.push(classify_encounter_on_route(world, route, encounter));
    }
    risks.sort_by_key(navigation_route_risk_sort_key);
    let mut critical_risks = 0;
    let mut active_risks = 0;
    let mut unevaluable_risks = 0;
    for risk in &risks {
        match risk.classification {
            NavigationRouteRiskClassification::NearSegment { .. } => {
                active_risks += 1;
                if route_risk_is_critical(world, risk.source) {
                    critical_risks += 1;
                }
            }
            NavigationRouteRiskClassification::NotEvaluable => unevaluable_risks += 1,
            NavigationRouteRiskClassification::OutsideMargin { .. }
            | NavigationRouteRiskClassification::Resolved => {}
        }
    }
    Some(NavigationRouteOption {
        route: route.to_vec(),
        distance,
        eta: Duration(duration),
        fuel_required,
        risks,
        critical_risks,
        active_risks,
        unevaluable_risks,
        viable: ship_operational && fuel_available.is_some_and(|available| available >= fuel_required),
    })
}

fn enumerate_navigation_paths(
    world: &GameplayWorld,
    destination: DestinationId,
    path: &mut Vec<DestinationId>,
    visited: &mut BTreeSet<DestinationId>,
    maximum_depth: usize,
    output: &mut Vec<Vec<DestinationId>>,
) {
    let Some(&current) = path.last() else {
        return;
    };
    if current == destination {
        output.push(path.clone());
        return;
    }
    if path.len() >= maximum_depth {
        return;
    }
    let next_destinations = world
        .navigation_routes()
        .filter(|route| route.from == current && route.state == crate::navigation::NavigationRouteState::Available)
        .map(|route| route.to)
        .collect::<Vec<_>>();
    for next in next_destinations {
        if !visited.insert(next) {
            continue;
        }
        path.push(next);
        enumerate_navigation_paths(world, destination, path, visited, maximum_depth, output);
        path.pop();
        visited.remove(&next);
    }
}

fn route_risk_is_critical(world: &GameplayWorld, source: NavigationRouteRiskSource) -> bool {
    match source {
        NavigationRouteRiskSource::Hazard(_) => true,
        NavigationRouteRiskSource::Encounter(id) => world
            .spatial_encounter(id)
            .is_some_and(|encounter| matches!(encounter.kind, SpatialEncounterKind::Hostile { .. })),
    }
}

fn compare_route_options(left: &NavigationRouteOption, right: &NavigationRouteOption) -> std::cmp::Ordering {
    left.critical_risks
        .cmp(&right.critical_risks)
        .then_with(|| left.active_risks.cmp(&right.active_risks))
        .then_with(|| left.unevaluable_risks.cmp(&right.unevaluable_risks))
        .then_with(|| left.eta.0.cmp(&right.eta.0))
        .then_with(|| left.fuel_required.cmp(&right.fuel_required))
        .then_with(|| left.distance.total_cmp(&right.distance))
        .then_with(|| left.route.cmp(&right.route))
}

fn route_is_better(candidate: &NavigationRouteOption, current: &NavigationRouteOption) -> bool {
    compare_route_options(candidate, current) == std::cmp::Ordering::Less
}

fn build_alternative_recommendation(
    current: &NavigationRouteOption,
    alternative: &NavigationRouteOption,
) -> Option<NavigationAlternativeRecommendation> {
    let mut current_sources = current.risks.iter().filter_map(|risk| {
        if matches!(risk.classification, NavigationRouteRiskClassification::NearSegment { .. }) {
            Some(risk.source)
        } else {
            None
        }
    });
    let alternative_sources = alternative.risks.iter().filter_map(|risk| {
        if matches!(risk.classification, NavigationRouteRiskClassification::NearSegment { .. }) {
            Some(risk.source)
        } else {
            None
        }
    });
    let alternative_sources = alternative_sources.collect::<BTreeSet<_>>();
    let avoided_risk = current_sources
        .find(|source| !alternative_sources.contains(source));
    let reason = if let Some(source) = avoided_risk {
        NavigationRecommendationReason::AvoidsKnownRisk(source)
    } else if alternative.critical_risks < current.critical_risks
        || alternative.active_risks < current.active_risks
        || alternative.unevaluable_risks < current.unevaluable_risks
    {
        NavigationRecommendationReason::FewerKnownRisks
    } else if alternative.eta < current.eta {
        NavigationRecommendationReason::LowerEta
    } else if alternative.fuel_required < current.fuel_required {
        NavigationRecommendationReason::LowerFuel
    } else {
        NavigationRecommendationReason::ShorterDistance
    };
    Some(NavigationAlternativeRecommendation {
        current: current.route.clone(),
        alternative: alternative.route.clone(),
        reason,
        avoided_risk,
        current_eta: current.eta,
        alternative_eta: alternative.eta,
        current_fuel: current.fuel_required,
        alternative_fuel: alternative.fuel_required,
        current_distance: current.distance,
        alternative_distance: alternative.distance,
    })
}

fn tactical_priority_rank(priority: crate::companion::TacticalPriority) -> u8 {
    match priority {
        crate::companion::TacticalPriority::Critical => 0,
        crate::companion::TacticalPriority::High => 1,
        crate::companion::TacticalPriority::Normal => 2,
    }
}

fn validate_interactable_geometry(
    position: engine::Vec3,
    interaction_range: f32,
) -> Result<(), InteractionRegistrationError> {
    if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
        return Err(InteractionRegistrationError::InvalidPosition);
    }
    if !interaction_range.is_finite() || interaction_range <= 0.0 {
        return Err(InteractionRegistrationError::InvalidRange);
    }
    Ok(())
}
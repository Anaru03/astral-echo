use engine::{EntityId, EntityRegistry, GameTime};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    collectible::{Collectible, CollectibleError, ItemDefinition, ItemDefinitionError},
    combat::{Combatant, EnemyConfiguration},
    components::{Inventory, InventoryError},
    door::{Door, DoorAccessRequirement},
    derelict::{Derelict, DerelictError, DerelictId},
    electrical::{ElectricalPanel, ElectricalPanelConfiguration, ElectricalPanelRegistrationError},
    encounter::{EncounterId, SpatialEncounter, SpatialEncounterError, SpatialEncounterKind},
    events::EventLog,
    hazard::{EnvironmentalHazard, EnvironmentalHazardError, HazardId},
    healing::{HealingError, HealingResult},
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
use engine::{EntityId, EntityRegistry, EntityRegistrySnapshot, GameTime, Tick, Duration, Vec3};
use std::fmt;

use crate::{
    companion::CompanionBehavior,
    collectible::{ItemCategory, ItemDefinition},
    combat::{AttackType, EnemyConfiguration, Shield, WeaponAmmunition, WeaponConfiguration},
    derelict::{Derelict, DerelictReward, DerelictState},
    encounter::{EncounterId, SpatialEncounter, SpatialEncounterKind, SpatialEncounterState},
    hazard::{EnvironmentalHazard, EnvironmentalHazardType, HazardId},
    domains::{mission::MissionState, objective::ObjectiveState},
    knowledge::{KnowledgeState, KnowledgeTarget},
    navigation::{Destination, DestinationId, DestinationType, DiscoveryState, NavigationRoute, NavigationRouteState},
    types::{ItemId, MissionId, ObjectiveId, SimulationPhase},
    revival::ReviveEffect,
    reward::{EnemyReward, RewardDistributionPolicy, RewardEntry},
    resources::ShipUpgrade,
    world_activity::{CombatZone, FunctionalWorld, ResourceSite, StorySite, TradeMarket, TradeOffer, WorldActivity},
    world::GameplayWorld,
};

pub const GAMEPLAY_SAVE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSaveState {
    pub id: EntityId,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompanionSaveState {
    pub entity: EntityId,
    pub followed_player: EntityId,
    pub position: Vec3,
    pub behavior: CompanionBehavior,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventorySaveState {
    pub owner: EntityId,
    pub capacity: u32,
    pub entries: Vec<(ItemId, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CombatantSaveState {
    pub id: EntityId,
    pub is_player: bool,
    pub active: bool,
    pub maximum_health: u32,
    pub current_health: u32,
    pub position: Vec3,
    pub archetype: Option<crate::combat::EnemyArchetype>,
    pub weapon: WeaponConfiguration,
    pub equipped_weapon: Option<WeaponConfiguration>,
    pub shield: Option<Shield>,
    pub mobility: u32,
    pub last_attack_at: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MissionSaveState {
    pub id: MissionId,
    pub state: MissionState,
    pub objectives: Vec<(ObjectiveId, ObjectiveState, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollectibleSaveState {
    pub entity: EntityId,
    pub item: ItemId,
    pub amount: u32,
    pub position: Vec3,
    pub interaction_range: f32,
    pub owner: Option<EntityId>,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct GameplaySaveState {
    pub schema_version: u32,
    pub phase: SimulationPhase,
    pub time: GameTime,
    pub registry: EntityRegistrySnapshot,
    pub players: Vec<PlayerSaveState>,
    pub companions: Vec<CompanionSaveState>,
    pub inventories: Vec<InventorySaveState>,
    pub combatants: Vec<CombatantSaveState>,
    pub currencies: Vec<(EntityId, u64)>,
    pub missions: Vec<MissionSaveState>,
    pub knowledge: Vec<(KnowledgeTarget, KnowledgeState)>,
    pub collectibles: Vec<CollectibleSaveState>,
    pub ship_fuel: Vec<(EntityId, u32, u32)>,
    pub ship_integrity: Vec<(EntityId, u32, u32, u32, u32)>,
    pub ship_upgrades: Vec<(EntityId, ShipUpgrade)>,
    pub ship_position: Option<Vec3>,
    pub destinations: Vec<Destination>,
    pub routes: Vec<NavigationRoute>,
    pub item_definitions: Vec<ItemDefinition>,
    pub enemy_rewards: Vec<crate::reward::EnemyReward>,
    pub processed_enemy_rewards: Vec<EntityId>,
    pub hazards: Vec<EnvironmentalHazard>,
    pub encounters: Vec<SpatialEncounter>,
    pub derelicts: Vec<Derelict>,
    pub functional_worlds: Vec<FunctionalWorld>,
    pub(crate) world_snapshot: Option<Box<GameplayWorld>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveError {
    InvalidHeader,
    UnsupportedSchema,
    MalformedRecord,
    InvalidNumber,
    InvalidFloat,
    InvalidRegistry,
    DuplicateRecord,
    InvalidReference,
    InvalidState,
}

impl fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SaveError {}

impl GameplaySaveState {
    pub fn serialize(&self) -> String {
        let mut output = format!("ASTRAL_ECHO_SAVE|{}\n", self.schema_version);
        output.push_str(&format!("phase|{}\n", phase(self.phase)));
        output.push_str(&format!("time|{}|{}\n", self.time.tick.0, self.time.elapsed.0));
        for destination in &self.destinations {
            output.push_str(&format!(
                "destination|{}|{}|{}|{}|{}|{}|{}|{}\n",
                destination.id.value(), destination_type(destination.destination_type),
                bits(destination.position.x), bits(destination.position.y), bits(destination.position.z),
                destination.travel_duration.0, destination.fuel_cost, discovery(destination.discovery)
            ));
        }
        for route in &self.routes {
            output.push_str(&format!(
                "route|{}|{}|{}|{}|{}|{}\n",
                route.from.value(), route.to.value(), bits(route.distance), route.duration.0,
                route.fuel_cost, route_state(route.state)
            ));
        }
        output.push_str(&format!(
            "registry|{}|{}|{}\n",
            self.registry.next_id,
            ids(&self.registry.known),
            ids(&self.registry.active)
        ));
        for player in &self.players {
            output.push_str(&format!(
                "player|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                player.id.value(), bits(player.position.x), bits(player.position.y), bits(player.position.z),
                bits(player.yaw), bits(player.pitch), bits(player.velocity.x), bits(player.velocity.y),
                bits(player.velocity.z), bool_value(player.active)
            ));
        }
        for companion in &self.companions {
            let (behavior, target) = companion_behavior(companion.behavior);
            output.push_str(&format!(
                "companion|{}|{}|{}|{}|{}|{}|{}|{}\n",
                companion.entity.value(), companion.followed_player.value(),
                bits(companion.position.x), bits(companion.position.y), bits(companion.position.z),
                behavior, target, bool_value(companion.active)
            ));
        }
        for inventory in &self.inventories {
            output.push_str(&format!(
                "inventory|{}|{}|{}\n",
                inventory.owner.value(), inventory.capacity, inventory_entries(&inventory.entries)
            ));
        }
        for combatant in &self.combatants {
            output.push_str(&format!(
                "combatant|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                combatant.id.value(), bool_value(combatant.is_player), bool_value(combatant.active),
                combatant.maximum_health, combatant.current_health, bits(combatant.position.x),
                bits(combatant.position.y), bits(combatant.position.z), archetype(combatant.archetype),
                weapon(combatant.weapon), optional_weapon(combatant.equipped_weapon),
                optional_shield(combatant.shield), combatant.mobility,
                combatant.last_attack_at.map_or(u64::MAX, |value| value.0)
            ));
        }
        for definition in &self.item_definitions {
            output.push_str(&format!("item_definition|{}|{}|{}|{}|{}|{}|{}|{}\n",
                definition.id.0, item_category(definition.category), definition.max_stack,
                definition.healing.map_or(0, |effect| effect.health_per_unit),
                definition.revival.map_or(0, |effect| effect.item.0),
                definition.revival.map_or(0, |effect| effect.units_per_revival),
                definition.revival.map_or(0, |effect| effect.health_restored),
                definition.revival.map_or(0, |effect| effect.range)));
        }
        for reward in &self.enemy_rewards {
            output.push_str(&format!("enemy_reward|{}|{}|{}\n", archetype(Some(reward.archetype)),
                distribution(reward.distribution), reward_entries(&reward.entries)));
        }
        for enemy in &self.processed_enemy_rewards {
            output.push_str(&format!("processed_reward|{}\n", enemy.value()));
        }
        for hazard in &self.hazards {
            output.push_str(&format!("hazard|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", hazard.id.value(), hazard_type(hazard.hazard_type), bits(hazard.position.x), bits(hazard.position.y), bits(hazard.position.z), bits(hazard.radius), hazard.damage, hazard.interval.0, bool_value(hazard.active), hazard.accumulated().0));
        }
        for encounter in &self.encounters {
            output.push_str(&format!("encounter|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", encounter.id.value(), encounter_kind(encounter.kind), bits(encounter.position.x), bits(encounter.position.y), bits(encounter.position.z), bits(encounter.activation_radius), encounter_state(encounter.state), encounter_payload(encounter.kind), encounter.spawned_enemy.map_or(u64::MAX, EntityId::value)));
        }
        for derelict in &self.derelicts {
            output.push_str(&format!("derelict|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", derelict.id.value(), bits(derelict.position.x), bits(derelict.position.y), bits(derelict.position.z), bits(derelict.interaction_range), derelict_state(derelict.state), derelict.associated_encounter.map_or(u32::MAX, |value| value.value()), derelict.enemy_configuration.map_or_else(|| "x".to_string(), enemy_configuration), distribution(derelict.distribution), derelict.spawned_enemy.map_or(u64::MAX, EntityId::value), bool_value(derelict.enemy_defeated), entity_ids(&derelict.reward_collectibles), bool_value(derelict.reward_generated)));
            output.push_str(&format!("derelict_rewards|{}|{}\n", derelict.id.value(), derelict_rewards(&derelict.rewards)));
        }
        for world in &self.functional_worlds {
            output.push_str(&format!("functional_world|{}|{}\n", world.destination.value(), bits(world.interaction_range)));
            for activity in &world.activities { output.push_str(&format!("world_activity|{}|{}\n", world.destination.value(), world_activity(activity))); }
        }
        for (entity, currency) in &self.currencies {
            output.push_str(&format!("currency|{}|{}\n", entity.value(), currency));
        }
        for mission in &self.missions {
            output.push_str(&format!("mission|{}|{}\n", mission.id.0, mission_state(mission.state)));
            for (objective, state, progress) in &mission.objectives {
                output.push_str(&format!(
                    "objective|{}|{}|{}|{}\n", objective.0, objective_state(*state), progress, mission.id.0
                ));
            }
        }
        for (target, state) in &self.knowledge {
            output.push_str(&format!("knowledge|{}|{}|{}\n", knowledge_target(*target), knowledge_state(*state), 0));
        }
        for collectible in &self.collectibles {
            output.push_str(&format!(
                "collectible|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                collectible.entity.value(), collectible.item.0, collectible.amount,
                bits(collectible.position.x), bits(collectible.position.y), bits(collectible.position.z),
                bits(collectible.interaction_range), collectible.owner.map_or(0, EntityId::value), bool_value(collectible.active)
            ));
        }
        for (ship, current, capacity) in &self.ship_fuel {
            output.push_str(&format!("fuel|{}|{}|{}\n", ship.value(), current, capacity));
        }
        for (ship, upgrade) in &self.ship_upgrades {
            output.push_str(&format!("ship_upgrade|{}|{}\n", ship.value(), upgrade.code()));
        }
        for (ship, current, maximum, threshold, _) in &self.ship_integrity {
            output.push_str(&format!("integrity|{}|{}|{}|{}\n", ship.value(), current, maximum, threshold));
        }
        if let Some(position) = self.ship_position {
            output.push_str(&format!("ship_position|{}|{}|{}\n", bits(position.x), bits(position.y), bits(position.z)));
        }
        output
    }

    pub fn encode(&self) -> String { self.serialize() }

    pub fn deserialize(input: &str) -> Result<Self, SaveError> {
        let mut lines = input.lines();
        let header = lines.next().ok_or(SaveError::InvalidHeader)?.split('|').collect::<Vec<_>>();
        if header.len() != 2 || header[0] != "ASTRAL_ECHO_SAVE" { return Err(SaveError::InvalidHeader); }
        let schema_version = parse_u32(header[1])?;
        if schema_version != GAMEPLAY_SAVE_SCHEMA_VERSION { return Err(SaveError::UnsupportedSchema); }
        let mut state = Self {
            schema_version, phase: SimulationPhase::Loading, time: GameTime::new(), registry: EntityRegistrySnapshot::default(),
            players: Vec::new(), companions: Vec::new(), inventories: Vec::new(), combatants: Vec::new(), currencies: Vec::new(),
            missions: Vec::new(), knowledge: Vec::new(), collectibles: Vec::new(), ship_fuel: Vec::new(),
            ship_integrity: Vec::new(), ship_upgrades: Vec::new(), ship_position: None,
            destinations: Vec::new(), routes: Vec::new(), item_definitions: Vec::new(),
            enemy_rewards: Vec::new(), processed_enemy_rewards: Vec::new(), hazards: Vec::new(), encounters: Vec::new(), derelicts: Vec::new(), functional_worlds: Vec::new(),
            world_snapshot: None,
        };
        for line in lines {
            let fields = line.split('|').collect::<Vec<_>>();
            if fields.is_empty() { continue; }
            match fields[0] {
                "phase" if fields.len() == 2 => state.phase = parse_phase(fields[1])?,
                "time" if fields.len() == 3 => state.time = GameTime { tick: Tick(parse_u64(fields[1])?), elapsed: Duration(parse_u64(fields[2])?) },
                "destination" if fields.len() == 9 => state.destinations.push(Destination { id: DestinationId::new(parse_u32(fields[1])?), destination_type: parse_destination_type(fields[2])?, position: vec3(&fields, 3)?, travel_duration: Duration(parse_u64(fields[6])?), fuel_cost: parse_u32(fields[7])?, discovery: parse_discovery(fields[8])? }),
                "route" if fields.len() == 7 => state.routes.push(NavigationRoute { from: DestinationId::new(parse_u32(fields[1])?), to: DestinationId::new(parse_u32(fields[2])?), distance: float(fields[3])?, duration: Duration(parse_u64(fields[4])?), fuel_cost: parse_u32(fields[5])?, state: parse_route_state(fields[6])? }),
                "registry" if fields.len() == 4 => state.registry = EntityRegistrySnapshot { next_id: parse_u64(fields[1])?, known: parse_ids(fields[2])?, active: parse_ids(fields[3])? },
                "player" if fields.len() == 11 => state.players.push(PlayerSaveState { id: EntityId::new(parse_u64(fields[1])?), position: vec3(&fields, 2)?, yaw: float(fields[5])?, pitch: float(fields[6])?, velocity: vec3(&fields, 7)?, active: parse_bool(fields[10])? }),
                "companion" if fields.len() == 9 => state.companions.push(CompanionSaveState { entity: EntityId::new(parse_u64(fields[1])?), followed_player: EntityId::new(parse_u64(fields[2])?), position: vec3(&fields, 3)?, behavior: parse_companion_behavior(fields[6], fields[7])?, active: parse_bool(fields[8])? }),
                "inventory" if fields.len() == 4 => state.inventories.push(InventorySaveState { owner: EntityId::new(parse_u64(fields[1])?), capacity: parse_u32(fields[2])?, entries: parse_inventory(fields[3])? }),
                "combatant" if fields.len() == 15 => state.combatants.push(parse_combatant(&fields)?),
                "item_definition" if fields.len() == 9 => state.item_definitions.push(parse_item_definition(&fields)?),
                "enemy_reward" if fields.len() == 4 => state.enemy_rewards.push(parse_enemy_reward(&fields)?),
                "processed_reward" if fields.len() == 2 => state.processed_enemy_rewards.push(EntityId::new(parse_u64(fields[1])?)),
                "hazard" if fields.len() == 11 => state.hazards.push(parse_hazard(&fields)?),
                "encounter" if fields.len() == 10 => state.encounters.push(parse_encounter(&fields)?),
                "derelict" if fields.len() == 14 => state.derelicts.push(parse_derelict(&fields)?),
                "derelict_rewards" if fields.len() == 3 => {
                    let id = parse_u32(fields[1])?;
                    let rewards = parse_derelict_rewards(fields[2])?;
                    state.derelicts.iter_mut().find(|value| value.id.value() == id).ok_or(SaveError::InvalidReference)?.rewards = rewards;
                }
                "functional_world" if fields.len() == 3 => state.functional_worlds.push(FunctionalWorld::new(crate::navigation::DestinationId::new(parse_u32(fields[1])?), float(fields[2])?, Vec::new())),
                "world_activity" if fields.len() == 3 => {
                    let destination = crate::navigation::DestinationId::new(parse_u32(fields[1])?);
                    let activity = parse_world_activity(fields[2])?;
                    state.functional_worlds.iter_mut().find(|value| value.destination == destination).ok_or(SaveError::InvalidReference)?.activities.push(activity);
                }
                "currency" if fields.len() == 3 => state.currencies.push((EntityId::new(parse_u64(fields[1])?), parse_u64(fields[2])?)),
                "mission" if fields.len() == 3 => state.missions.push(MissionSaveState { id: MissionId(parse_u32(fields[1])?), state: parse_mission_state(fields[2])?, objectives: Vec::new() }),
                "objective" if fields.len() == 5 => {
                    let mission = MissionId(parse_u32(fields[4])?);
                    let objective = (ObjectiveId(parse_u32(fields[1])?), parse_objective_state(fields[2])?, parse_u32(fields[3])?);
                    if let Some(saved) = state.missions.iter_mut().find(|value| value.id == mission) { saved.objectives.push(objective); } else { return Err(SaveError::InvalidReference); }
                }
                "knowledge" if fields.len() == 4 => state.knowledge.push((parse_knowledge_target(fields[1])?, parse_knowledge_state(fields[2])?)),
                "collectible" if fields.len() == 10 => state.collectibles.push(CollectibleSaveState { entity: EntityId::new(parse_u64(fields[1])?), item: ItemId(parse_u32(fields[2])?), amount: parse_u32(fields[3])?, position: vec3(&fields, 4)?, interaction_range: float(fields[7])?, owner: match parse_u64(fields[8])? { 0 => None, value => Some(EntityId::new(value)) }, active: parse_bool(fields[9])? }),
                "fuel" if fields.len() == 4 => state.ship_fuel.push((EntityId::new(parse_u64(fields[1])?), parse_u32(fields[2])?, parse_u32(fields[3])?)),
                "ship_upgrade" if fields.len() == 3 => {
                    let code = parse_u32(fields[2])?;
                    let code = u8::try_from(code).map_err(|_| SaveError::InvalidState)?;
                    state.ship_upgrades.push((EntityId::new(parse_u64(fields[1])?), ShipUpgrade::from_code(code).ok_or(SaveError::InvalidState)?));
                }
                "integrity" if fields.len() == 5 => state.ship_integrity.push((EntityId::new(parse_u64(fields[1])?), parse_u32(fields[2])?, parse_u32(fields[3])?, parse_u32(fields[4])?, parse_u32(fields[2])?)),
                "ship_position" if fields.len() == 4 => state.ship_position = Some(vec3(&fields, 1)?),
                _ => return Err(SaveError::MalformedRecord),
            }
        }
        EntityRegistry::from_snapshot(state.registry.clone()).map_err(|_| SaveError::InvalidRegistry)?;
        validate_unique_entities(&state)?;
        Ok(state)
    }

    pub fn decode(input: &str) -> Result<Self, SaveError> { Self::deserialize(input) }
}

fn validate_unique_entities(state: &GameplaySaveState) -> Result<(), SaveError> {
    let mut player_ids = Vec::new();
    for id in state.players.iter().map(|v| v.id) {
        if player_ids.contains(&id) { return Err(SaveError::DuplicateRecord); }
        player_ids.push(id);
    }
    let mut companion_ids = Vec::new();
    for id in state.companions.iter().map(|v| v.entity) {
        if companion_ids.contains(&id) || player_ids.contains(&id) {
            return Err(SaveError::DuplicateRecord);
        }
        companion_ids.push(id);
    }
    let mut combatant_ids = Vec::new();
    for id in state.combatants.iter().map(|v| v.id) {
        if combatant_ids.contains(&id) { return Err(SaveError::DuplicateRecord); }
        combatant_ids.push(id);
    }
    let mut collectible_ids = Vec::new();
    for id in state.collectibles.iter().map(|v| v.entity) {
        if collectible_ids.contains(&id) { return Err(SaveError::DuplicateRecord); }
        collectible_ids.push(id);
    }
    let mut inventory_ids = Vec::new();
    for id in state.inventories.iter().map(|v| v.owner) {
        if inventory_ids.contains(&id) { return Err(SaveError::DuplicateRecord); }
        inventory_ids.push(id);
    }
    let mut mission_ids = Vec::new();
    for mission in &state.missions {
        if mission_ids.contains(&mission.id) { return Err(SaveError::DuplicateRecord); }
        mission_ids.push(mission.id);
        let mut objective_ids = Vec::new();
        for (objective, _, _) in &mission.objectives {
            if !objective_ids.contains(objective) { objective_ids.push(*objective); } else { return Err(SaveError::DuplicateRecord); }
        }
    }
    let mut processed_rewards = Vec::new();
    for enemy in &state.processed_enemy_rewards {
        if processed_rewards.contains(enemy) { 
            return Err(SaveError::DuplicateRecord); 
        }
        
        processed_rewards.push(*enemy);
    }
    let mut upgrades = Vec::new();
    for (ship, upgrade) in &state.ship_upgrades {
        if upgrades.contains(&(*ship, *upgrade)) { 
            return Err(SaveError::DuplicateRecord); 
        }
        
        upgrades.push((*ship, *upgrade));
    }

    if state.players.iter().any(|v| !state.registry.known.contains(&v.id)) { 
        return Err(SaveError::InvalidReference); 
    }
    
    Ok(())
}

fn bits(value: f32) -> u32 { 
    value.to_bits() 
}

fn float(value: &str) -> Result<f32, SaveError> { 
    Ok(f32::from_bits(parse_u32(value)?)) 
}

fn vec3(fields: &[&str], index: usize) -> Result<Vec3, SaveError> { 
    Ok(Vec3 { x: float(fields[index])?, y: float(fields[index + 1])?, z: float(fields[index + 2])? }) 
}

fn parse_u32(value: &str) -> Result<u32, SaveError> { 
    value.parse().map_err(|_| SaveError::InvalidNumber) 
}

fn parse_u64(value: &str) -> Result<u64, SaveError> { 
    value.parse().map_err(|_| SaveError::InvalidNumber) 
}

fn bool_value(value: bool) -> u8 { 
    u8::from(value) 
}

fn parse_bool(value: &str) -> Result<bool, SaveError> { 
    match value { 
        "0" => Ok(false), "1" => Ok(true), _ => Err(SaveError::MalformedRecord) 
    } 
}

fn ids(values: &[EntityId]) -> String { 
    values.iter().map(|v| v.value().to_string()).collect::<Vec<_>>().join(",") 
}

fn parse_ids(value: &str) -> Result<Vec<EntityId>, SaveError> { 
    if value.is_empty() { 
        return Ok(Vec::new()); 
    } 
    
    value.split(',').map(|v| Ok(EntityId::new(parse_u64(v)?))).collect() 
}

fn inventory_entries(values: &[(ItemId, u32)]) -> String { 
    values.iter().map(|(id, amount)| format!("{}:{amount}", id.0)).collect::<Vec<_>>().join(",") 
}

fn parse_inventory(value: &str) -> Result<Vec<(ItemId, u32)>, SaveError> { 
    if value.is_empty() { 
        return Ok(Vec::new()); 
    } 

    value.split(',').map(|v| { 
        let mut fields = v.split(':'); 
        Ok((ItemId(parse_u32(fields.next().ok_or(SaveError::MalformedRecord)?)?), parse_u32(fields.next().ok_or(SaveError::MalformedRecord)?)?)) 
    }).collect() 
}

fn mission_state(value: MissionState) -> u8 { 
    match value { 
        MissionState::Locked => 0, MissionState::Available => 1, MissionState::Active => 2, MissionState::Completed => 3, MissionState::Failed => 4 
    } 
}

fn phase(value: SimulationPhase) -> u8 {
    match value { 
        SimulationPhase::Loading => 0, SimulationPhase::Running => 1, SimulationPhase::Paused => 2, SimulationPhase::Complete => 3 
    } 
}

fn parse_phase(value: &str) -> Result<SimulationPhase, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(SimulationPhase::Loading), 1 => Ok(SimulationPhase::Running), 2 => Ok(SimulationPhase::Paused), 3 => Ok(SimulationPhase::Complete), _ => Err(SaveError::InvalidState) 
    } 
}

fn destination_type(value: DestinationType) -> u8 { 
    match value { DestinationType::Planet => 0, DestinationType::SpacePoint => 1 } 
}

fn parse_destination_type(value: &str) -> Result<DestinationType, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(DestinationType::Planet), 1 => Ok(DestinationType::SpacePoint), _ => Err(SaveError::InvalidState) 
    } 
}

fn discovery(value: DiscoveryState) -> u8 { 
    match value { 
        DiscoveryState::Unknown => 0, DiscoveryState::Known => 1, DiscoveryState::Discovered => 2 
    } 
}

fn parse_discovery(value: &str) -> Result<DiscoveryState, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(DiscoveryState::Unknown), 1 => Ok(DiscoveryState::Known), 2 => Ok(DiscoveryState::Discovered), _ => Err(SaveError::InvalidState) 
    } 
}

fn route_state(value: NavigationRouteState) -> u8 { 
    match value { 
        NavigationRouteState::Available => 0, NavigationRouteState::Blocked => 1 
    } 
}

fn parse_route_state(value: &str) -> Result<NavigationRouteState, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(NavigationRouteState::Available), 1 => Ok(NavigationRouteState::Blocked), _ => Err(SaveError::InvalidState) 
    } 
}

fn objective_state(value: ObjectiveState) -> u8 { 
    match value { 
        ObjectiveState::Locked => 0, ObjectiveState::Available => 1, ObjectiveState::Active => 2, ObjectiveState::Completed => 3, ObjectiveState::Failed => 4 
    } 
}

fn knowledge_state(value: KnowledgeState) -> u8 { 
    match value { 
        KnowledgeState::Unknown => 0, KnowledgeState::Detected => 1, KnowledgeState::Identified => 2, KnowledgeState::Discovered => 3, KnowledgeState::Studied => 4 
    } 
}

fn knowledge_target(value: KnowledgeTarget) -> String { 
    match value { 
        KnowledgeTarget::Destination(id) => format!("d{}", id.value()), KnowledgeTarget::Enemy(id) => format!("e{}", id.value()), KnowledgeTarget::Item(id) => format!("i{}", id.0) 
    } 
}

fn parse_mission_state(value: &str) -> Result<MissionState, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(MissionState::Locked), 1 => Ok(MissionState::Available), 2 => Ok(MissionState::Active), 3 => Ok(MissionState::Completed), 4 => Ok(MissionState::Failed), _ => Err(SaveError::InvalidState) 
    } 
}

fn parse_objective_state(value: &str) -> Result<ObjectiveState, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(ObjectiveState::Locked), 1 => Ok(ObjectiveState::Available), 2 => Ok(ObjectiveState::Active), 3 => Ok(ObjectiveState::Completed), 4 => Ok(ObjectiveState::Failed), _ => Err(SaveError::InvalidState) 
    } 
}

fn parse_knowledge_state(value: &str) -> Result<KnowledgeState, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(KnowledgeState::Unknown), 1 => Ok(KnowledgeState::Detected), 2 => Ok(KnowledgeState::Identified), 3 => Ok(KnowledgeState::Discovered), 4 => Ok(KnowledgeState::Studied), _ => Err(SaveError::InvalidState) 
    } 
}

fn companion_behavior(value: CompanionBehavior) -> (u32, String) {
    match value {
        CompanionBehavior::Accompany => (0, "x".to_string()),
        CompanionBehavior::Investigate(target) => (1, knowledge_target(target)),
        CompanionBehavior::CombatSupport => (2, "x".to_string()),
        CompanionBehavior::Waiting => (3, "x".to_string()),
        CompanionBehavior::Unavailable => (4, "x".to_string()),
    }
}

fn parse_companion_behavior(value: &str, target: &str) -> Result<CompanionBehavior, SaveError> {
    match parse_u32(value)? {
        0 if target == "x" => Ok(CompanionBehavior::Accompany),
        1 => Ok(CompanionBehavior::Investigate(parse_knowledge_target(target)?)),
        2 if target == "x" => Ok(CompanionBehavior::CombatSupport),
        3 if target == "x" => Ok(CompanionBehavior::Waiting),
        4 if target == "x" => Ok(CompanionBehavior::Unavailable),
        _ => Err(SaveError::InvalidState),
    }
}

fn parse_knowledge_target(value: &str) -> Result<KnowledgeTarget, SaveError> {
    if value.len() < 2 {
        return Err(SaveError::MalformedRecord);
    }
    let (kind, number) = value.split_at(1);
    let id = parse_u64(number)?;
    match kind {
        "d" => Ok(KnowledgeTarget::Destination(crate::navigation::DestinationId::new(
            u32::try_from(id).map_err(|_| SaveError::InvalidNumber)?,
        ))),
        "e" => Ok(KnowledgeTarget::Enemy(EntityId::new(id))),
        "i" => Ok(KnowledgeTarget::Item(ItemId(
            u32::try_from(id).map_err(|_| SaveError::InvalidNumber)?,
        ))),
        _ => Err(SaveError::MalformedRecord),
    }
}

fn attack_type(value: AttackType) -> u8 { 
    match value { 
        AttackType::Kinetic => 0, AttackType::Energy => 1, AttackType::Plasma => 2 
    } 
}

fn parse_attack_type(value: &str) -> Result<AttackType, SaveError> {
    match parse_u32(value)? {
        0 => Ok(AttackType::Kinetic),
        1 => Ok(AttackType::Energy),
        2 => Ok(AttackType::Plasma),
        _ => Err(SaveError::InvalidState),
    }
}

fn archetype(value: Option<crate::combat::EnemyArchetype>) -> u8 { 
    match value { 
        None => 255, Some(crate::combat::EnemyArchetype::Shielded) => 0, Some(crate::combat::EnemyArchetype::Shooter) => 1, Some(crate::combat::EnemyArchetype::Charger) => 2 
    } 
}

fn parse_archetype(value: &str) -> Result<Option<crate::combat::EnemyArchetype>, SaveError> {
    match parse_u32(value)? {
        255 => Ok(None),
        0 => Ok(Some(crate::combat::EnemyArchetype::Shielded)),
        1 => Ok(Some(crate::combat::EnemyArchetype::Shooter)),
        2 => Ok(Some(crate::combat::EnemyArchetype::Charger)),
        _ => Err(SaveError::InvalidState),
    }
}

fn weapon(value: WeaponConfiguration) -> String { 
    match value.ammunition { Some(ammunition) => format!("{},{},{},{},{},{}", attack_type(value.attack_type), value.damage, value.range, value.cooldown.0, ammunition.item.0, ammunition.amount_per_attack), None => format!("{},{},{},{}", attack_type(value.attack_type), value.damage, value.range, value.cooldown.0) 
    } 
}

fn parse_weapon(value: &str) -> Result<WeaponConfiguration, SaveError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 4 && fields.len() != 6 {
        return Err(SaveError::MalformedRecord);
    }
    let attack = parse_attack_type(fields[0])?;
    let damage = parse_u32(fields[1])?;
    let range = parse_u32(fields[2])?;
    let cooldown = Duration(parse_u64(fields[3])?);
    let value = if fields.len() == 6 {
        WeaponConfiguration::with_ammunition(
            attack,
            damage,
            range,
            cooldown,
            WeaponAmmunition {
                item: ItemId(parse_u32(fields[4])?),
                amount_per_attack: parse_u32(fields[5])?,
            },
        )
    } else {
        WeaponConfiguration::new(attack, damage, range, cooldown)
    };
    value.validate().map_err(|_| SaveError::InvalidState)?;
    Ok(value)
}

fn optional_weapon(value: Option<WeaponConfiguration>) -> String { 
    value.map_or_else(|| "x".to_string(), weapon) 
}

fn parse_optional_weapon(value: &str) -> Result<Option<WeaponConfiguration>, SaveError> { 
    if value == "x" { 
        Ok(None) } else { Ok(Some(parse_weapon(value)?)) 
    } 
}

fn shield(value: Shield) -> String { 
    format!("{},{},{}", value.current, value.maximum, attack_type(value.weakness)) 
}

fn optional_shield(value: Option<Shield>) -> String { 
    value.map_or_else(|| "x".to_string(), shield) 
}

fn parse_optional_shield(value: &str) -> Result<Option<Shield>, SaveError> {
    if value == "x" {
        return Ok(None);
    }
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(SaveError::MalformedRecord);
    }
    let value = Shield {
        current: parse_u32(fields[0])?,
        maximum: parse_u32(fields[1])?,
        weakness: parse_attack_type(fields[2])?,
    };
    if value.maximum == 0 || value.current > value.maximum {
        return Err(SaveError::InvalidState);
    }
    Ok(Some(value))
}

fn parse_combatant(fields: &[&str]) -> Result<CombatantSaveState, SaveError> { 
    let value = CombatantSaveState { id: EntityId::new(parse_u64(fields[1])?), is_player: parse_bool(fields[2])?, active: parse_bool(fields[3])?, maximum_health: parse_u32(fields[4])?, current_health: parse_u32(fields[5])?, position: vec3(fields, 6)?, archetype: parse_archetype(fields[9])?, weapon: parse_weapon(fields[10])?, equipped_weapon: parse_optional_weapon(fields[11])?, shield: parse_optional_shield(fields[12])?, mobility: parse_u32(fields[13])?, last_attack_at: match parse_u64(fields[14])? { u64::MAX => None, value => Some(Duration(value)) } }; 

    if value.maximum_health == 0 || value.current_health > value.maximum_health || value.mobility == 0 { 
        return Err(SaveError::InvalidState); 
    } Ok(value) 
}

fn item_category(value: ItemCategory) -> u8 { 
    match value { 
        ItemCategory::RepairMaterial => 0, ItemCategory::CombatAmmunition => 1, ItemCategory::MineralSample => 2, ItemCategory::ElectronicComponent => 3, ItemCategory::HealingConsumable => 4, ItemCategory::RevivalConsumable => 5 
    } 
}

fn parse_item_category(value: &str) -> Result<ItemCategory, SaveError> {
    match parse_u32(value)? {
        0 => Ok(ItemCategory::RepairMaterial),
        1 => Ok(ItemCategory::CombatAmmunition),
        2 => Ok(ItemCategory::MineralSample),
        3 => Ok(ItemCategory::ElectronicComponent),
        4 => Ok(ItemCategory::HealingConsumable),
        5 => Ok(ItemCategory::RevivalConsumable),
        _ => Err(SaveError::InvalidState),
    }
}

fn parse_item_definition(fields: &[&str]) -> Result<ItemDefinition, SaveError> { 
    let category = parse_item_category(fields[2])?; 
    let healing = parse_u32(fields[4])?; 
    let revival_item = parse_u32(fields[5])?; 
    let revival = if revival_item > 0 { 
        Some(ReviveEffect::new(ItemId(revival_item), parse_u32(fields[6])?, parse_u32(fields[7])?, parse_u32(fields[8])?)) 
    } else { 
        None 
    }; 
    let definition = ItemDefinition { id: ItemId(parse_u32(fields[1])?), category, max_stack: parse_u32(fields[3])?, healing: (healing > 0).then(|| crate::healing::HealingEffect::new(healing)), revival }; 
    definition.validate().map_err(|_| SaveError::InvalidState)?; 
    Ok(definition) 
}

fn distribution(value: RewardDistributionPolicy) -> u8 { 
    match value { 
        RewardDistributionPolicy::Shared => 0, RewardDistributionPolicy::Personal => 1 
    } 
}

fn parse_distribution(value: &str) -> Result<RewardDistributionPolicy, SaveError> { 
    match parse_u32(value)? { 
        0 => Ok(RewardDistributionPolicy::Shared), 1 => Ok(RewardDistributionPolicy::Personal), _ => Err(SaveError::InvalidState) 
    } 
}

fn reward_entries(values: &[RewardEntry]) -> String { 
    values.iter().map(|entry| format!("{}:{}", entry.item.0, entry.amount)).collect::<Vec<_>>().join(",") 
}

fn parse_reward_entries(value: &str) -> Result<Vec<RewardEntry>, SaveError> {
    if value.is_empty() {
        return Err(SaveError::InvalidState);
    }
    value
        .split(',')
        .map(|entry| {
            let fields = entry.split(':').collect::<Vec<_>>();
            if fields.len() != 2 {
                return Err(SaveError::MalformedRecord);
            }
            Ok(RewardEntry {
                item: ItemId(parse_u32(fields[0])?),
                amount: parse_u32(fields[1])?,
            })
        })
        .collect()
}

fn parse_enemy_reward(fields: &[&str]) -> Result<EnemyReward, SaveError> { 
    let archetype = parse_archetype(fields[1])?.ok_or(SaveError::InvalidState)?; 
    let reward = EnemyReward { archetype, entries: parse_reward_entries(fields[3])?, distribution: parse_distribution(fields[2])? }; 
    reward.validate().map_err(|_| SaveError::InvalidState)?; 
    Ok(reward) 
}

fn hazard_type(value: EnvironmentalHazardType) -> u8 { 
    match value { 
        EnvironmentalHazardType::Radiation => 0, EnvironmentalHazardType::SpaceStorm => 1, EnvironmentalHazardType::GravitationalAnomaly => 2, EnvironmentalHazardType::DebrisField => 3 
    } 
}

fn parse_hazard(fields: &[&str]) -> Result<EnvironmentalHazard, SaveError> {
    let hazard_type = match parse_u32(fields[2])? {
        0 => EnvironmentalHazardType::Radiation,
        1 => EnvironmentalHazardType::SpaceStorm,
        2 => EnvironmentalHazardType::GravitationalAnomaly,
        3 => EnvironmentalHazardType::DebrisField,
        _ => return Err(SaveError::InvalidState),
    };
    let value = EnvironmentalHazard::new(
        HazardId::new(parse_u32(fields[1])?),
        hazard_type,
        vec3(fields, 3)?,
        float(fields[6])?,
        parse_u32(fields[7])?,
        Duration(parse_u64(fields[8])?),
        parse_bool(fields[9])?,
    );
    value.validate().map_err(|_| SaveError::InvalidState)?;
    let mut value = value;
    value.restore_accumulated(Duration(parse_u64(fields[10])?));
    Ok(value)
}

fn encounter_state(value: SpatialEncounterState) -> u8 { 
    match value { 
        SpatialEncounterState::Pending => 0, SpatialEncounterState::Active => 1, SpatialEncounterState::Resolved => 2 
    }
}

fn encounter_kind(value: SpatialEncounterKind) -> u8 { 
    match value { 
        SpatialEncounterKind::Signal { .. } => 0, SpatialEncounterKind::Hazard { .. } => 1, SpatialEncounterKind::Hostile { .. } => 2, SpatialEncounterKind::Resource { .. } => 3 
    } 
}

fn encounter_payload(value: SpatialEncounterKind) -> String { 
    match value { 
        SpatialEncounterKind::Signal { destination } => format!("d:{}", destination.value()), SpatialEncounterKind::Hazard { hazard } => format!("h:{}", hazard.value()), SpatialEncounterKind::Hostile { configuration } => format!("e:{}", enemy_configuration(configuration)), SpatialEncounterKind::Resource { item, amount } => format!("r:{}:{}", item.0, amount) 
    } 
}

fn parse_encounter(fields: &[&str]) -> Result<SpatialEncounter, SaveError> {
    let payload = fields[8];
    let kind = match parse_u32(fields[2])? {
        0 => SpatialEncounterKind::Signal {
            destination: crate::navigation::DestinationId::new(parse_prefixed_u32(payload, 'd')?),
        },
        1 => SpatialEncounterKind::Hazard {
            hazard: HazardId::new(parse_prefixed_u32(payload, 'h')?),
        },
        2 => SpatialEncounterKind::Hostile {
            configuration: parse_enemy_configuration(
                payload.strip_prefix("e:").ok_or(SaveError::MalformedRecord)?,
            )?,
        },
        3 => {
            let values = payload
                .strip_prefix("r:")
                .ok_or(SaveError::MalformedRecord)?
                .split(':')
                .collect::<Vec<_>>();
            if values.len() != 2 {
                return Err(SaveError::MalformedRecord);
            }
            SpatialEncounterKind::Resource {
                item: ItemId(parse_u32(values[0])?),
                amount: parse_u32(values[1])?,
            }
        }
        _ => return Err(SaveError::InvalidState),
    };
    let state = match parse_u32(fields[7])? {
        0 => SpatialEncounterState::Pending,
        1 => SpatialEncounterState::Active,
        2 => SpatialEncounterState::Resolved,
        _ => return Err(SaveError::InvalidState),
    };
    let encounter = SpatialEncounter {
        id: EncounterId::new(parse_u32(fields[1])?),
        kind,
        position: vec3(fields, 3)?,
        activation_radius: float(fields[6])?,
        state,
        spawned_enemy: match parse_u64(fields[9])? {
            u64::MAX => None,
            value => Some(EntityId::new(value)),
        },
    };
    encounter.validate().map_err(|_| SaveError::InvalidState)?;
    Ok(encounter)
}

fn parse_prefixed_u32(value: &str, prefix: char) -> Result<u32, SaveError> { 
    let expected = format!("{prefix}:"); parse_u32(value.strip_prefix(&expected).ok_or(SaveError::MalformedRecord)?)
}

fn enemy_configuration(value: EnemyConfiguration) -> String { 
    format!("{};{};{};{};{};{};{};{}", archetype(Some(value.archetype)), bits(value.position.x), bits(value.position.y), bits(value.position.z), value.maximum_health, weapon(value.weapon), optional_shield(value.shield), value.mobility) 
}

fn parse_enemy_configuration(value: &str) -> Result<EnemyConfiguration, SaveError> {
    let fields = value.split(';').collect::<Vec<_>>();
    if fields.len() != 8 {
        return Err(SaveError::MalformedRecord);
    }
    let archetype = parse_archetype(fields[0])?.ok_or(SaveError::InvalidState)?;
    let value = EnemyConfiguration {
        archetype,
        position: Vec3 {
            x: float(fields[1])?,
            y: float(fields[2])?,
            z: float(fields[3])?,
        },
        maximum_health: parse_u32(fields[4])?,
        weapon: parse_weapon(fields[5])?,
        shield: parse_optional_shield(fields[6])?,
        mobility: parse_u32(fields[7])?,
    };
    value.validate().map_err(|_| SaveError::InvalidState)?;
    Ok(value)
}

fn derelict_state(value: DerelictState) -> u8 { 
    match value { 
        DerelictState::Undiscovered => 0, DerelictState::Discovered => 1, DerelictState::Exploring => 2, DerelictState::Resolved => 3 
    } 
}

fn entity_ids(values: &[EntityId]) -> String { 
    values.iter().map(|value| value.value().to_string()).collect::<Vec<_>>().join(",") 
}

fn derelict_rewards(values: &[DerelictReward]) -> String { 
    values.iter().map(|value| format!("{}:{}", value.item.0, value.amount)).collect::<Vec<_>>().join(",") 
}

fn parse_derelict_rewards(value: &str) -> Result<Vec<DerelictReward>, SaveError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|entry| {
            let fields = entry.split(':').collect::<Vec<_>>();
            if fields.len() != 2 {
                return Err(SaveError::MalformedRecord);
            }
            Ok(DerelictReward {
                item: ItemId(parse_u32(fields[0])?),
                amount: parse_u32(fields[1])?,
            })
        })
        .collect()
}

fn parse_entity_ids(value: &str) -> Result<Vec<EntityId>, SaveError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|value| Ok(EntityId::new(parse_u64(value)?)))
        .collect()
}

fn parse_derelict(fields: &[&str]) -> Result<Derelict, SaveError> {
    let state = match parse_u32(fields[6])? {
        0 => DerelictState::Undiscovered,
        1 => DerelictState::Discovered,
        2 => DerelictState::Exploring,
        3 => DerelictState::Resolved,
        _ => return Err(SaveError::InvalidState),
    };
    let associated_encounter = match parse_u32(fields[7])? {
        u32::MAX => None,
        value => Some(EncounterId::new(value)),
    };
    let value = Derelict {
        id: crate::derelict::DerelictId::new(parse_u32(fields[1])?),
        position: vec3(fields, 2)?,
        interaction_range: float(fields[5])?,
        state,
        associated_encounter,
        enemy_configuration: if fields[8] == "x" {
            None
        } else {
            Some(parse_enemy_configuration(fields[8])?)
        },
        rewards: Vec::new(),
        distribution: parse_distribution(fields[9])?,
        spawned_enemy: match parse_u64(fields[10])? {
            u64::MAX => None,
            value => Some(EntityId::new(value)),
        },
        enemy_defeated: parse_bool(fields[11])?,
        reward_collectibles: parse_entity_ids(fields[12])?,
        reward_generated: parse_bool(fields[13])?,
    };
    value.validate().map_err(|_| SaveError::InvalidState)?;
    Ok(value)
}

fn world_activity(value: &WorldActivity) -> String { 
    match value { 
        WorldActivity::Trade(market) => format!("t;{}", market.offers.iter().map(|offer| format!("{}:{}:{}:{}", offer.item.0, offer.buy_price, offer.sell_price, offer.stock)).collect::<Vec<_>>().join(",")), WorldActivity::Resource(site) => format!("r;{};{};{}", site.item.0, site.available, site.extraction_amount), WorldActivity::Combat(zone) => format!("c;{};{};{}", enemy_configuration(zone.configuration), zone.active_enemy.map_or(u64::MAX, EntityId::value), bool_value(zone.resolved)), WorldActivity::Story(site) => format!("s;{};{}", knowledge_target(site.target), bool_value(site.interacted)) 
    } 
}

fn parse_world_activity(value: &str) -> Result<WorldActivity, SaveError> {
    let fields = value.split(';').collect::<Vec<_>>();
    match fields.first().copied() {
        Some("t") => {
            let offers = if fields.get(1).is_some_and(|value| !value.is_empty()) {
                fields[1]
                    .split(',')
                    .map(|offer| {
                        let values = offer.split(':').collect::<Vec<_>>();
                        if values.len() != 4 {
                            return Err(SaveError::MalformedRecord);
                        }
                        Ok(TradeOffer {
                            item: ItemId(parse_u32(values[0])?),
                            buy_price: parse_u64(values[1])?,
                            sell_price: parse_u64(values[2])?,
                            stock: parse_u32(values[3])?,
                        })
                    })
                    .collect::<Result<Vec<_>, SaveError>>()?
            } else {
                Vec::new()
            };
            Ok(WorldActivity::Trade(TradeMarket { offers }))
        }
        Some("r") if fields.len() == 4 => Ok(WorldActivity::Resource(ResourceSite {
            item: ItemId(parse_u32(fields[1])?),
            available: parse_u32(fields[2])?,
            extraction_amount: parse_u32(fields[3])?,
        })),
        Some("c") if fields.len() == 11 => Ok(WorldActivity::Combat(CombatZone {
            configuration: parse_enemy_configuration(&fields[1..9].join(";"))?,
            active_enemy: match parse_u64(fields[9])? {
                u64::MAX => None,
                value => Some(EntityId::new(value)),
            },
            resolved: parse_bool(fields[10])?,
        })),
        Some("s") if fields.len() == 3 => Ok(WorldActivity::Story(StorySite {
            target: parse_knowledge_target(fields[1])?,
            interacted: parse_bool(fields[2])?,
        })),
        _ => Err(SaveError::MalformedRecord),
    }
}

pub(crate) fn validate_persisted_functional_world(value: &FunctionalWorld) -> Result<(), SaveError> {
    if value.destination.value() == 0 || !value.interaction_range.is_finite() || value.interaction_range <= 0.0 || value.activities.is_empty() { return Err(SaveError::InvalidState); }
    let mut categories = Vec::new();
    for activity in &value.activities {
        if categories.contains(&activity.category()) { return Err(SaveError::DuplicateRecord); }
        categories.push(activity.category());
        match activity {
            WorldActivity::Trade(market) => {
                if market.offers.is_empty() || market.offers.iter().any(|offer| offer.item.0 == 0 || offer.buy_price == 0 || offer.sell_price == 0) { return Err(SaveError::InvalidState); }
                let mut items = Vec::new();
                for offer in &market.offers { if !items.iter().all(|item| *item != offer.item) { return Err(SaveError::DuplicateRecord); } items.push(offer.item); }
            }
            WorldActivity::Resource(site) => {
                if site.item.0 == 0 || site.extraction_amount == 0 { return Err(SaveError::InvalidState); }
            }
            WorldActivity::Combat(zone) => zone.configuration.validate().map_err(|_| SaveError::InvalidState)?,
            WorldActivity::Story(_) => {}
        }
    }
    Ok(())
}
use engine::{EntityId, EntityRegistry, EntityRegistrySnapshot, GameTime, Tick, Duration, Vec3};
use std::fmt;

use crate::{
    domains::{mission::MissionState, objective::ObjectiveState},
    knowledge::{KnowledgeState, KnowledgeTarget},
    navigation::{Destination, DestinationId, DestinationType, DiscoveryState, NavigationRoute, NavigationRouteState},
    types::{ItemId, MissionId, ObjectiveId, SimulationPhase},
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
    pub inventories: Vec<InventorySaveState>,
    pub combatants: Vec<CombatantSaveState>,
    pub currencies: Vec<(EntityId, u64)>,
    pub missions: Vec<MissionSaveState>,
    pub knowledge: Vec<(KnowledgeTarget, KnowledgeState)>,
    pub collectibles: Vec<CollectibleSaveState>,
    pub ship_fuel: Vec<(EntityId, u32, u32)>,
    pub ship_integrity: Vec<(EntityId, u32, u32, u32, u32)>,
    pub ship_position: Option<Vec3>,
    pub destinations: Vec<Destination>,
    pub routes: Vec<NavigationRoute>,
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
        for inventory in &self.inventories {
            output.push_str(&format!(
                "inventory|{}|{}|{}\n",
                inventory.owner.value(), inventory.capacity, inventory_entries(&inventory.entries)
            ));
        }
        for combatant in &self.combatants {
            output.push_str(&format!(
                "combatant|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                combatant.id.value(), bool_value(combatant.is_player), bool_value(combatant.active),
                combatant.maximum_health, combatant.current_health, bits(combatant.position.x),
                bits(combatant.position.y), bits(combatant.position.z), 0, 0
            ));
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
            players: Vec::new(), inventories: Vec::new(), combatants: Vec::new(), currencies: Vec::new(),
            missions: Vec::new(), knowledge: Vec::new(), collectibles: Vec::new(), ship_fuel: Vec::new(),
            ship_integrity: Vec::new(), ship_position: None,
            destinations: Vec::new(), routes: Vec::new(),
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
                "inventory" if fields.len() == 4 => state.inventories.push(InventorySaveState { owner: EntityId::new(parse_u64(fields[1])?), capacity: parse_u32(fields[2])?, entries: parse_inventory(fields[3])? }),
                "combatant" if fields.len() == 11 => state.combatants.push(CombatantSaveState { id: EntityId::new(parse_u64(fields[1])?), is_player: parse_bool(fields[2])?, active: parse_bool(fields[3])?, maximum_health: parse_u32(fields[4])?, current_health: parse_u32(fields[5])?, position: vec3(&fields, 6)? }),
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
    if state.players.iter().any(|v| !state.registry.known.contains(&v.id)) { return Err(SaveError::InvalidReference); }
    Ok(())
}

fn bits(value: f32) -> u32 { value.to_bits() }
fn float(value: &str) -> Result<f32, SaveError> { Ok(f32::from_bits(parse_u32(value)?)) }
fn vec3(fields: &[&str], index: usize) -> Result<Vec3, SaveError> { Ok(Vec3 { x: float(fields[index])?, y: float(fields[index + 1])?, z: float(fields[index + 2])? }) }
fn parse_u32(value: &str) -> Result<u32, SaveError> { value.parse().map_err(|_| SaveError::InvalidNumber) }
fn parse_u64(value: &str) -> Result<u64, SaveError> { value.parse().map_err(|_| SaveError::InvalidNumber) }
fn bool_value(value: bool) -> u8 { u8::from(value) }
fn parse_bool(value: &str) -> Result<bool, SaveError> { match value { "0" => Ok(false), "1" => Ok(true), _ => Err(SaveError::MalformedRecord) } }
fn ids(values: &[EntityId]) -> String { values.iter().map(|v| v.value().to_string()).collect::<Vec<_>>().join(",") }
fn parse_ids(value: &str) -> Result<Vec<EntityId>, SaveError> { if value.is_empty() { return Ok(Vec::new()); } value.split(',').map(|v| Ok(EntityId::new(parse_u64(v)?))).collect() }
fn inventory_entries(values: &[(ItemId, u32)]) -> String { values.iter().map(|(id, amount)| format!("{}:{amount}", id.0)).collect::<Vec<_>>().join(",") }
fn parse_inventory(value: &str) -> Result<Vec<(ItemId, u32)>, SaveError> { if value.is_empty() { return Ok(Vec::new()); } value.split(',').map(|v| { let mut fields = v.split(':'); Ok((ItemId(parse_u32(fields.next().ok_or(SaveError::MalformedRecord)?)?), parse_u32(fields.next().ok_or(SaveError::MalformedRecord)?)?)) }).collect() }
fn mission_state(value: MissionState) -> u8 { match value { MissionState::Locked => 0, MissionState::Available => 1, MissionState::Active => 2, MissionState::Completed => 3, MissionState::Failed => 4 } }
fn phase(value: SimulationPhase) -> u8 { match value { SimulationPhase::Loading => 0, SimulationPhase::Running => 1, SimulationPhase::Paused => 2, SimulationPhase::Complete => 3 } }
fn parse_phase(value: &str) -> Result<SimulationPhase, SaveError> { match parse_u32(value)? { 0 => Ok(SimulationPhase::Loading), 1 => Ok(SimulationPhase::Running), 2 => Ok(SimulationPhase::Paused), 3 => Ok(SimulationPhase::Complete), _ => Err(SaveError::InvalidState) } }
fn destination_type(value: DestinationType) -> u8 { match value { DestinationType::Planet => 0, DestinationType::SpacePoint => 1 } }
fn parse_destination_type(value: &str) -> Result<DestinationType, SaveError> { match parse_u32(value)? { 0 => Ok(DestinationType::Planet), 1 => Ok(DestinationType::SpacePoint), _ => Err(SaveError::InvalidState) } }
fn discovery(value: DiscoveryState) -> u8 { match value { DiscoveryState::Unknown => 0, DiscoveryState::Known => 1, DiscoveryState::Discovered => 2 } }
fn parse_discovery(value: &str) -> Result<DiscoveryState, SaveError> { match parse_u32(value)? { 0 => Ok(DiscoveryState::Unknown), 1 => Ok(DiscoveryState::Known), 2 => Ok(DiscoveryState::Discovered), _ => Err(SaveError::InvalidState) } }
fn route_state(value: NavigationRouteState) -> u8 { match value { NavigationRouteState::Available => 0, NavigationRouteState::Blocked => 1 } }
fn parse_route_state(value: &str) -> Result<NavigationRouteState, SaveError> { match parse_u32(value)? { 0 => Ok(NavigationRouteState::Available), 1 => Ok(NavigationRouteState::Blocked), _ => Err(SaveError::InvalidState) } }
fn objective_state(value: ObjectiveState) -> u8 { match value { ObjectiveState::Locked => 0, ObjectiveState::Available => 1, ObjectiveState::Active => 2, ObjectiveState::Completed => 3, ObjectiveState::Failed => 4 } }
fn knowledge_state(value: KnowledgeState) -> u8 { match value { KnowledgeState::Unknown => 0, KnowledgeState::Detected => 1, KnowledgeState::Identified => 2, KnowledgeState::Discovered => 3, KnowledgeState::Studied => 4 } }
fn knowledge_target(value: KnowledgeTarget) -> String { match value { KnowledgeTarget::Destination(id) => format!("d{}", id.value()), KnowledgeTarget::Enemy(id) => format!("e{}", id.value()), KnowledgeTarget::Item(id) => format!("i{}", id.0) } }
fn parse_mission_state(value: &str) -> Result<MissionState, SaveError> { match parse_u32(value)? { 0 => Ok(MissionState::Locked), 1 => Ok(MissionState::Available), 2 => Ok(MissionState::Active), 3 => Ok(MissionState::Completed), 4 => Ok(MissionState::Failed), _ => Err(SaveError::InvalidState) } }
fn parse_objective_state(value: &str) -> Result<ObjectiveState, SaveError> { match parse_u32(value)? { 0 => Ok(ObjectiveState::Locked), 1 => Ok(ObjectiveState::Available), 2 => Ok(ObjectiveState::Active), 3 => Ok(ObjectiveState::Completed), 4 => Ok(ObjectiveState::Failed), _ => Err(SaveError::InvalidState) } }
fn parse_knowledge_state(value: &str) -> Result<KnowledgeState, SaveError> { match parse_u32(value)? { 0 => Ok(KnowledgeState::Unknown), 1 => Ok(KnowledgeState::Detected), 2 => Ok(KnowledgeState::Identified), 3 => Ok(KnowledgeState::Discovered), 4 => Ok(KnowledgeState::Studied), _ => Err(SaveError::InvalidState) } }
fn parse_knowledge_target(value: &str) -> Result<KnowledgeTarget, SaveError> { if value.len() < 2 { return Err(SaveError::MalformedRecord); } let (kind, number) = value.split_at(1); let id = parse_u64(number)?; match kind { "d" => Ok(KnowledgeTarget::Destination(crate::navigation::DestinationId::new(u32::try_from(id).map_err(|_| SaveError::InvalidNumber)?))), "e" => Ok(KnowledgeTarget::Enemy(EntityId::new(id))), "i" => Ok(KnowledgeTarget::Item(ItemId(u32::try_from(id).map_err(|_| SaveError::InvalidNumber)?))), _ => Err(SaveError::MalformedRecord) } }
use engine::{EntityId, EntityRegistry, GameTime};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    collectible::{Collectible, CollectibleError, ItemDefinition, ItemDefinitionError},
    combat::{Combatant, EnemyConfiguration},
    components::{Inventory, InventoryError},
    door::{Door, DoorAccessRequirement},
    electrical::{ElectricalPanel, ElectricalPanelConfiguration, ElectricalPanelRegistrationError},
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
    revival::{RevivalError, ReviveResult},
    reward::{EnemyReward, RewardError},
    station::{Station, StationType},
    types::{GameplayState, SimulationPhase},
};

#[derive(Debug, Default)]
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
    enemy_rewards: BTreeMap<crate::combat::EnemyArchetype, EnemyReward>,
    processed_enemy_rewards: BTreeSet<EntityId>,
    knowledge: BTreeMap<KnowledgeTarget, KnowledgeState>,
    events: EventLog,
}

impl GameplayWorld {
    pub fn new() -> Self {
        Self::default()
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
    pub fn players(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
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
    pub fn create_player(&mut self) -> EntityId {
        let entity = self.entities.create();
        let player = Player::new(entity);
        self.players.insert(entity, player);
        self.combatants.insert(entity, Combatant::player(entity));
        self.events
            .push(crate::events::GameplayEvent::PlayerCreated {
                player: player.id(),
                position: player.position,
            });
        entity
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
        let entity = self.entities.create();
        let collectible = Collectible {
            entity,
            item,
            amount,
            position,
            interaction_range,
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
        let collectibles: Vec<_> = reward
            .entries
            .iter()
            .map(|entry| {
                let entity = self.entities.create();
                Collectible {
                    entity,
                    item: entry.item,
                    amount: entry.amount,
                    position,
                    interaction_range: 2.0,
                    active: true,
                }
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
use engine::{EntityId, EntityRegistry, GameTime};
use std::collections::BTreeMap;

use crate::{
    collectible::{Collectible, CollectibleError, ItemDefinition, ItemDefinitionError},
    combat::{Combatant, EnemyConfiguration},
    components::{Inventory, InventoryError},
    door::{Door, DoorAccessRequirement},
    electrical::{ElectricalPanel, ElectricalPanelConfiguration, ElectricalPanelRegistrationError},
    events::EventLog,
    interaction::{Interactable, InteractionBehavior, InteractionRegistrationError},
    life_support::{LifeSupportConfiguration, LifeSupportPanel, LifeSupportRegistrationError},
    mission::{Mission, MissionRegistrationError},
    navigation::{
        Destination, DestinationId, NavigationRegistrationError, NavigationState, TravelProgress,
        TravelState,
    },
    player::Player,
    resources::{FuelError, FuelTank, ShipResources},
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
    pub fn collectible(&self, entity: EntityId) -> Option<&Collectible> {
        self.collectibles.get(&entity)
    }
    pub fn collectibles(&self) -> impl Iterator<Item = &Collectible> {
        self.collectibles.values()
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
    pub fn initialize_navigation_ship(&mut self, ship: EntityId) {
        self.navigation.initialize_ship(ship);
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
        if !self.entities.contains(ship) {
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
    pub(crate) fn player_mut(&mut self, entity: EntityId) -> Option<&mut Player> {
        self.players.get_mut(&entity)
    }
    pub(crate) fn combatant_mut(&mut self, entity: EntityId) -> Option<&mut Combatant> {
        self.combatants.get_mut(&entity)
    }
    pub(crate) fn inventory_mut(&mut self, player: EntityId) -> Option<&mut Inventory> {
        self.inventories.get_mut(&player)
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
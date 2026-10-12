pub mod cooperacion;
pub mod ecos_anomalia;
pub mod el_despertar;
pub mod la_defensa_imposible;
pub mod tormenta_gravitacional;

use engine::EntityId;
use gameplay::types::MissionId;
use gameplay::{
    systems::{
        CombatSystem, CompanionSystem, DoorSystem, ElectricalSystem, EnemyAiSystem,
        EnvironmentalHazardSystem, HullIntegritySystem, InteractionSystem, InventorySystem,
        LifeSupportSystem, NavigationSystem, PilotSystem, PlayerSystem, ResourceSystem,
        ScannerSystem, StationSystem, DerelictSystem, FunctionalWorldSystem,
        SpatialEncounterSystem,
    },
    CommandResult, DoorAccessRequirement, GameplayCommand, GameplayProcessor, GameplayState,
    GameplayWorld,
};
use gameplay::{
    HealingSystem, KnowledgeSystem, Mission, MissionProgress, MissionRegistrationError,
    MissionSystem, RevivalSystem, RewardSystem,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectEchoIds {
    pub first_player: EntityId,
    pub first_ship: EntityId,
}

pub struct ProjectEchoSession {
    pub ids: ProjectEchoIds,
    world: GameplayWorld,
    processor: GameplayProcessor,
}

const INITIAL_FUEL_CAPACITY: u32 = 100;
const INITIAL_FUEL: u32 = 60;
const INITIAL_HULL_MAXIMUM: u32 = 100;
const INITIAL_HULL: u32 = 100;
const CRITICAL_HULL_THRESHOLD: u32 = 25;

impl ProjectEchoSession {
    pub fn new() -> Self {
        let mut world = GameplayWorld::new();
        world.start();
        let first_player = world
            .create_player()
            .expect("el jugador inicial debe poder crearse");
        world
            .initialize_inventory(first_player, 20)
            .expect("el inventario inicial del jugador debe ser válido");
        let repair_configuration = gameplay::ShipRepairConfiguration::default();
        world
            .register_item_definition(gameplay::ItemDefinition::new(
                repair_configuration.material,
                gameplay::ItemCategory::RepairMaterial,
                20,
            ))
            .expect("el material de reparación debe registrarse una sola vez");
        world
            .register_item_definition(gameplay::ItemDefinition::new(
                gameplay::types::ItemId(2),
                gameplay::ItemCategory::CombatAmmunition,
                100,
            ))
            .expect("la munición estándar debe registrarse una sola vez");
        world
            .register_item_definition(gameplay::ItemDefinition::new(
                gameplay::types::ItemId(3),
                gameplay::ItemCategory::CombatAmmunition,
                100,
            ))
            .expect("la munición de energía debe registrarse una sola vez");
        world
            .register_item_definition(
                gameplay::ItemDefinition::new(
                    gameplay::types::ItemId(4),
                    gameplay::ItemCategory::HealingConsumable,
                    10,
                )
                .with_healing(gameplay::HealingEffect::new(25)),
            )
            .expect("el consumible curativo debe registrarse una sola vez");
        world
            .register_item_definition(
                gameplay::ItemDefinition::new(
                    gameplay::types::ItemId(5),
                    gameplay::ItemCategory::RevivalConsumable,
                    5,
                )
                .with_revival(gameplay::ReviveEffect::new(
                    gameplay::types::ItemId(5),
                    1,
                    30,
                    3,
                )),
            )
            .expect("el consumible de reanimaciÃ³n debe registrarse una sola vez");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Shielded,
                vec![gameplay::RewardEntry {
                    item: repair_configuration.material,
                    amount: 1,
                }],
            ))
            .expect("la recompensa del enemigo escudado debe ser vÃ¡lida");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Shooter,
                vec![gameplay::RewardEntry {
                    item: gameplay::types::ItemId(3),
                    amount: 1,
                }],
            ))
            .expect("la recompensa del tirador debe ser vÃ¡lida");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Charger,
                vec![gameplay::RewardEntry {
                    item: repair_configuration.material,
                    amount: 2,
                }],
            ))
            .expect("la recompensa del embestidor debe ser vÃ¡lida");
        let first_ship = world.create_entity();
        world
            .initialize_navigation_ship(first_ship)
            .expect("la nave inicial debe registrarse una sola vez");
        world
            .initialize_ship_integrity(
                first_ship,
                INITIAL_HULL_MAXIMUM,
                INITIAL_HULL,
                CRITICAL_HULL_THRESHOLD,
            )
            .expect("la integridad inicial de la nave debe ser válida");
        world
            .initialize_ship_resources(first_ship, INITIAL_FUEL_CAPACITY, INITIAL_FUEL)
            .expect("los recursos iniciales de la nave deben ser válidos");
        let awakening =
            el_despertar::definition().expect("El despertar debe tener una definición válida");
        world
            .register_mission(awakening)
            .expect("La misión El despertar debe registrarse una sola vez");
        let anomaly = ecos_anomalia::definition()
            .expect("La mision Ecos de la anomalia debe tener una definicion valida");
        world
            .register_mission(anomaly)
            .expect("La mision Ecos de la anomalia debe registrarse una sola vez");
        let storm = tormenta_gravitacional::definition()
            .expect("La mision Tormenta gravitacional debe tener una definicion valida");
        world
            .register_mission(storm)
            .expect("La mision Tormenta gravitacional debe registrarse una sola vez");
        let defense = la_defensa_imposible::definition()
            .expect("La mision La defensa imposible debe tener una definicion valida");
        world
            .register_mission(defense)
            .expect("La mision La defensa imposible debe registrarse una sola vez");
        let mut processor = GameplayProcessor::new();
        processor.add_system(PlayerSystem);
        processor.add_system(CompanionSystem::new());
        processor.add_system(CombatSystem);
        processor.add_system(EnemyAiSystem::new());
        processor.add_system(RewardSystem::new());
        processor.add_system(InventorySystem);
        processor.add_system(HealingSystem);
        processor.add_system(RevivalSystem);
        processor.add_system(HullIntegritySystem);
        processor.add_system(PilotSystem::new());
        processor.add_system(ScannerSystem);
        processor.add_system(InteractionSystem);
        processor.add_system(ElectricalSystem::new());
        processor.add_system(LifeSupportSystem::new());
        processor.add_system(EnvironmentalHazardSystem::new());
        processor.add_system(NavigationSystem::new());
        processor.add_system(ResourceSystem);
        processor.add_system(DoorSystem);
        processor.add_system(StationSystem);
        processor.add_system(KnowledgeSystem::new());
        processor.add_system(MissionSystem::new());
        processor.add_system(SpatialEncounterSystem::new());
        processor.add_system(DerelictSystem::new());
        processor.add_system(FunctionalWorldSystem::new());
        Self {
            ids: ProjectEchoIds {
                first_player,
                first_ship,
            },
            world,
            processor,
        }
    }

    pub fn world(&self) -> &GameplayWorld {
        &self.world
    }
    pub fn world_mut(&mut self) -> &mut GameplayWorld {
        &mut self.world
    }

    pub fn save_state(&self) -> gameplay::GameplaySaveState {
        self.world.save_state()
    }

    pub fn restore_state(
        &mut self,
        save: gameplay::GameplaySaveState,
    ) -> Result<(), gameplay::SaveError> {
        self.world.load_state(save)
    }
    pub fn state(&self) -> GameplayState {
        self.world.state()
    }

    pub fn create_console(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<engine::EntityId, gameplay::InteractionRegistrationError> {
        self.world.create_console(position, interaction_range)
    }

    pub fn create_ship_status_console(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<engine::EntityId, gameplay::InteractionRegistrationError> {
        self.world
            .create_ship_status_console(position, interaction_range)
    }

    pub fn create_door(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<engine::EntityId, gameplay::InteractionRegistrationError> {
        self.world.create_door(position, interaction_range)
    }

    pub fn create_bridge_door(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
    ) -> Result<engine::EntityId, gameplay::InteractionRegistrationError> {
        self.world.create_bridge_door_with_requirement(
            position,
            interaction_range,
            DoorAccessRequirement::MissionObjectives {
                mission: el_despertar::EL_DESPERTAR,
                required: [
                    el_despertar::RESTAURAR_ENERGIA,
                    el_despertar::RESTABLECER_SOPORTE_VITAL,
                ],
            },
        )
    }

    pub fn create_station(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        station_type: gameplay::StationType,
    ) -> Result<engine::EntityId, gameplay::InteractionRegistrationError> {
        self.world
            .create_station(position, interaction_range, station_type)
    }

    pub fn create_electrical_panel(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        configuration: gameplay::ElectricalPanelConfiguration,
    ) -> Result<engine::EntityId, gameplay::ElectricalPanelRegistrationError> {
        self.world
            .create_electrical_panel(position, interaction_range, configuration)
    }

    pub fn create_life_support_panel(
        &mut self,
        position: engine::Vec3,
        interaction_range: f32,
        configuration: gameplay::LifeSupportConfiguration,
    ) -> Result<engine::EntityId, gameplay::LifeSupportRegistrationError> {
        self.world
            .create_life_support_panel(position, interaction_range, configuration)
    }

    pub fn register_destination(
        &mut self,
        destination: gameplay::Destination,
    ) -> Result<(), gameplay::NavigationRegistrationError> {
        self.world.register_destination(destination)
    }

    pub fn register_mission(&mut self, mission: Mission) -> Result<(), MissionRegistrationError> {
        self.world.register_mission(mission)
    }

    pub fn mission_progress(&self, mission: MissionId) -> Option<MissionProgress> {
        self.world.mission(mission).map(Mission::progress)
    }

    pub fn update<I>(&mut self, commands: I) -> Vec<CommandResult>
    where
        I: IntoIterator<Item = GameplayCommand>,
    {
        self.processor.update(&mut self.world, commands)
    }
}

impl Default for ProjectEchoSession {
    fn default() -> Self {
        Self::new()
    }
}
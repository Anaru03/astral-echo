pub mod campaign;
pub mod cooperacion;
pub mod ecos_anomalia;
pub mod el_despertar;
pub mod la_defensa_imposible;
pub mod multiplayer;
pub mod render_backend;
pub mod render_bridge;
pub mod tormenta_gravitacional;

use engine::EntityId;
use gameplay::types::MissionId;
use gameplay::{
    systems::{
        CombatSystem, CompanionSystem, DerelictSystem, DoorSystem, ElectricalSystem, EnemyAiSystem,
        EnvironmentalHazardSystem, FunctionalWorldSystem, HullIntegritySystem, InteractionSystem,
        InventorySystem, LifeSupportSystem, NavigationSystem, PilotSystem, PlayerSystem,
        ResourceSystem, ScannerSystem, SpatialEncounterSystem, StationSystem,
    },
    CommandResult, DoorAccessRequirement, GameplayCommand, GameplayProcessor, GameplayState,
    GameplayWorld,
};
use gameplay::{CommandStatus, Destination, DestinationType, EnemyArchetype, EnemyConfiguration};
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
const INITIAL_FUEL: u32 = INITIAL_FUEL_CAPACITY;
const INITIAL_INVENTORY_CAPACITY: u32 = 250;
const INITIAL_FUEL_RESERVE: u32 = 200;
const INITIAL_OXYGEN_RESERVE: u32 = 5;
const INITIAL_REPAIR_MATERIALS: u32 = 3;
const INITIAL_KINETIC_AMMUNITION: u32 = 8;
const INITIAL_ENERGY_AMMUNITION: u32 = 4;
const INITIAL_HULL_MAXIMUM: u32 = 100;
const INITIAL_HULL: u32 = 100;
const CRITICAL_HULL_THRESHOLD: u32 = 25;

fn register_campaign_missions(world: &mut GameplayWorld) -> Result<(), MissionRegistrationError> {
    let missions = [
        el_despertar::definition().expect("El despertar debe tener una definicion valida"),
        ecos_anomalia::definition().expect("Ecos de la anomalia debe tener una definicion valida"),
        tormenta_gravitacional::definition()
            .expect("Tormenta gravitacional debe tener una definicion valida"),
        la_defensa_imposible::definition()
            .expect("La defensa imposible debe tener una definicion valida"),
        cooperacion::definition().expect("Progresion cooperativa debe tener una definicion valida"),
    ];

    for mission in missions {
        world.register_mission(mission)?;
    }
    Ok(())
}

impl ProjectEchoSession {
    pub fn new() -> Self {
        let mut world = GameplayWorld::new();
        world.start();
        let first_player = world
            .create_player()
            .expect("el jugador inicial debe poder crearse");
        world
            .initialize_inventory(first_player, INITIAL_INVENTORY_CAPACITY)
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
            .register_item_definition(gameplay::ItemDefinition::new(
                gameplay::FUEL_RESOURCE_ITEM,
                gameplay::ItemCategory::Fuel,
                INITIAL_FUEL_RESERVE,
            ))
            .expect("el combustible recuperable debe registrarse una sola vez");
        world
            .register_item_definition(gameplay::ItemDefinition::new(
                gameplay::OXYGEN_RESOURCE_ITEM,
                gameplay::ItemCategory::Oxygen,
                20,
            ))
            .expect("el oxigeno recuperable debe registrarse una sola vez");
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
            .expect("el consumible de reanimación debe registrarse una sola vez");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Shielded,
                vec![gameplay::RewardEntry {
                    item: repair_configuration.material,
                    amount: 1,
                }],
            ))
            .expect("la recompensa del enemigo escudado debe ser válida");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Shooter,
                vec![gameplay::RewardEntry {
                    item: gameplay::types::ItemId(3),
                    amount: 3,
                }],
            ))
            .expect("la recompensa del tirador debe ser válida");
        world
            .register_enemy_reward(gameplay::EnemyReward::new(
                gameplay::EnemyArchetype::Charger,
                vec![gameplay::RewardEntry {
                    item: repair_configuration.material,
                    amount: 2,
                }],
            ))
            .expect("la recompensa del embestidor debe ser válida");
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
        register_campaign_missions(&mut world)
            .expect("Las misiones de campaña deben registrarse una sola vez");
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
        let mut session = Self {
            ids: ProjectEchoIds {
                first_player,
                first_ship,
            },
            world,
            processor,
        };
        for (item, amount) in [
            (repair_configuration.material, INITIAL_REPAIR_MATERIALS),
            (gameplay::types::ItemId(2), INITIAL_KINETIC_AMMUNITION),
            (gameplay::types::ItemId(3), INITIAL_ENERGY_AMMUNITION),
            (gameplay::FUEL_RESOURCE_ITEM, INITIAL_FUEL_RESERVE),
            (gameplay::OXYGEN_RESOURCE_ITEM, INITIAL_OXYGEN_RESERVE),
        ] {
            session
                .world_mut()
                .seed_inventory_item(first_player, item, amount)
                .expect("los recursos iniciales deben caber en el inventario");
        }
        session
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

    /// Reintenta preparar el contenido de la misión cooperativa después de un
    /// inicio aceptado cuya preparación no pudo completarse.
    ///
    /// La operación no cambia el estado de la misión ni reinicia objetivos;
    /// únicamente puede ejecutarse mientras Mission5 permanece activa.
    pub fn retry_cooperative_mission_content(&mut self) -> CommandResult {
        let mission = cooperacion::PROGRESION_COOPERATIVA;
        if !self
            .world
            .mission(mission)
            .is_some_and(|value| value.state() == gameplay::domains::mission::MissionState::Active)
        {
            return CommandResult::rejected(gameplay::CommandRejection::InvalidMissionTransition);
        }

        match self.ensure_cooperative_mission_content() {
            Ok(()) => CommandResult::success(true),
            Err(error) => {
                eprintln!("Preparación cooperativa fallida: {error}");
                CommandResult::rejected(gameplay::CommandRejection::MissionContentUnavailable)
            }
        }
    }

    pub fn update<I>(&mut self, commands: I) -> Vec<CommandResult>
    where
        I: IntoIterator<Item = GameplayCommand>,
    {
        let commands: Vec<_> = commands.into_iter().collect();
        let gameplay_commands: Vec<_> = commands
            .iter()
            .filter(|command| !matches!(command, GameplayCommand::RetryCooperativeMissionContent))
            .cloned()
            .collect();
        let processor_results = self.processor.update(&mut self.world, gameplay_commands);
        let mut processor_results = processor_results.into_iter();
        let mut results = Vec::with_capacity(commands.len());
        for command in &commands {
            if matches!(command, GameplayCommand::RetryCooperativeMissionContent) {
                results.push(self.retry_cooperative_mission_content());
            } else {
                results.push(
                    processor_results
                        .next()
                        .expect("cada comando de gameplay debe producir un resultado"),
                );
            }
        }
        let should_prepare_cooperative_mission =
            commands
                .iter()
                .zip(results.iter())
                .any(|(command, result)| {
                    matches!(
                        command,
                        GameplayCommand::StartMission { mission }
                            if *mission == cooperacion::PROGRESION_COOPERATIVA
                    ) && matches!(result.status, CommandStatus::Success { changed: true })
                });
        if should_prepare_cooperative_mission {
            if let Err(error) = self.ensure_cooperative_mission_content() {
                results.push(CommandResult::rejected(
                    gameplay::CommandRejection::MissionContentUnavailable,
                ));
                eprintln!("Preparación cooperativa fallida: {error}");
            }
        }
        campaign::unlock_following_missions(self);
        results
    }

    fn ensure_cooperative_mission_content(&mut self) -> Result<(), String> {
        let destination = cooperacion::SIGNAL_DESTINATION;
        if !self.world.players().any(|player| {
            player.is_active
                && self.world.entities().contains(player.entity_id)
                && self.world.inventory(player.entity_id).is_some()
        }) {
            return Err("no existe un jugador activo con inventario".to_owned());
        }

        if let Some(existing) = self.world.destination(destination) {
            if existing.destination_type != DestinationType::SpacePoint {
                return Err(format!(
                    "el destino {destination:?} no es una señal espacial compatible"
                ));
            }
        } else {
            self.world
                .register_destination(Destination {
                    id: destination,
                    destination_type: DestinationType::SpacePoint,
                    position: engine::Vec3::ZERO,
                    travel_duration: engine::Duration(100),
                    fuel_cost: 1,
                    discovery: gameplay::DiscoveryState::Unknown,
                })
                .map_err(|error| format!("registrar destino cooperativo: {error:?}"))?;
        }

        if self.world.combatants().any(|combatant| {
            !combatant.is_player
                && combatant.active
                && combatant.archetype == Some(EnemyArchetype::Shooter)
        }) {
            return Ok(());
        }

        let result = self.processor.process_followup(
            &mut self.world,
            GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(
                    EnemyArchetype::Shooter,
                    engine::Vec3::ZERO,
                ),
            },
        );
        if matches!(result.status, CommandStatus::Success { changed: true }) {
            Ok(())
        } else {
            Err(format!(
                "crear amenaza cooperativa rechazado: {:?}",
                result.status
            ))
        }
    }
}

impl Default for ProjectEchoSession {
    fn default() -> Self {
        Self::new()
    }
}

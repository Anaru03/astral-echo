pub mod el_despertar;

use engine::EntityId;
use gameplay::types::MissionId;
use gameplay::{
    systems::{DoorSystem, ElectricalSystem, InteractionSystem, PlayerSystem, StationSystem},
    CommandResult, GameplayCommand, GameplayProcessor, GameplayState, GameplayWorld,
};
use gameplay::{Mission, MissionProgress, MissionRegistrationError, MissionSystem};

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

impl ProjectEchoSession {
    pub fn new() -> Self {
        let mut world = GameplayWorld::new();
        world.start();
        let first_player = world.create_player();
        let first_ship = world.create_entity();
        let awakening =
            el_despertar::definition().expect("El despertar debe tener una definición válida");
        world
            .register_mission(awakening)
            .expect("La misión El despertar debe registrarse una sola vez");
        let mut processor = GameplayProcessor::new();
        processor.add_system(PlayerSystem);
        processor.add_system(InteractionSystem);
        processor.add_system(ElectricalSystem::new());
        processor.add_system(DoorSystem);
        processor.add_system(StationSystem);
        processor.add_system(MissionSystem::new());
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
        self.world.create_bridge_door(position, interaction_range)
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
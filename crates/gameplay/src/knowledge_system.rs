use crate::{
    commands::{CommandRejection, CommandResult, GameplayCommand},
    events::GameplayEvent,
    knowledge::{KnowledgeError, KnowledgeState, KnowledgeTarget},
    systems::GameplaySystem,
    world::GameplayWorld,
};

pub struct KnowledgeSystem {
    event_cursor: usize,
}

impl KnowledgeSystem {
    pub const fn new() -> Self {
        Self { event_cursor: 0 }
    }

    fn register(&self, world: &mut GameplayWorld, target: KnowledgeTarget) -> CommandResult {
        match world.register_knowledge(target) {
            Ok(changed) => CommandResult::success(changed),
            Err(KnowledgeError::TargetNotFound) => {
                CommandResult::rejected(CommandRejection::KnowledgeTargetNotFound)
            }
            Err(KnowledgeError::AlreadyRegistered) => {
                CommandResult::rejected(CommandRejection::KnowledgeAlreadyRegistered)
            }
            _ => CommandResult::rejected(CommandRejection::KnowledgeTransitionNotAllowed),
        }
    }

    fn identify(&self, world: &mut GameplayWorld, target: KnowledgeTarget) -> CommandResult {
        let current = world.knowledge(target);
        if current == KnowledgeState::Unknown {
            return CommandResult::rejected(CommandRejection::KnowledgeUnknown);
        }
        if current >= KnowledgeState::Identified {
            return CommandResult::success(false);
        }
        match world.set_knowledge(target, KnowledgeState::Identified) {
            Ok(true) => {
                world.publish(GameplayEvent::KnowledgeIdentified { target });
                CommandResult::success(true)
            }
            Ok(false) => CommandResult::success(false),
            Err(_) => CommandResult::rejected(CommandRejection::KnowledgeTransitionNotAllowed),
        }
    }

    fn study(
        &self,
        world: &mut GameplayWorld,
        player: engine::EntityId,
        target: KnowledgeTarget,
    ) -> CommandResult {
        if world.phase() != crate::types::SimulationPhase::Running {
            return CommandResult::rejected(CommandRejection::SimulationNotRunning);
        }
        let Some(player_state) = world.player(player) else {
            return CommandResult::rejected(CommandRejection::PlayerNotFound);
        };
        if !world.entities().contains(player) || !player_state.is_active {
            return CommandResult::rejected(CommandRejection::PlayerInactive);
        }
        let current = world.knowledge(target);
        if current == KnowledgeState::Unknown {
            return CommandResult::rejected(CommandRejection::KnowledgeUnknown);
        }
        if current == KnowledgeState::Studied {
            return CommandResult::success(false);
        }
        if !current.can_study() {
            return CommandResult::rejected(CommandRejection::KnowledgeNotIdentified);
        }
        match world.set_knowledge(target, KnowledgeState::Studied) {
            Ok(true) => {
                world.publish(GameplayEvent::KnowledgeStudied { player, target });
                CommandResult::success(true)
            }
            Ok(false) => CommandResult::success(false),
            Err(_) => CommandResult::rejected(CommandRejection::KnowledgeTransitionNotAllowed),
        }
    }

    fn apply_event(
        &self,
        world: &mut GameplayWorld,
        target: KnowledgeTarget,
        state: KnowledgeState,
    ) {
        let current = world.knowledge(target);
        if state <= current || world.set_knowledge(target, state).ok() != Some(true) {
            return;
        }
        let event = match state {
            KnowledgeState::Detected => GameplayEvent::KnowledgeDetected { target },
            KnowledgeState::Identified => GameplayEvent::KnowledgeIdentified { target },
            KnowledgeState::Discovered => GameplayEvent::KnowledgeDiscovered { target },
            KnowledgeState::Unknown | KnowledgeState::Studied => return,
        };
        world.publish(event);
    }

    fn observe_new_events(&mut self, world: &mut GameplayWorld) {
        let length = world.events().len();
        if self.event_cursor > length {
            self.event_cursor = 0;
        }
        let events: Vec<_> = world
            .events()
            .iter()
            .skip(self.event_cursor)
            .cloned()
            .collect();
        self.event_cursor = length;
        for event in events {
            match event {
                GameplayEvent::SignalDetected { destination, .. } => self.apply_event(
                    world,
                    KnowledgeTarget::Destination(destination),
                    KnowledgeState::Detected,
                ),
                GameplayEvent::DestinationIdentified { destination, .. } => self.apply_event(
                    world,
                    KnowledgeTarget::Destination(destination),
                    KnowledgeState::Identified,
                ),
                GameplayEvent::PlanetDiscovered { destination } => self.apply_event(
                    world,
                    KnowledgeTarget::Destination(destination),
                    KnowledgeState::Discovered,
                ),
                GameplayEvent::EnemyCreated { enemy, .. } => self.apply_event(
                    world,
                    KnowledgeTarget::Enemy(enemy),
                    KnowledgeState::Detected,
                ),
                GameplayEvent::ItemCollected { item, .. } => {
                    self.apply_event(world, KnowledgeTarget::Item(item), KnowledgeState::Detected)
                }
                _ => {}
            }
        }
    }
}

impl Default for KnowledgeSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeSystem {
    pub fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        let result = match command {
            GameplayCommand::RegisterKnowledge { target } => self.register(world, target),
            GameplayCommand::IdentifyKnowledge { target } => self.identify(world, target),
            GameplayCommand::StudyKnowledge { player, target } => self.study(world, player, target),
            _ => return None,
        };
        Some(result)
    }
}

impl GameplaySystem for KnowledgeSystem {
    fn process(
        &mut self,
        world: &mut GameplayWorld,
        command: GameplayCommand,
    ) -> Option<CommandResult> {
        KnowledgeSystem::process(self, world, command)
    }

    fn observes_events(&self) -> bool {
        true
    }

    fn observe_events(&mut self, world: &mut GameplayWorld) {
        self.observe_new_events(world);
    }
}
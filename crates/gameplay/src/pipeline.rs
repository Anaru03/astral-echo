use engine::{Duration, Tick};

use crate::{
    commands::{CommandInvalidity, CommandResult, GameplayCommand},
    events::GameplayEvent,
    systems::{GameplaySystem, TimeSystem},
    world::GameplayWorld,
};

pub struct GameplayProcessor {
    systems: Vec<Box<dyn GameplaySystem>>,
}

impl Default for GameplayProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl GameplayProcessor {
    pub fn new() -> Self {
        Self {
            systems: vec![Box::new(TimeSystem)],
        }
    }

    pub fn add_system<S>(&mut self, system: S)
    where
        S: GameplaySystem + 'static,
    {
        self.systems.push(Box::new(system));
    }

    pub fn update<I>(&mut self, world: &mut GameplayWorld, commands: I) -> Vec<CommandResult>
    where
        I: IntoIterator<Item = GameplayCommand>,
    {
        for system in &mut self.systems {
            system.reset_event_cursor();
        }
        world.clear_events();
        commands
            .into_iter()
            .map(|command| self.process(world, command))
            .collect()
    }

    fn process(&mut self, world: &mut GameplayWorld, command: GameplayCommand) -> CommandResult {
        let mut result = None;
        for system in &mut self.systems {
            if let Some(outcome) = system.process(world, command.clone()) {
                result = Some(outcome);
                break;
            }
        }
        for system in &mut self.systems {
            if system.observes_events() {
                system.observe_events(world);
            }
        }
        result.unwrap_or_else(|| CommandResult::invalid(CommandInvalidity::UnsupportedCommand))
    }
}

pub(crate) fn advance_time(world: &mut GameplayWorld, delta: Duration) -> CommandResult {
    if world.phase() != crate::types::SimulationPhase::Running {
        return CommandResult::rejected(crate::commands::CommandRejection::SimulationNotRunning);
    }
    if delta.0 == 0 {
        return CommandResult::rejected(crate::commands::CommandRejection::InvalidDuration);
    }
    let before = world.time();
    let Some(tick) = before.tick.0.checked_add(1) else {
        return CommandResult::rejected(crate::commands::CommandRejection::InvalidDuration);
    };
    let Some(elapsed) = before.elapsed.0.checked_add(delta.0) else {
        return CommandResult::rejected(crate::commands::CommandRejection::InvalidDuration);
    };
    let after = engine::GameTime {
        tick: Tick(tick),
        elapsed: Duration(elapsed),
    };
    world.set_time(after);
    world.publish(GameplayEvent::TimeAdvanced {
        from: before,
        to: after,
    });
    CommandResult::success(true)
}
mod combat;
mod erick;
mod navigation;
mod progression;

pub(crate) fn run_combat_and_expeditions() -> Result<(), String> {
    combat::run()
}

pub(crate) fn run_navigation_and_exploration() -> Result<(), String> {
    navigation::run()
}

pub(crate) fn run_progression_and_systems() -> Result<(), String> {
    progression::run()
}

pub(crate) fn run_erick() -> Result<(), String> {
    erick::run()
}
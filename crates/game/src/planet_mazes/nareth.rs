use crate::planet_maze::{PlanetMaze, PlanetMazeKind};

pub const NAME: &str = "Nareth";
pub const SEED: u64 = 4004;
pub const MIN_PATH: usize = 82;
pub const MAX_PATH: usize = 110;
pub const SHORTCUTS: usize = 2;
pub const COLOR: [u8; 3] = [196, 154, 255];

pub fn generate() -> PlanetMaze {
    PlanetMaze::generate_balanced(PlanetMazeKind::Nareth, SEED, MIN_PATH, MAX_PATH, SHORTCUTS)
}

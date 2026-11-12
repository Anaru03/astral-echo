use crate::planet_maze::{PlanetMaze, PlanetMazeKind};

pub const NAME: &str = "Tessara";
pub const SEED: u64 = 2002;
pub const MIN_PATH: usize = 50;
pub const MAX_PATH: usize = 64;
pub const SHORTCUTS: usize = 5;
pub const COLOR: [u8; 3] = [103, 184, 232];

pub fn generate() -> PlanetMaze {
    PlanetMaze::generate_balanced(PlanetMazeKind::Tessara, SEED, MIN_PATH, MAX_PATH, SHORTCUTS)
}

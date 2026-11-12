use crate::planet_maze::{PlanetMaze, PlanetMazeKind};

pub const NAME: &str = "Building";
pub const SEED: u64 = 1001;
pub const MIN_PATH: usize = 36;
pub const MAX_PATH: usize = 50;
pub const SHORTCUTS: usize = 8;
pub const COLOR: [u8; 3] = [215, 163, 91];

pub fn generate() -> PlanetMaze {
    PlanetMaze::generate_balanced(
        PlanetMazeKind::Building,
        SEED,
        MIN_PATH,
        MAX_PATH,
        SHORTCUTS,
    )
}

use crate::planet_maze::{PlanetMaze, PlanetMazeKind};

pub const NAME: &str = "Kharvex";
pub const SEED: u64 = 3003;
pub const MIN_PATH: usize = 66;
pub const MAX_PATH: usize = 80;
pub const SHORTCUTS: usize = 3;
pub const COLOR: [u8; 3] = [231, 155, 80];

pub fn generate() -> PlanetMaze {
    PlanetMaze::generate_balanced(
        PlanetMazeKind::Kharvex,
        SEED,
        MIN_PATH,
        MAX_PATH,
        SHORTCUTS,
    )
}

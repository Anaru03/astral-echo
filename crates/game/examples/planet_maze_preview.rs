use game::planet_mazes::{
    building, tessara, kharvex, nareth,
};
use std::fs;

fn main() -> std::io::Result<()> {
    let output = "/tmp/astral_echo_mazes";
    fs::create_dir_all(output)?;

    let mazes = [
        ("building", building::generate()),
        ("tessara", tessara::generate()),
        ("kharvex", kharvex::generate()),
        ("nareth", nareth::generate()),
    ];

    for (name, maze) in mazes {
        fs::write(
            format!("{output}/{name}.txt"),
            maze.as_text(),
        )?;

        println!(
            "{}: {} pasos",
            name,
            maze.shortest_path_length().unwrap_or(0)
        );
    }

    Ok(())
}

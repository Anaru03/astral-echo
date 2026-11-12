use std::collections::VecDeque;

pub const MAZE_SIZE: usize = 21;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanetMazeKind {
    Building,
    Tessara,
    Kharvex,
    Nareth,
}

impl PlanetMazeKind {
    pub fn from_destination(id: u32) -> Option<Self> {
        match id {
            9002 => Some(Self::Building),
            9601 => Some(Self::Tessara),
            9602 => Some(Self::Kharvex),
            9603 => Some(Self::Nareth),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlanetMaze {
    pub kind: PlanetMazeKind,
    pub cells: [[bool; MAZE_SIZE]; MAZE_SIZE],
    pub entrance: (usize, usize),
    pub exit: (usize, usize),
}

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

impl PlanetMaze {
    fn generate_base(kind: PlanetMazeKind, seed: u64) -> Self {
        let mut cells = [[false; MAZE_SIZE]; MAZE_SIZE];
        let mut stack = vec![(1usize, 1usize)];
        let mut rng = seed.max(1);

        cells[1][1] = true;

        while let Some(&(x, y)) = stack.last() {
            let mut neighbors = Vec::new();

            for (dx, dy) in [(0isize, -2isize), (2, 0), (0, 2), (-2, 0)] {
                let nx = x as isize + dx;
                let ny = y as isize + dy;

                if nx > 0
                    && ny > 0
                    && nx < (MAZE_SIZE - 1) as isize
                    && ny < (MAZE_SIZE - 1) as isize
                    && !cells[ny as usize][nx as usize]
                {
                    neighbors.push((nx as usize, ny as usize));
                }
            }

            if neighbors.is_empty() {
                stack.pop();
                continue;
            }

            let (nx, ny) = neighbors[(random(&mut rng) as usize) % neighbors.len()];

            cells[(y + ny) / 2][(x + nx) / 2] = true;
            cells[ny][nx] = true;
            stack.push((nx, ny));
        }

        Self {
            kind,
            cells,
            entrance: (1, 1),
            exit: (19, 19),
        }
    }

    pub fn generate_balanced(
        kind: PlanetMazeKind,
        seed: u64,
        min_path: usize,
        max_path: usize,
        shortcuts: usize,
    ) -> Self {
        let mut best = None;

        for attempt in 0..2000u64 {
            let candidate_seed = seed.wrapping_add(attempt.wrapping_mul(7919));
            let mut maze = Self::generate_base(kind, candidate_seed);

            let mut rng = candidate_seed.max(1);
            let mut candidates = Vec::new();

            for y in 1..MAZE_SIZE - 1 {
                for x in 1..MAZE_SIZE - 1 {
                    if maze.cells[y][x] {
                        continue;
                    }

                    let horizontal = maze.cells[y][x - 1] && maze.cells[y][x + 1];
                    let vertical = maze.cells[y - 1][x] && maze.cells[y + 1][x];

                    if horizontal || vertical {
                        candidates.push((x, y));
                    }
                }
            }

            // Mezclar candidatos de forma determinista.
            for i in (1..candidates.len()).rev() {
                let j = (random(&mut rng) as usize) % (i + 1);
                candidates.swap(i, j);
            }

            let mut opened = 0;

            for (x, y) in candidates {
                if opened >= shortcuts {
                    break;
                }

                maze.cells[y][x] = true;

                if maze
                    .shortest_path_length()
                    .is_some_and(|distance| distance >= min_path)
                {
                    opened += 1;
                } else {
                    maze.cells[y][x] = false;
                }
            }

            if let Some(distance) = maze.shortest_path_length() {
                if distance >= min_path && distance <= max_path && opened == shortcuts {
                    return maze;
                }

                let error =
                    distance.abs_diff((min_path + max_path) / 2) + (shortcuts - opened) * 10;

                if best.as_ref().is_none_or(|(score, _)| error < *score) {
                    best = Some((error, maze));
                }
            }
        }

        // Nunca presentar un mapa fuera del rango como balanceado.
        // Si no se encuentra uno valido, avisar explicitamente.
        let (_, maze) = best.expect("Debe existir un laberinto generado");
        panic!(
            "No se encontro un laberinto balanceado para {:?}. \
             Mejor candidato: {:?}. Ajusta los parametros.",
            kind,
            maze.shortest_path_length()
        );
    }

    pub fn is_walkable(&self, x: usize, y: usize) -> bool {
        x < MAZE_SIZE && y < MAZE_SIZE && self.cells[y][x]
    }

    pub fn shortest_path_length(&self) -> Option<usize> {
        let mut distances = [[usize::MAX; MAZE_SIZE]; MAZE_SIZE];
        let mut queue = VecDeque::new();

        let (sx, sy) = self.entrance;
        distances[sy][sx] = 0;
        queue.push_back((sx, sy));

        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == self.exit {
                return Some(distances[y][x]);
            }

            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let nx = x as isize + dx;
                let ny = y as isize + dy;

                if nx < 0 || ny < 0 {
                    continue;
                }

                let (nx, ny) = (nx as usize, ny as usize);

                if self.is_walkable(nx, ny) && distances[ny][nx] == usize::MAX {
                    distances[ny][nx] = distances[y][x] + 1;
                    queue.push_back((nx, ny));
                }
            }
        }

        None
    }

    pub fn as_text(&self) -> String {
        let mut output = String::new();

        for y in 0..MAZE_SIZE {
            for x in 0..MAZE_SIZE {
                let symbol = if (x, y) == self.entrance {
                    'E'
                } else if (x, y) == self.exit {
                    'S'
                } else if self.cells[y][x] {
                    ' '
                } else {
                    '#'
                };
                output.push(symbol);
            }
            output.push('\n');
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use crate::planet_mazes;

    #[test]
    fn four_mazes_are_balanced() {
        for (maze, min, max) in [
            (planet_mazes::building::generate(), 36, 50),
            (planet_mazes::tessara::generate(), 50, 64),
            (planet_mazes::kharvex::generate(), 66, 80),
            (planet_mazes::nareth::generate(), 82, 110),
        ] {
            let length = maze.shortest_path_length().unwrap();
            assert!(
                (min..=max).contains(&length),
                "Camino fuera del rango: {length}"
            );
        }
    }

    #[test]
    fn planets_have_different_maps() {
        let a = planet_mazes::building::generate();
        let b = planet_mazes::tessara::generate();
        let c = planet_mazes::kharvex::generate();
        let d = planet_mazes::nareth::generate();

        assert_ne!(a.cells, b.cells);
        assert_ne!(a.cells, c.cells);
        assert_ne!(a.cells, d.cells);
        assert_ne!(b.cells, c.cells);
        assert_ne!(b.cells, d.cells);
        assert_ne!(c.cells, d.cells);
    }
}

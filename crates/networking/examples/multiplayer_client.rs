
use networking::{
    ClientMessage, GameClient, NetworkOrientation, NetworkVec3,
    PlayerTransform, RemotePlayers, ServerMessage,
};

use std::io::{self, BufRead, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn print_help() {
    println!();
    println!("=== Comandos disponibles ===");
    println!("  help                  Mostrar ayuda");
    println!("  players               Mostrar jugadores remotos");
    println!("  move <x> <y> <z>      Enviar dirección de movimiento");
    println!("  rotate <yaw> <pitch>  Cambiar orientación");
    println!("  ping                  Comprobar conexión");
    println!("  quit                  Salir de la partida");
    println!();
    println!("Ejemplo: move 1 0 0");
    println!("Ejemplo: rotate 90 15");
    println!();
}

fn parse_f32(value: &str) -> Result<f32, String> {
    let number = value
        .parse::<f32>()
        .map_err(|_| format!("Número inválido: {value}"))?;

    if !number.is_finite() {
        return Err("El número debe ser finito".to_string());
    }

    Ok(number)
}

fn parse_values(
    arguments: &[&str],
    expected: usize,
) -> Result<Vec<f32>, String> {
    if arguments.len() != expected {
        return Err(format!(
            "Se esperaban {expected} valores numéricos"
        ));
    }

    arguments.iter().map(|value| parse_f32(value)).collect()
}

fn print_players(players: &RemotePlayers) {
    println!();
    println!("=== Jugadores remotos ===");

    if players.is_empty() {
        println!("No hay otros jugadores conectados.");
        return;
    }

    for player in players.iter() {
        println!("Jugador {}: {}", player.id, player.name);

        if let Some(transform) = player.transform {
            println!(
                "  Posición: ({:.2}, {:.2}, {:.2})",
                transform.position.x,
                transform.position.y,
                transform.position.z
            );

            println!(
                "  Orientación: yaw={:.2}, pitch={:.2}",
                transform.orientation.yaw_degrees,
                transform.orientation.pitch_degrees
            );

            println!(
                "  Velocidad: ({:.2}, {:.2}, {:.2})",
                transform.velocity.x,
                transform.velocity.y,
                transform.velocity.z
            );
        } else {
            println!("  Esperando primer movimiento...");
        }
    }

    println!();
}

fn process_event(
    message: ServerMessage,
    players: &mut RemotePlayers,
) {
    match &message {
        ServerMessage::PlayerJoined { player_id, name } => {
            println!();
            println!("[+] Jugador conectado: {name} (ID {player_id})");
        }

        ServerMessage::PlayerLeft { player_id } => {
            println!();
            println!("[-] Jugador desconectado: ID {player_id}");
        }

        ServerMessage::PlayerMoved {
            player_id,
            sequence,
            transform,
        } => {
            println!();
            println!(
                "[Movimiento] Jugador {} | secuencia {}",
                player_id, sequence
            );

            println!(
                "  Posición: ({:.2}, {:.2}, {:.2})",
                transform.position.x,
                transform.position.y,
                transform.position.z
            );
        }

        ServerMessage::Pong => {
            println!("[Servidor] Pong recibido");
        }

        ServerMessage::Rejected { reason } => {
            println!("[Servidor] Mensaje rechazado: {reason}");
        }

        ServerMessage::Welcome {
            player_id,
            max_players,
        } => {
            println!(
                "[Servidor] Bienvenido, jugador {} (máximo {})",
                player_id, max_players
            );
        }
    }

    players.apply(message);
}

fn run(address: &str, name: &str) -> Result<(), String> {
    println!("=== Astral Echo Multiplayer ===");
    println!("Conectando a {address}...");

    let mut client = GameClient::connect(address, name)
        .map_err(|error| format!("No se pudo conectar: {error}"))?;

    println!("Conectado correctamente.");
    println!("Nombre: {name}");
    println!("ID asignado: {}", client.player_id());

    let mut players = RemotePlayers::new();

    let mut transform = PlayerTransform::new(
        NetworkVec3::new(0.0, 0.0, 0.0),
        NetworkOrientation::new(0.0, 0.0),
        NetworkVec3::new(0.0, 0.0, 0.0),
    );

    let mut sequence = 0_u64;

    let (sender, receiver) = mpsc::channel::<String>();

    // Leer comandos de teclado sin bloquear la recepción TCP.
    thread::spawn(move || {
        let stdin = io::stdin();

        for line in stdin.lock().lines() {
            match line {
                Ok(command) => {
                    if sender.send(command).is_err() {
                        break;
                    }
                }

                Err(_) => break,
            }
        }
    });

    print_help();

    loop {
        // Procesar eventos recibidos del servidor.
        loop {
            match client.try_receive() {
                Ok(Some(message)) => {
                    process_event(message, &mut players);
                }

                Ok(None) => break,

                Err(error) => {
                    return Err(format!(
                        "Se perdió la conexión con el servidor: {error}"
                    ));
                }
            }
        }

        // Procesar comandos ingresados por el jugador.
        match receiver.try_recv() {
            Ok(line) => {
                let parts: Vec<&str> = line.split_whitespace().collect();

                if parts.is_empty() {
                    continue;
                }

                match parts[0].to_lowercase().as_str() {
                    "help" => {
                        print_help();
                    }

                    "players" => {
                        print_players(&players);
                    }

                    "move" => {
                        let values = match parse_values(&parts[1..], 3) {
                            Ok(values) => values,
                            Err(error) => {
                                println!("{error}");
                                continue;
                            }
                        };

                        let velocity = NetworkVec3::new(
                            values[0],
                            values[1],
                            values[2],
                        );

                        let magnitude_squared =
                            velocity.x * velocity.x
                            + velocity.y * velocity.y
                            + velocity.z * velocity.z;

                        if !magnitude_squared.is_finite()
                            || magnitude_squared > 1.0001
                        {
                            println!(
                                "La dirección debe tener magnitud máxima de 1."
                            );
                            continue;
                        }

                        transform.velocity = velocity;

                        sequence = sequence
                            .checked_add(1)
                            .ok_or("Se agotaron las secuencias")?;

                        client
                            .send(&ClientMessage::PlayerMove {
                                sequence,
                                transform,
                            })
                            .map_err(|error| error.to_string())?;

                        println!(
                            "Movimiento enviado (secuencia {sequence})"
                        );
                    }

                    "rotate" => {
                        let values = match parse_values(&parts[1..], 2) {
                            Ok(values) => values,
                            Err(error) => {
                                println!("{error}");
                                continue;
                            }
                        };

                        let yaw = values[0];
                        let pitch = values[1];

                        if !(-360.0..=360.0).contains(&yaw)
                            || !(-360.0..=360.0).contains(&pitch)
                        {
                            println!(
                                "La orientación debe estar entre -360 y 360 grados."
                            );
                            continue;
                        }

                        transform.orientation =
                            NetworkOrientation::new(yaw, pitch);

                        println!(
                            "Orientación actualizada: yaw={yaw}, pitch={pitch}"
                        );
                        println!(
                            "Usa 'move' para enviar el cambio al servidor."
                        );
                    }

                    "ping" => {
                        client
                            .ping()
                            .map_err(|error| error.to_string())?;

                        println!("Conexión activa.");
                    }

                    "quit" | "exit" => {
                        println!("Desconectando...");

                        client
                            .disconnect()
                            .map_err(|error| error.to_string())?;

                        println!("Hasta pronto.");
                        return Ok(());
                    }

                    _ => {
                        println!("Comando desconocido. Usa 'help'.");
                    }
                }
            }

            Err(mpsc::TryRecvError::Empty) => {}

            Err(mpsc::TryRecvError::Disconnected) => {
                client
                    .disconnect()
                    .map_err(|error| error.to_string())?;

                return Ok(());
            }
        }

        io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;

        thread::sleep(POLL_INTERVAL);
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);

    let address = arguments.next();
    let name = arguments.next();

    if arguments.next().is_some()
        || address.is_none()
        || name.is_none()
    {
        eprintln!(
            "Uso: cargo run -p networking --example multiplayer_client -- <dirección> <nombre>"
        );

        std::process::exit(1);
    }

    if let Err(error) = run(
        address.as_deref().unwrap(),
        name.as_deref().unwrap(),
    ) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

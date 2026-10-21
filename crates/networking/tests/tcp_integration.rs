use networking::{
    ClientMessage, GameClient, GameServer, NetworkOrientation, NetworkVec3, PlayerTransform,
    ServerMessage, MAX_PLAYERS,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

// Iniciar un servidor TCP para las pruebas.
fn start_server() -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let server = GameServer::bind("127.0.0.1:0").expect("No se pudo iniciar el servidor");

    let address = server.local_addr().unwrap().to_string();
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&shutdown);

    let handle = thread::spawn(move || {
        server.run_until(&signal).unwrap();
    });

    (address, shutdown, handle)
}

// Detener el servidor al finalizar las pruebas.
fn stop_server(shutdown: Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
    shutdown.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}

// Crear datos de movimiento para las pruebas.
fn sample_movement() -> PlayerTransform {
    PlayerTransform::new(
        NetworkVec3::new(10.0, 2.0, 5.0),
        NetworkOrientation::new(90.0, 15.0),
        NetworkVec3::new(1.0, 0.0, 0.0),
    )
}

// Verificar que un cliente pueda conectarse al servidor.
#[test]
fn test_client_connection() {
    let (address, shutdown, handle) = start_server();

    let mut client = GameClient::connect(&address, "Jugador 1").unwrap();

    assert!(client.player_id() > 0);

    client.ping().unwrap();
    client.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que tres jugadores puedan conectarse simultáneamente.
#[test]
fn test_three_players() {
    let (address, shutdown, handle) = start_server();

    let mut clients = Vec::new();

    for i in 0..MAX_PLAYERS {
        let client = GameClient::connect(&address, &format!("Jugador {}", i + 1)).unwrap();

        clients.push(client);
    }

    let ids: Vec<_> = clients.iter().map(|client| client.player_id()).collect();

    assert_eq!(ids.len(), MAX_PLAYERS);

    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j]);
        }
    }

    for client in &mut clients {
        client.ping().unwrap();
        client.disconnect().unwrap();
    }

    stop_server(shutdown, handle);
}

// Verificar que el servidor rechace a un cuarto jugador.
#[test]
fn test_fourth_player_rejected() {
    let (address, shutdown, handle) = start_server();

    let mut clients = Vec::new();

    for i in 0..MAX_PLAYERS {
        clients.push(GameClient::connect(&address, &format!("Jugador {}", i + 1)).unwrap());
    }

    let fourth = GameClient::connect(&address, "Jugador 4");

    assert!(fourth.is_err());

    for client in &mut clients {
        client.disconnect().unwrap();
    }

    stop_server(shutdown, handle);
}

// Verificar que un jugador pueda salir y otro conectarse.
#[test]
fn test_player_reconnection() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();

    let first_id = first.player_id();
    first.disconnect().unwrap();

    thread::sleep(Duration::from_millis(50));

    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    assert_ne!(first_id, second.player_id());

    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que un jugador reciba la notificación
// cuando otro jugador entra a la partida.
#[test]
fn test_player_join_notification() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    let event = first.receive().unwrap();

    assert_eq!(
        event,
        ServerMessage::PlayerJoined {
            player_id: second.player_id(),
            name: "Jugador 2".to_string(),
        }
    );

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que un nuevo jugador conozca
// a los participantes que ya estaban conectados.
#[test]
fn test_existing_players_notification() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();
    let mut third = GameClient::connect(&address, "Jugador 3").unwrap();

    let first_event = third.receive().unwrap();
    let second_event = third.receive().unwrap();

    let expected = [
        ServerMessage::PlayerJoined {
            player_id: first.player_id(),
            name: "Jugador 1".to_string(),
        },
        ServerMessage::PlayerJoined {
            player_id: second.player_id(),
            name: "Jugador 2".to_string(),
        },
    ];

    assert!(expected.contains(&first_event));
    assert!(expected.contains(&second_event));
    assert_ne!(first_event, second_event);

    first.disconnect().unwrap();
    second.disconnect().unwrap();
    third.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar la notificación de salida voluntaria.
#[test]
fn test_player_leave_notification() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    assert!(matches!(
        first.receive().unwrap(),
        ServerMessage::PlayerJoined { .. }
    ));

    let second_id = second.player_id();
    second.disconnect().unwrap();

    assert_eq!(
        first.receive().unwrap(),
        ServerMessage::PlayerLeft {
            player_id: second_id,
        }
    );

    first.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar una desconexión inesperada.
#[test]
fn test_unexpected_disconnect() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();

    let second_id = second.player_id();

    drop(second);

    assert_eq!(
        first.receive().unwrap(),
        ServerMessage::PlayerLeft {
            player_id: second_id,
        }
    );

    first.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que el movimiento llegue a otro jugador.
#[test]
fn test_movement_broadcast() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 1,
            transform,
        })
        .unwrap();

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerJoined {
            player_id: first.player_id(),
            name: "Jugador 1".to_string(),
        }
    );

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 1,
            transform,
        }
    );

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que dos jugadores reciban el mismo movimiento.
#[test]
fn test_movement_reaches_two_players() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();
    let mut third = GameClient::connect(&address, "Jugador 3").unwrap();

    first.receive().unwrap();
    first.receive().unwrap();

    second.receive().unwrap();
    second.receive().unwrap();

    third.receive().unwrap();
    third.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 1,
            transform,
        })
        .unwrap();

    let expected = ServerMessage::PlayerMoved {
        player_id: first.player_id(),
        sequence: 1,
        transform,
    };

    assert_eq!(second.receive().unwrap(), expected);
    assert_eq!(third.receive().unwrap(), expected);

    first.disconnect().unwrap();
    second.disconnect().unwrap();
    third.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que el servidor ignore una actualización antigua.
#[test]
fn test_outdated_movement_is_ignored() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();
    second.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 5,
            transform,
        })
        .unwrap();

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 5,
            transform,
        }
    );

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 3,
            transform,
        })
        .unwrap();

    second.ping().unwrap();

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que el movimiento del emisor llegue al receptor,
// sin que el servidor lo reenvíe al jugador original.
#[test]
fn test_sender_does_not_receive_own_movement() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();
    second.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 1,
            transform,
        })
        .unwrap();

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 1,
            transform,
        }
    );

    // Sincronizar la respuesta del servidor.
    first.ping().unwrap();

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que Ping conserve los movimientos pendientes.
#[test]
fn test_ping_preserves_movement_events() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();
    second.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 1,
            transform,
        })
        .unwrap();

    // Confirmar que el servidor procesó el movimiento.
    first.ping().unwrap();

    // El cliente debe conservar el evento al recibir Pong.
    second.ping().unwrap();

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 1,
            transform,
        }
    );

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

// Verificar que los movimientos antiguos no se distribuyan.
#[test]
fn test_movement_sequence_ordering() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    first.receive().unwrap();
    second.receive().unwrap();

    let transform = sample_movement();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 10,
            transform,
        })
        .unwrap();

    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 10,
            transform,
        }
    );

    // Enviar primero un movimiento antiguo y después uno nuevo.
    first
        .send(&ClientMessage::PlayerMove {
            sequence: 5,
            transform,
        })
        .unwrap();

    first
        .send(&ClientMessage::PlayerMove {
            sequence: 11,
            transform,
        })
        .unwrap();

    // El siguiente evento debe corresponder al movimiento nuevo.
    assert_eq!(
        second.receive().unwrap(),
        ServerMessage::PlayerMoved {
            player_id: first.player_id(),
            sequence: 11,
            transform,
        }
    );

    first.disconnect().unwrap();
    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

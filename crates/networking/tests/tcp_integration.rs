use networking::{GameClient, GameServer, ServerMessage, MAX_PLAYERS};
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

    // Permitir que el servidor procese la desconexión.
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

// Verificar que se notifique cuando un jugador
// abandona voluntariamente la partida.
#[test]
fn test_player_leave_notification() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    // Consumir la notificación de entrada.
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

// Verificar que el servidor detecte una desconexión
// inesperada y notifique a los demás jugadores.
#[test]
fn test_unexpected_disconnect() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();
    let second = GameClient::connect(&address, "Jugador 2").unwrap();

    // Consumir la notificación de entrada.
    first.receive().unwrap();

    let second_id = second.player_id();

    // Cerrar la conexión sin enviar Leave.
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

use networking::{GameClient, GameServer, MAX_PLAYERS};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

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

fn stop_server(shutdown: Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
    shutdown.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}

#[test]
fn test_client_connection() {
    let (address, shutdown, handle) = start_server();

    let mut client = GameClient::connect(&address, "Jugador 1").unwrap();

    assert!(client.player_id() > 0);
    client.ping().unwrap();
    client.disconnect().unwrap();

    stop_server(shutdown, handle);
}

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

#[test]
fn test_player_reconnection() {
    let (address, shutdown, handle) = start_server();

    let mut first = GameClient::connect(&address, "Jugador 1").unwrap();

    let first_id = first.player_id();
    first.disconnect().unwrap();

    // Dar tiempo al servidor para procesar la desconexión.
    thread::sleep(Duration::from_millis(50));

    let mut second = GameClient::connect(&address, "Jugador 2").unwrap();

    assert_ne!(first_id, second.player_id());

    second.disconnect().unwrap();

    stop_server(shutdown, handle);
}

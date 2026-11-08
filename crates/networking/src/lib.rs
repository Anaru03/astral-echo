pub mod client;
pub mod protocol;
pub mod remote_players;
pub mod server;
pub mod sync;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    PlayerJoined {
        player_id: NetworkPlayerId,
        name: String,
    },
    PlayerMoved {
        player_id: NetworkPlayerId,
        sequence: u64,
        transform: PlayerTransform,
    },
    PlayerInteracted {
        player_id: NetworkPlayerId,
        request_id: u64,
        target: u64,
    },
    PlayerAttacked {
        player_id: NetworkPlayerId,
        request_id: u64,
        enemy: u64,
    },
    PlayerCollected {
        player_id: NetworkPlayerId,
        request_id: u64,
        collectible: u64,
        amount: u32,
    },
    PlayerEquippedWeapon {
        player_id: NetworkPlayerId,
        request_id: u64,
        item: u32,
    },
    PlayerUsedHealingItem {
        player_id: NetworkPlayerId,
        request_id: u64,
        item: u32,
        amount: u32,
    },
    PlayerRequestedTravel {
        player_id: NetworkPlayerId,
        request_id: u64,
        pilot_station: u64,
        destination: u32,
    },
    PlayerLeft {
        player_id: NetworkPlayerId,
    },
    MissionRecoveryRequested {
        player_id: NetworkPlayerId,
        request_id: u64,
    },
}

#[derive(Clone, Default)]
pub struct ServerEventQueue {
    events: Arc<Mutex<VecDeque<ServerEvent>>>,
}

impl ServerEventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pop(&self) -> Option<ServerEvent> {
        self.events.lock().ok()?.pop_front()
    }

    pub(crate) fn push(&self, event: ServerEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);
        }
    }
}

pub use client::GameClient;

pub use protocol::{
    decode_client, decode_server, encode, ClientMessage, NetworkPlayerId, ProtocolError,
    Mission5ContentProjection, Mission5DestinationProjection, Mission5ObjectiveProjection,
    Mission5DiscoveryProjection, Mission5ShooterProjection, Mission5DestinationTypeProjection,
    MissionRecoveryOutcome, MissionStateProjection, ObjectiveStateProjection, ServerMessage,
    EnemyProjection, InventoryItemProjection, SharedInventoryProjection,
    MAX_PLAYERS, PROTOCOL_VERSION,
};

pub use remote_players::{RemotePlayer, RemotePlayers};

pub use server::{GameServer, GameServerHandle};

pub use sync::{NetworkOrientation, NetworkVec3, PlayerTransform};

pub mod protocol;

pub use protocol::{
    decode_client, decode_server, encode, ClientMessage, NetworkPlayerId, ProtocolError,
    ServerMessage, MAX_PLAYERS, PROTOCOL_VERSION,
};

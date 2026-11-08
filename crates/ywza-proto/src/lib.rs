//! ywza-proto: wire types, framing, config and iroh helpers shared by every ywza client.

mod config;
mod error;
mod frame;
mod msg;
mod net;
pub mod pair;
mod paths;
mod relay_trust;

pub use config::{Config, Desktop, Llm, Ocr, OcrEngine, Peer, Permissions, Stream};
pub use error::Error;
pub use frame::{read_frame, write_frame, MAX_FRAME};
pub use msg::{
    AgentMsg, ClientMsg, FrameHeader, InputEvent, MouseButton, SessionEvent, SessionId,
    SessionStatus, SessionSummary, Target, ToolInfo,
};
pub use net::{accept_control, bind, connect, dns_resolver, relay_mode, Control, Ticket, ALPN};
pub use pair::{agent_side, client_side, new_code, rendezvous_key, PairInfo, PAIR_ALPN};
pub use paths::{config_dir, load_or_create_secret_key};
pub use relay_trust::{fingerprint, shareable_relay, split_pin, with_relay};

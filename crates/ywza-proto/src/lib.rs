//! ywza-proto: wire types, framing, config and iroh helpers shared by every ywza client.

pub mod client;
mod config;
mod error;
mod frame;
mod log;
mod msg;
mod net;
pub mod pair;
mod paths;
mod relay_trust;
pub mod xfer;

pub use config::{
    glob, Action, Approval, ApprovalMode, Config, Desktop, Llm, Ocr, OcrEngine, Peer, Permissions,
    Rule, Stream,
};
pub use error::Error;
pub use frame::{read_frame, write_frame, MAX_FRAME};
#[cfg(feature = "log")]
pub use log::init as init_log;
pub use log::{Format, LogConfig};
pub use msg::{
    AgentMsg, ClientMsg, ElementInfo, FileHeader, FrameHeader, InputEvent, MonitorInfo,
    MouseButton, ScreenRect, SessionEvent, SessionId, SessionStatus, SessionSummary, Target,
    ToolInfo,
};
pub use net::{accept_control, bind, connect, dns_resolver, relay_mode, Control, Ticket, ALPN};
pub use pair::{agent_side, client_side, new_code, rendezvous_key, PairInfo, PAIR_ALPN};
pub use paths::{config_dir, load_or_create_secret_key};
pub use relay_trust::{fingerprint, shareable_relay, split_pin, with_relay};

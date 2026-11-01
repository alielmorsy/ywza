mod agent;
mod client;
mod session;
mod stream;

pub use agent::{AgentMsg, ToolInfo};
pub use client::ClientMsg;
pub use session::{SessionEvent, SessionId, SessionStatus, SessionSummary};
pub use stream::{FrameHeader, InputEvent, MouseButton, Target};

#[cfg(test)]
pub(crate) mod testutil {
    use serde::{de::DeserializeOwned, Serialize};
    use serde_json::Value;

    use crate::SessionId;

    /// Serializes, checks the exact JSON, and deserializes back.
    pub fn check<T>(v: T, expect: Value)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let got = serde_json::to_value(&v).unwrap();
        assert_eq!(got, expect);
        assert_eq!(serde_json::from_value::<T>(got).unwrap(), v);
    }

    pub fn sid() -> SessionId {
        SessionId(uuid::Uuid::from_u128(1))
    }

    pub const SID: &str = "00000000-0000-0000-0000-000000000001";
}

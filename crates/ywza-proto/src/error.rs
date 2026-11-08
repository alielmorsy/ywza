/// Errors from framing, key files, tickets, pairing and the iroh transport.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("frame of {len} bytes exceeds the {max} byte limit", max = crate::MAX_FRAME)]
    FrameTooLarge { len: usize },
    /// The peer closed the stream cleanly between frames.
    #[error("stream closed")]
    Closed,
    #[error("transport: {0}")]
    Transport(Box<dyn std::error::Error + Send + Sync>),
    #[error("bad relay url `{0}`: {1}")]
    RelayUrl(String, String),
    #[error("bad ticket: {0}")]
    Ticket(String),
    #[error("wrong pairing code")]
    BadCode,
    #[error("pairing failed: {0}")]
    Pair(String),
}

impl Error {
    pub(crate) fn transport(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Transport(Box::new(e))
    }
}

use crate::{Image, TextLine};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    /// Windows engine on Windows (escalating to Paddle), Paddle elsewhere.
    Auto,
    Windows,
    Paddle,
}

pub trait Recognizer: Send + Sync {
    /// Blocking.
    fn recognize(&self, img: &Image) -> Result<Vec<TextLine>>;
}

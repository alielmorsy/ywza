//! ywza-stream: live view encoding. Captures a monitor whenever it changes and encodes
//! H.264 (Annex-B) with openh264.

mod ctl;
mod encoder;
mod scale;
mod session;

pub use ctl::{Ctl, DEFAULT_BITRATE};
pub use encoder::{Encoded, Streamer, KEYFRAME_EVERY};
pub use scale::{resize_bgra, target_size, MAX_WIDTH};
pub use session::run;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

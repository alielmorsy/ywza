use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const DEFAULT_BITRATE: u32 = 4_000_000;

/// Shared knobs between the capture thread and whoever consumes its frames.
pub struct Ctl {
    pub stop: AtomicBool,
    keyframe: AtomicBool,
    bitrate: AtomicU32,
}

impl Default for Ctl {
    fn default() -> Self {
        Self {
            stop: AtomicBool::new(false),
            keyframe: AtomicBool::new(false),
            bitrate: AtomicU32::new(DEFAULT_BITRATE),
        }
    }
}

impl Ctl {
    pub fn request_keyframe(&self) {
        self.keyframe.store(true, Ordering::Relaxed);
    }
    pub fn bitrate(&self) -> u32 {
        self.bitrate.load(Ordering::Relaxed)
    }
    pub fn set_bitrate(&self, bps: u32) {
        self.bitrate.store(bps, Ordering::Relaxed);
    }
    pub(crate) fn take_keyframe(&self) -> bool {
        self.keyframe.swap(false, Ordering::Relaxed)
    }
}

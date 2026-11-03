use std::time::{Duration, Instant};

use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, FrameType, RateControlMode, UsageType,
};
use openh264::formats::{BgraSliceU8, YUVBuffer};
use openh264::{OpenH264API, Timestamp};

use crate::scale::{resize_bgra, target_size};
use crate::Result;

pub const KEYFRAME_EVERY: Duration = Duration::from_secs(2);

/// One encoded frame.
#[derive(Debug, Clone)]
pub struct Encoded {
    /// Annex-B NAL units (start codes included); keyframes carry SPS/PPS.
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub width: u32,
    pub height: u32,
    /// Microseconds since the encoder started.
    pub pts_us: u64,
}

/// BGRA frames in, H.264 out. Skips frames identical to the previous one.
pub struct Streamer {
    encoder: Option<Encoder>,
    bitrate: u32,
    fps: u32,
    prev: Vec<u8>,
    prev_size: (u32, u32),
    scaled: Vec<u8>,
    start: Instant,
    last_key: Option<Instant>,
    /// The encoder dropped the latest frame for rate control; [`Streamer::flush`] retries it.
    skipped: bool,
}

impl Streamer {
    pub fn new(bitrate: u32, fps: u32) -> Self {
        Self {
            encoder: None,
            bitrate,
            fps: fps.max(1),
            prev: Vec::new(),
            prev_size: (0, 0),
            scaled: Vec::new(),
            start: Instant::now(),
            last_key: None,
            skipped: false,
        }
    }

    /// Takes effect on the next frame (the encoder restarts, so that frame is a keyframe).
    pub fn set_bitrate(&mut self, bps: u32) {
        if bps != self.bitrate {
            self.bitrate = bps;
            self.encoder = None;
        }
    }

    /// Encodes `bgra` (tightly packed, `w`x`h`) unless it equals the previous frame and
    /// no keyframe is forced. Returns None for skipped frames.
    pub fn push(
        &mut self,
        bgra: &[u8],
        w: u32,
        h: u32,
        force_key: bool,
    ) -> Result<Option<Encoded>> {
        if !force_key && (w, h) == self.prev_size && bgra == self.prev.as_slice() {
            return Ok(None);
        }
        self.prev.clear();
        self.prev.extend_from_slice(bgra);
        self.prev_size = (w, h);
        self.encode_prev(force_key)
    }

    /// Re-encodes the last frame as a keyframe (a viewer needs one while the screen is static).
    pub fn repeat_keyframe(&mut self) -> Result<Option<Encoded>> {
        if self.prev.is_empty() {
            return Ok(None);
        }
        self.encode_prev(true)
    }

    /// Retries a frame the rate control skipped, so a screen that just went static still
    /// ends up showing its final state.
    pub fn flush(&mut self) -> Result<Option<Encoded>> {
        if !self.skipped {
            return Ok(None);
        }
        self.encode_prev(false)
    }

    fn encode_prev(&mut self, force_key: bool) -> Result<Option<Encoded>> {
        let (w, h) = self.prev_size;
        let (tw, th) = target_size(w, h);
        let src: &[u8] = if (tw, th) == (w, h) {
            &self.prev
        } else {
            resize_bgra(&self.prev, w, h, tw, th, &mut self.scaled);
            &self.scaled
        };
        let yuv = YUVBuffer::from_bgra8_source(BgraSliceU8::new(src, (tw as usize, th as usize)));
        let encoder = match &mut self.encoder {
            Some(e) => e,
            None => {
                let cfg = EncoderConfig::new()
                    .bitrate(BitRate::from_bps(self.bitrate))
                    .max_frame_rate(FrameRate::from_hz(self.fps as f32))
                    .rate_control_mode(RateControlMode::Bitrate)
                    .usage_type(UsageType::ScreenContentRealTime)
                    .adaptive_quantization(false)
                    .background_detection(false)
                    .skip_frames(true);
                self.encoder
                    .insert(Encoder::with_api_config(OpenH264API::from_source(), cfg)?)
            }
        };
        let now = Instant::now();
        if force_key || self.last_key.is_none_or(|t| now - t >= KEYFRAME_EVERY) {
            encoder.force_intra_frame();
        }
        let pts = now - self.start;
        let bits = encoder.encode_at(&yuv, Timestamp::from_millis(pts.as_millis() as u64))?;
        let keyframe = matches!(bits.frame_type(), FrameType::IDR | FrameType::I);
        let data = bits.to_vec();
        self.skipped = data.is_empty();
        if self.skipped {
            return Ok(None);
        }
        if keyframe {
            self.last_key = Some(now);
        }
        Ok(Some(Encoded {
            data,
            keyframe,
            width: tw,
            height: th,
            pts_us: pts.as_micros() as u64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_BITRATE;

    fn frame(w: u32, h: u32, v: u8) -> Vec<u8> {
        (0..w * h * 4)
            .map(|i| (i as u8).wrapping_mul(7) ^ v)
            .collect()
    }

    #[test]
    fn static_frames_are_skipped() {
        let mut s = Streamer::new(DEFAULT_BITRATE, 30);
        let a = frame(64, 48, 0);
        let first = s
            .push(&a, 64, 48, false)
            .unwrap()
            .expect("first frame encodes");
        assert!(first.keyframe);
        assert_eq!(&first.data[..4], &[0, 0, 0, 1]);
        assert_eq!((first.width, first.height), (64, 48));
        assert!(
            s.push(&a, 64, 48, false).unwrap().is_none(),
            "unchanged frame sends nothing"
        );
        let p = s
            .push(&frame(64, 48, 1), 64, 48, false)
            .unwrap()
            .expect("changed frame encodes");
        assert!(!p.keyframe);
        assert!(
            s.push(&frame(64, 48, 1), 64, 48, true)
                .unwrap()
                .unwrap()
                .keyframe,
            "forced keyframe"
        );
        assert!(s.repeat_keyframe().unwrap().unwrap().keyframe);
    }

    #[test]
    fn odd_width_goes_through_resizer() {
        let mut s = Streamer::new(DEFAULT_BITRATE, 30);
        let e = s.push(&frame(33, 20, 0), 33, 20, false).unwrap().unwrap();
        assert_eq!((e.width, e.height), (32, 20));
    }
}

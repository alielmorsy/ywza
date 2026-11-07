//! Delay-based bitrate control. Pure logic: the caller supplies every timestamp.

use std::time::{Duration, Instant};

pub const MIN_BITRATE: u32 = 500_000;
pub const MAX_BITRATE: u32 = 15_000_000;
const HIGH_DELAY: f64 = 0.150;
const LOW_DELAY: f64 = 0.040;
const DECREASE_GAP: Duration = Duration::from_secs(1);
const INCREASE_GAP: Duration = Duration::from_secs(2);
const CALM: Duration = Duration::from_secs(2);
const DRIFT_EVERY: Duration = Duration::from_secs(10);
const ALPHA: f64 = 0.2;

pub struct RateCtl {
    bitrate: u32,
    /// Minimum P-frame ack latency, drifting up so route changes are tracked.
    base: Option<Duration>,
    recent_min: Option<Duration>,
    drift_at: Instant,
    /// EWMA of queueing delay (latency above `base`) in seconds.
    delay: f64,
    calm_since: Option<Instant>,
    last_dec: Option<Instant>,
    last_inc: Option<Instant>,
}

impl RateCtl {
    pub fn new(bitrate: u32, now: Instant) -> Self {
        Self {
            bitrate,
            base: None,
            recent_min: None,
            drift_at: now,
            delay: 0.0,
            calm_since: None,
            last_dec: None,
            last_inc: None,
        }
    }

    pub fn bitrate(&self) -> u32 {
        self.bitrate
    }

    pub fn base(&self) -> Option<Duration> {
        self.base
    }

    /// Congestion with no usable samples (acks stopped arriving): back off like high delay.
    pub fn on_stall(&mut self, now: Instant) {
        self.calm_since = None;
        if due(self.last_dec, now, DECREASE_GAP) {
            self.bitrate = ((self.bitrate as f64 * 0.75) as u32).max(MIN_BITRATE);
            self.last_dec = Some(now);
        }
    }

    /// Keyframes are large by nature and say nothing about congestion.
    pub fn on_ack(&mut self, now: Instant, latency: Duration, keyframe: bool) {
        if keyframe {
            return;
        }
        let base = self.base.map_or(latency, |b| b.min(latency));
        let recent = self.recent_min.map_or(latency, |m| m.min(latency));
        self.base = Some(base);
        self.recent_min = Some(recent);
        if now.duration_since(self.drift_at) >= DRIFT_EVERY {
            self.base = Some(base + recent.saturating_sub(base).mul_f64(0.1));
            self.recent_min = None;
            self.drift_at = now;
        }
        let sample = latency.saturating_sub(self.base.unwrap_or(latency));
        self.delay += ALPHA * (sample.as_secs_f64() - self.delay);

        if self.delay > HIGH_DELAY {
            self.on_stall(now);
        } else if self.delay < LOW_DELAY {
            let since = *self.calm_since.get_or_insert(now);
            if now.duration_since(since) >= CALM && due(self.last_inc, now, INCREASE_GAP) {
                self.bitrate = ((self.bitrate as f64 * 1.15) as u32).min(MAX_BITRATE);
                self.last_inc = Some(now);
            }
        } else {
            self.calm_since = None;
        }
    }
}

fn due(last: Option<Instant>, now: Instant, gap: Duration) -> bool {
    last.is_none_or(|t| now.duration_since(t) >= gap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: Duration = Duration::from_millis(33);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Feeds one ack per frame interval for `secs`; returns every (time, bitrate) change.
    fn feed(
        c: &mut RateCtl,
        t0: Instant,
        secs: u64,
        lat: Duration,
        key: bool,
    ) -> Vec<(Duration, u32)> {
        let mut changes = Vec::new();
        let mut t = Duration::ZERO;
        while t < Duration::from_secs(secs) {
            let before = c.bitrate();
            c.on_ack(t0 + t, lat, key);
            if c.bitrate() != before {
                changes.push((t, c.bitrate()));
            }
            t += STEP;
        }
        changes
    }

    #[test]
    fn low_delay_ramps_to_cap() {
        let t0 = Instant::now();
        let mut c = RateCtl::new(6_000_000, t0);
        let ch = feed(&mut c, t0, 60, ms(100), false);
        assert_eq!(c.bitrate(), MAX_BITRATE);
        assert!(ch.windows(2).all(|w| w[1].0 - w[0].0 >= INCREASE_GAP));
    }

    #[test]
    fn high_delay_lowers_to_floor_once_per_second() {
        let t0 = Instant::now();
        let mut c = RateCtl::new(6_000_000, t0);
        c.on_ack(t0, ms(20), false);
        let ch = feed(&mut c, t0, 30, ms(400), false);
        assert_eq!(c.bitrate(), MIN_BITRATE);
        assert!(ch.iter().all(|&(_, b)| b >= MIN_BITRATE));
        assert!(ch.windows(2).all(|w| w[1].0 - w[0].0 >= DECREASE_GAP));
    }

    #[test]
    fn stall_backs_off_with_gap_and_floor() {
        let t0 = Instant::now();
        let mut c = RateCtl::new(1_000_000, t0);
        c.on_stall(t0);
        assert_eq!(c.bitrate(), 750_000);
        c.on_stall(t0 + ms(500));
        assert_eq!(c.bitrate(), 750_000);
        c.on_stall(t0 + ms(1000));
        assert_eq!(c.bitrate(), 562_500);
        c.on_stall(t0 + ms(2000));
        assert_eq!(c.bitrate(), MIN_BITRATE);
        c.on_stall(t0 + ms(3000));
        assert_eq!(c.bitrate(), MIN_BITRATE);
    }

    #[test]
    fn keyframes_are_ignored() {
        let t0 = Instant::now();
        let mut c = RateCtl::new(1_000_000, t0);
        c.on_ack(t0, ms(20), false);
        feed(&mut c, t0, 10, ms(2000), true);
        assert_eq!(c.bitrate(), 1_000_000);
        assert_eq!(c.base(), Some(ms(20)));
    }

    #[test]
    fn base_tracks_minimum_then_drifts() {
        let t0 = Instant::now();
        let mut c = RateCtl::new(6_000_000, t0);
        for l in [50, 30, 80] {
            c.on_ack(t0, ms(l), false);
        }
        assert_eq!(c.base(), Some(ms(30)));
        c.on_ack(t0 + Duration::from_secs(11), ms(100), false);
        c.on_ack(t0 + Duration::from_secs(22), ms(100), false);
        assert_eq!(c.base(), Some(ms(37)));
    }
}

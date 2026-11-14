use std::collections::VecDeque;

const HOUR_MS: u64 = 3_600_000;

/// Fires on a false to true change. The first reading is only a baseline, so a condition
/// that already holds when the watcher starts does not fire.
#[derive(Default)]
pub struct Edge {
    primed: bool,
    prev: bool,
}

impl Edge {
    pub fn rise(&mut self, now: bool) -> bool {
        let fire = self.primed && now && !self.prev;
        self.primed = true;
        self.prev = now;
        fire
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    Fire,
    /// Within `min_gap_s` of the last fire.
    Skip,
    /// Over `max_per_hour`: the watcher must be turned off.
    Disable,
}

/// `recent`: this run's fire times in ms (kept by the caller); `last_ms`: the persisted last fire.
pub fn gate(
    now_ms: u64,
    last_ms: Option<u64>,
    recent: &mut VecDeque<u64>,
    min_gap_s: u32,
    max_per_hour: u32,
) -> Gate {
    if last_ms.is_some_and(|l| now_ms < l.saturating_add(u64::from(min_gap_s) * 1000)) {
        return Gate::Skip;
    }
    while recent.front().is_some_and(|&t| now_ms.saturating_sub(t) >= HOUR_MS) {
        recent.pop_front();
    }
    if recent.len() >= max_per_hour as usize {
        return Gate::Disable;
    }
    recent.push_back(now_ms);
    Gate::Fire
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_fires_only_on_rise_and_never_on_the_baseline() {
        let mut e = Edge::default();
        assert!(!e.rise(true));
        assert!(!e.rise(true));
        assert!(!e.rise(false));
        assert!(e.rise(true));
        assert!(!e.rise(true));
        assert!(!e.rise(false));
        assert!(e.rise(true));
        let mut e = Edge::default();
        assert!(!e.rise(false));
        assert!(e.rise(true));
    }

    #[test]
    fn min_gap_skips_and_hourly_cap_disables() {
        let mut recent = VecDeque::new();
        assert_eq!(gate(0, None, &mut recent, 60, 2), Gate::Fire);
        assert_eq!(gate(30_000, Some(0), &mut recent, 60, 2), Gate::Skip);
        assert_eq!(gate(60_000, Some(0), &mut recent, 60, 2), Gate::Fire);
        assert_eq!(gate(120_000, Some(60_000), &mut recent, 60, 2), Gate::Disable);
        // The first fire leaves the hour window.
        assert_eq!(gate(3_600_000, Some(60_000), &mut recent, 60, 2), Gate::Fire);
    }
}

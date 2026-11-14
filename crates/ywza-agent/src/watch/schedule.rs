use chrono::{Days, Duration, NaiveDateTime, NaiveTime};

/// "HH:MM", 24 h.
pub fn parse_at(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// The next fire after `now`: `every_min` minutes later, else the next `at` (today or tomorrow).
pub fn next_after(
    every_min: Option<u32>,
    at: Option<NaiveTime>,
    now: NaiveDateTime,
) -> Option<NaiveDateTime> {
    match (every_min, at) {
        (Some(m), _) => now.checked_add_signed(Duration::minutes(m.into())),
        (None, Some(t)) => {
            let today = now.date().and_time(t);
            if today > now {
                Some(today)
            } else {
                today.checked_add_days(Days::new(1))
            }
        }
        _ => None,
    }
}

/// True once `now` reaches `next` and moves it on. The first call only sets the next fire, so
/// starting a watcher (or the agent) never fires at once.
pub fn due(
    next: &mut Option<NaiveDateTime>,
    every_min: Option<u32>,
    at: Option<NaiveTime>,
    now: NaiveDateTime,
) -> bool {
    match *next {
        Some(n) if now >= n => {
            *next = next_after(every_min, at, now);
            true
        }
        Some(_) => false,
        None => {
            *next = next_after(every_min, at, now);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn t(d: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, d)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    #[test]
    fn at_parses_hh_mm_only() {
        assert_eq!(parse_at("09:30"), NaiveTime::from_hms_opt(9, 30, 0));
        assert_eq!(parse_at(" 23:59 "), NaiveTime::from_hms_opt(23, 59, 0));
        assert_eq!(parse_at("24:00"), None);
        assert_eq!(parse_at("noon"), None);
    }

    #[test]
    fn next_fire_is_today_or_tomorrow() {
        let at = NaiveTime::from_hms_opt(9, 30, 0);
        assert_eq!(next_after(None, at, t(1, 8, 0, 0)), Some(t(1, 9, 30, 0)));
        assert_eq!(next_after(None, at, t(1, 9, 30, 0)), Some(t(2, 9, 30, 0)));
        assert_eq!(next_after(None, at, t(1, 10, 0, 0)), Some(t(2, 9, 30, 0)));
        assert_eq!(next_after(Some(5), None, t(1, 10, 0, 0)), Some(t(1, 10, 5, 0)));
        assert_eq!(next_after(None, None, t(1, 10, 0, 0)), None);
    }

    #[test]
    fn due_arms_first_then_fires_and_advances() {
        let mut next = None;
        assert!(!due(&mut next, Some(10), None, t(1, 8, 0, 0)));
        assert_eq!(next, Some(t(1, 8, 10, 0)));
        assert!(!due(&mut next, Some(10), None, t(1, 8, 9, 59)));
        assert!(due(&mut next, Some(10), None, t(1, 8, 10, 0)));
        assert_eq!(next, Some(t(1, 8, 20, 0)));
        // A long sleep fires once, not once per missed slot.
        assert!(due(&mut next, Some(10), None, t(1, 12, 0, 0)));
        assert!(!due(&mut next, Some(10), None, t(1, 12, 1, 0)));
    }
}

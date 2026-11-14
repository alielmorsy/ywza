use super::store::Entries;

/// Shorter values would mangle ordinary text.
pub const MIN_LEN: usize = 4;

/// Values worth hiding, longest first so a value that contains another one wins.
pub fn secrets(entries: &Entries) -> Vec<(&str, &str)> {
    let mut v: Vec<_> = entries
        .iter()
        .filter(|(_, v)| v.len() >= MIN_LEN)
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    v.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    v
}

/// Replaces every vault value in `text` with its `{{vault:key}}` placeholder, in one pass.
pub fn scrub_text(text: &str, entries: &Entries) -> String {
    let secrets = secrets(entries);
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if let Some((key, value)) = secrets.iter().find(|(_, v)| rest.starts_with(v)) {
            out.push_str(super::substitute::OPEN);
            out.push_str(key);
            out.push_str("}}");
            i += value.len();
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_occurrence_is_replaced() {
        let e = Entries::from([("gh".into(), "hunter22".into())]);
        assert_eq!(
            scrub_text("a hunter22 b\nhunter22hunter22", &e),
            "a {{vault:gh}} b\n{{vault:gh}}{{vault:gh}}"
        );
    }

    #[test]
    fn the_longer_of_overlapping_values_wins() {
        let e = Entries::from([("short".into(), "abcd".into()), ("long".into(), "abcd1234".into())]);
        assert_eq!(scrub_text("x abcd1234 abcd", &e), "x {{vault:long}} {{vault:short}}");
    }

    #[test]
    fn short_values_and_placeholder_names_are_left_alone() {
        let e = Entries::from([("p".into(), "123".into()), ("v".into(), "vault".into())]);
        assert_eq!(scrub_text("pin 123", &e), "pin 123");
        assert_eq!(scrub_text("{{vault:x}} é", &Entries::new()), "{{vault:x}} é");
        // A value equal to placeholder text never gets re-scanned into mangled output.
        assert_eq!(scrub_text("vault", &e), "{{vault:v}}");
    }
}

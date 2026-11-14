//! Step guards: a needle must appear in a text, ignoring case and runs of whitespace.
//! `expect` checks an element's name, `changes` a step's reported changes.

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn contains(text: &str, want: &str) -> bool {
    norm(text).contains(&norm(want))
}

#[cfg(test)]
mod tests {
    use super::contains;

    #[test]
    fn ignores_case_and_spacing() {
        assert!(contains("Save  As...", "save as"));
        assert!(!contains("Open", "save"));
    }
}

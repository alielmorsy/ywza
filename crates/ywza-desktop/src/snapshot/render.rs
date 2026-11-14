use super::diff::{same_shape, Delta};
use super::model::{Element, Snapshot};
use std::fmt::Write;

/// Above this many removed (or added) elements, that side is summarised.
const COLLAPSE: usize = 20;
const SAMPLE_REMOVED: usize = 8;
const SAMPLE_ADDED: usize = 12;

/// Text form of a diff: `+[20] dialog "Save as"`, `-[12]`, `~[3] = "new value"`.
/// A page change (many removed or added) starts with a count line and lists a sample.
pub fn render_diff(deltas: &[Delta]) -> String {
    let removed: Vec<String> = deltas
        .iter()
        .filter_map(|d| match d {
            Delta::Removed(o) => Some(format!("-[{}]", o.id)),
            _ => None,
        })
        .collect();
    let r = removed.len();
    let a = deltas
        .iter()
        .filter(|d| matches!(d, Delta::Added(_)))
        .count();
    let mut out = String::new();
    if r > COLLAPSE || a > COLLAPSE {
        let _ = writeln!(
            out,
            "{r} removed, {a} added (sample; get_elements lists all)"
        );
    }
    if r > COLLAPSE {
        let _ = writeln!(out, "{} …", removed[..SAMPLE_REMOVED].join(" "));
    } else {
        for l in &removed {
            let _ = writeln!(out, "{l}");
        }
    }
    let mut shown = 0;
    for d in deltas {
        let _ = match d {
            Delta::Removed(_) => continue,
            Delta::Added(n) => {
                shown += 1;
                if a > COLLAPSE && shown > SAMPLE_ADDED {
                    continue;
                }
                writeln!(out, "+{}", n.line())
            }
            Delta::Changed { old, new } if same_shape(old, new) => {
                writeln!(out, "~[{}]{}", new.id, new.value_text())
            }
            Delta::Changed { new, .. } => writeln!(out, "~{}", new.line()),
        };
    }
    if a > COLLAPSE {
        let _ = writeln!(out, "+… {} more added", a - SAMPLE_ADDED);
    }
    if out.is_empty() {
        out.push_str("(no changes)\n");
    }
    out
}

impl Snapshot {
    /// The compact text form the AI reads.
    pub fn render(&self) -> String {
        self.render_where(|_| true)
    }

    /// Only the web page: elements over the largest `document` (skipping browser
    /// toolbars, tabs and bookmarks) and open popups. The full list when there is no document.
    pub fn render_page(&self) -> String {
        let doc = self
            .elements
            .iter()
            .filter(|e| e.kind == "document" && !e.popup)
            .max_by_key(|e| e.rect.area());
        match doc.map(|d| d.rect) {
            Some(page) => self.render_where(|e| e.popup || !page.intersect(&e.rect).is_empty()),
            None => self.render(),
        }
    }

    fn render_where(&self, keep: impl Fn(&Element) -> bool) -> String {
        let w = &self.window;
        let mut out = format!(
            "window w{} {:?} ({}) {}x{} mon {} snapshot {}\n",
            w.id, w.title, w.process, w.rect.w, w.rect.h, w.monitor, self.seq
        );
        for e in self.elements.iter().filter(|e| keep(e)) {
            let _ = writeln!(out, "{}", e.line());
        }
        out
    }
}

impl Element {
    /// `[3] edit "Name" = "report.txt"`
    pub fn line(&self) -> String {
        let mut s = format!("[{}] {}", self.id, self.kind);
        if !self.name.is_empty() {
            let _ = write!(s, " {:?}", self.name);
            if self.learned {
                s += " (learned)";
            }
        } else if !self.automation_id.is_empty() {
            let _ = write!(s, " (id={})", self.automation_id);
        }
        if !self.group.is_empty() {
            let _ = write!(s, " in {:?}", self.group);
        }
        s += &self.value_text();
        if self.selected {
            s += " selected";
        }
        if self.busy {
            s += " busy";
        }
        // Has a dropdown/menu besides its main action (split buttons): `click` with
        // `expand` opens it.
        if self.patterns.expand && self.kind != "expandable" {
            s += " +menu";
        }
        if !self.enabled {
            s += " disabled";
        }
        s
    }

    pub(super) fn value_text(&self) -> String {
        match (&self.value, self.patterns.toggle) {
            (Some(v), true) => format!(" = {v}"),
            (Some(v), false) => format!(" = {v:?}"),
            (None, _) => String::new(),
        }
    }
}

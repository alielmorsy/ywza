use super::model::{Element, Snapshot};
use std::fmt::Write;

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

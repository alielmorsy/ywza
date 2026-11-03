use crate::{Host, TermId};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Owned,
    Attached(Host),
}

/// One row of the unified terminal listing.
#[derive(Debug, Clone)]
pub struct TermInfo {
    pub id: TermId,
    pub kind: Kind,
    pub name: String,
    pub pid: Option<u32>,
    pub cwd: Option<String>,
    pub title: String,
    /// Owned: "running" / "exited N". Attached: error text when not attachable.
    pub status: Option<String>,
    pub hwnd: Option<isize>,
}

impl fmt::Display for TermInfo {
    /// `[t2] pwsh (Windows Terminal tab) cwd title`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.kind {
            Kind::Owned => "owned",
            Kind::Attached(Host::WindowsTerminal) => "Windows Terminal tab",
            Kind::Attached(Host::Conhost) => "console window",
            Kind::Attached(Host::Other) => "console",
        };
        write!(f, "[{}] {} ({}", self.id, self.name, kind)?;
        if let Some(s) = &self.status {
            write!(f, ", {s}")?;
        }
        f.write_str(")")?;
        if let Some(c) = &self.cwd {
            write!(f, " {c}")?;
        }
        if !self.title.is_empty() {
            write!(f, " \"{}\"", self.title)?;
        }
        Ok(())
    }
}

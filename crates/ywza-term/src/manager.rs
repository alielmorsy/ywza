use crate::attach::{attached_keys, attached_read, attached_write, list_attachable, Attachable};
use crate::{err, Kind, OwnedTerm, Result, TermId, TermInfo};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

enum Entry {
    Owned(Arc<OwnedTerm>),
    Attached(Attachable),
}

#[derive(Default)]
struct Inner {
    next: u32,
    terms: BTreeMap<u32, Entry>,
}

#[derive(Default)]
pub struct TermManager {
    inner: Mutex<Inner>,
}

impl TermManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn entry(&self, id: TermId) -> Result<EntryRef> {
        match self.inner().terms.get(&id.0) {
            Some(Entry::Owned(t)) => Ok(EntryRef::Owned(t.clone())),
            Some(Entry::Attached(a)) => Ok(EntryRef::Attached(a.pid)),
            None => Err(err(format!("unknown terminal {id}; call list first"))),
        }
    }

    /// Start `command` (run through the platform shell) in a new owned PTY terminal.
    pub fn start(&self, command: &str, cwd: Option<&Path>) -> Result<TermId> {
        let t = Arc::new(OwnedTerm::start(command, cwd)?);
        let mut g = self.inner();
        g.next += 1;
        let id = g.next;
        g.terms.insert(id, Entry::Owned(t));
        Ok(TermId(id))
    }

    pub fn owned(&self, id: TermId) -> Result<Arc<OwnedTerm>> {
        match self.entry(id)? {
            EntryRef::Owned(t) => Ok(t),
            EntryRef::Attached(_) => Err(err(format!(
                "{id} is an attached terminal, not an owned one"
            ))),
        }
    }

    /// Owned terminals plus attachable consoles, merged. Attachable ids are assigned on first
    /// sight and kept while the console stays alive.
    pub fn list(&self) -> Vec<TermInfo> {
        let found = list_attachable().unwrap_or_default();
        let mut g = self.inner();
        let owned_pids: Vec<u32> = g
            .terms
            .values()
            .filter_map(|e| match e {
                Entry::Owned(t) => t.pid(),
                _ => None,
            })
            .collect();
        let found: Vec<Attachable> = found
            .into_iter()
            .filter(|a| {
                !owned_pids.contains(&a.pid) && !a.ancestors.iter().any(|p| owned_pids.contains(p))
            })
            .collect();
        g.terms.retain(|_, e| match e {
            Entry::Attached(a) => found.iter().any(|f| f.pid == a.pid),
            _ => true,
        });
        for a in found {
            let known = g.terms.iter_mut().find_map(|(_, e)| match e {
                Entry::Attached(old) if old.pid == a.pid => Some(old),
                _ => None,
            });
            match known {
                Some(old) => *old = a,
                None => {
                    g.next += 1;
                    let id = g.next;
                    g.terms.insert(id, Entry::Attached(a));
                }
            }
        }
        g.terms
            .iter()
            .map(|(&id, e)| match e {
                Entry::Owned(t) => TermInfo {
                    id: TermId(id),
                    kind: Kind::Owned,
                    name: t.command.clone(),
                    pid: t.pid(),
                    cwd: t.cwd.as_ref().map(|c| c.display().to_string()),
                    title: t.title(),
                    status: Some(match t.exit_status() {
                        None => "running".to_string(),
                        Some(c) => format!("exited {c}"),
                    }),
                    hwnd: None,
                },
                Entry::Attached(a) => TermInfo {
                    id: TermId(id),
                    kind: Kind::Attached(a.host),
                    name: a.name.clone(),
                    pid: Some(a.pid),
                    cwd: None,
                    title: a.title.clone(),
                    status: a.error.as_ref().map(|e| format!("not attachable: {e}")),
                    hwnd: a.hwnd,
                },
            })
            .collect()
    }

    /// The listing as text, one line per terminal.
    pub fn list_text(&self) -> String {
        self.list()
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn read(&self, id: TermId, lines: usize) -> Result<String> {
        match self.entry(id)? {
            EntryRef::Owned(t) => Ok(t.read(lines)),
            EntryRef::Attached(pid) => attached_read(pid, lines),
        }
    }

    pub fn write(&self, id: TermId, text: &str, enter: bool) -> Result<()> {
        match self.entry(id)? {
            EntryRef::Owned(t) => t.write(text, enter),
            EntryRef::Attached(pid) => attached_write(pid, text, enter),
        }
    }

    pub fn keys(&self, id: TermId, keys: &str) -> Result<()> {
        match self.entry(id)? {
            EntryRef::Owned(t) => t.keys(keys),
            EntryRef::Attached(pid) => attached_keys(pid, keys),
        }
    }

    /// Kill an owned terminal's process. Attached consoles are never killed.
    pub fn kill(&self, id: TermId) -> Result<()> {
        self.owned(id)?.kill()
    }

    /// Forget an owned terminal (kills it if still running).
    pub fn remove(&self, id: TermId) -> Result<()> {
        self.owned(id)?;
        self.inner().terms.remove(&id.0);
        Ok(())
    }
}

enum EntryRef {
    Owned(Arc<OwnedTerm>),
    Attached(u32),
}

use super::model::{Raw, Snapshot};
use crate::Rect;

const MAX_DEPTH: usize = 32;
const MAX_NAME: usize = 48;
const MAX_CANDIDATES: usize = 8;
const TOLERANCE: i32 = 10;
/// Candidates below this score are not returned.
const MIN_SCORE: f32 = 0.5;
/// The top two candidates closer than this are ambiguous.
const MARGIN: f32 = 0.1;
/// Score weights: automation id, name, ancestor path, position. Terms that cannot
/// discriminate (anchor without id or name) are left out and the rest renormalized.
const W_ID: f32 = 0.35;
const W_NAME: f32 = 0.25;
const W_PATH: f32 = 0.25;
const W_POS: f32 = 0.15;

/// One ancestor of an element, as far as it can be recognized again.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Step {
    pub role: &'static str,
    pub automation_id: String,
    pub name: String,
}

/// Durable fingerprint of an element: survives an app restart, unlike its key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Anchor {
    /// Window process name.
    pub process: String,
    /// Raw role.
    pub role: &'static str,
    pub automation_id: String,
    /// Trimmed, whitespace collapsed, lowercase.
    pub name: String,
    /// Ancestors from the root down (listed or not); the nearest 32 at most.
    pub path: Vec<Step>,
    /// Rect relative to the window (or popup bounds) origin.
    pub rel: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: u32,
    /// 0..=1, higher is a better match.
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resolve {
    /// Best first; the top one is clearly ahead.
    Found(Vec<Candidate>),
    /// The top candidates are too close to pick one.
    Ambiguous(Vec<Candidate>),
    NotFound,
}

pub(super) fn norm(s: &str) -> String {
    cap(&s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase())
}

pub(super) fn rel(rect: &Rect, origin: &Rect) -> Rect {
    Rect::new(rect.x - origin.x, rect.y - origin.y, rect.w, rect.h)
}

fn cap(s: &str) -> String {
    s.chars().take(MAX_NAME).collect()
}

/// True when `b` (in window `b_win`) sits where `a` (in `a_win`) did, relative to
/// its window, so a moved window does not invalidate everything.
pub(super) fn same_place(a: &Rect, a_win: &Rect, b: &Rect, b_win: &Rect) -> bool {
    ((b.x - b_win.x) - (a.x - a_win.x)).abs() <= TOLERANCE
        && ((b.y - b_win.y) - (a.y - a_win.y)).abs() <= TOLERANCE
        && (b.w - a.w).abs() <= TOLERANCE
        && (b.h - a.h).abs() <= TOLERANCE
}

/// Ancestor links of a walker's output, for paths and listed-parent lookups.
pub(super) struct Tree {
    /// Parent index per node; always smaller than the node's own index.
    parents: Vec<Option<usize>>,
    steps: Vec<Step>,
}

impl Tree {
    pub(super) fn new<'a>(raws: impl Iterator<Item = &'a Raw>) -> Tree {
        let (mut parents, mut steps) = (Vec::new(), Vec::new());
        for (i, r) in raws.enumerate() {
            parents.push(r.parent.filter(|&p| p < i));
            steps.push(Step {
                role: r.role,
                automation_id: cap(&r.automation_id),
                name: norm(&r.name),
            });
        }
        Tree { parents, steps }
    }

    pub(super) fn parent(&self, i: usize) -> Option<usize> {
        self.parents[i]
    }

    pub(super) fn path(&self, i: usize) -> Vec<Step> {
        let mut out = Vec::new();
        let mut at = i;
        while let Some(p) = self.parents[at] {
            if out.len() == MAX_DEPTH {
                break;
            }
            out.push(self.steps[p].clone());
            at = p;
        }
        out.reverse();
        out
    }
}

/// Path similarity: steps matching from the nearest ancestor upward, over the longer path.
fn path_score(a: &[Step], b: &[Step]) -> f32 {
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let same = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    same as f32 / longest as f32
}

fn score(a: &Anchor, c: &Anchor) -> f32 {
    let origin = Rect::default();
    let mut got = W_PATH * path_score(&a.path, &c.path);
    let mut total = W_PATH + W_POS;
    if same_place(&a.rel, &origin, &c.rel, &origin) {
        got += W_POS;
    }
    if !a.automation_id.is_empty() {
        total += W_ID;
        if a.automation_id == c.automation_id {
            got += W_ID;
        }
    }
    if !a.name.is_empty() {
        total += W_NAME;
        if a.name == c.name {
            got += W_NAME;
        }
    }
    (got / total).min(1.0)
}

/// Finds the element `anchor` describes in `snap`: same process and role, then ranked
/// by id, name, ancestor path and position.
pub fn resolve(anchor: &Anchor, snap: &Snapshot) -> Resolve {
    if !snap.window.process.eq_ignore_ascii_case(&anchor.process) {
        return Resolve::NotFound;
    }
    let mut found: Vec<Candidate> = snap
        .elements
        .iter()
        .filter(|e| e.anchor.role == anchor.role)
        .map(|e| Candidate {
            id: e.id,
            score: score(anchor, &e.anchor),
        })
        .filter(|c| c.score >= MIN_SCORE)
        .collect();
    found.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
    found.truncate(MAX_CANDIDATES);
    if found.is_empty() {
        Resolve::NotFound
    } else if found.len() > 1 && found[0].score - found[1].score < MARGIN {
        Resolve::Ambiguous(found)
    } else {
        Resolve::Found(found)
    }
}

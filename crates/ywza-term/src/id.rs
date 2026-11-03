use crate::{err, Error, Result};
use std::fmt;
use std::str::FromStr;

/// Terminal handle shown to the AI as `t2`. Stable for the life of a `TermManager`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermId(pub u32);

impl fmt::Display for TermId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

impl FromStr for TermId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let n = s.trim().trim_start_matches(['t', 'T']);
        n.parse()
            .map(TermId)
            .map_err(|_| err(format!("bad terminal id '{s}'")))
    }
}

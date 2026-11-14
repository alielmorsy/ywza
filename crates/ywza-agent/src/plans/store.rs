//! One JSON file per plan in `<dir>/plans`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

use super::{dir, Plan};
use crate::atomic::write_atomic;

/// Letters, digits, `-`, `_` and `.`, not starting with a dot: safe as a file name everywhere.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn path(name: &str) -> anyhow::Result<PathBuf> {
    if !valid_name(name) {
        bail!("bad plan name `{name}` (letters, digits, - _ . only)");
    }
    Ok(dir()?.join(format!("{name}.json")))
}

fn read(p: &Path) -> anyhow::Result<Plan> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
}

pub fn load(name: &str) -> anyhow::Result<Plan> {
    let p = path(name)?;
    if !p.exists() {
        bail!("no plan `{name}`");
    }
    read(&p)
}

pub fn save(plan: &Plan) -> anyhow::Result<()> {
    let p = path(&plan.name)?;
    std::fs::create_dir_all(dir()?)?;
    write_atomic(&p, &serde_json::to_string_pretty(plan)?)?;
    Ok(())
}

pub fn delete(name: &str) -> anyhow::Result<()> {
    let p = path(name)?;
    if !p.exists() {
        bail!("no plan `{name}`");
    }
    std::fs::remove_file(p)?;
    Ok(())
}

/// Every readable plan, sorted by name; broken files are skipped with a warning.
pub fn list() -> Vec<Plan> {
    let Ok(rd) = dir().and_then(|d| Ok(std::fs::read_dir(d)?)) else {
        return Vec::new();
    };
    let mut plans: Vec<Plan> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| {
            read(&p)
                .map_err(|e| tracing::warn!(error = %format!("{e:#}"), "skipping a plan"))
                .ok()
        })
        .collect();
    plans.sort_by(|a, b| a.name.cmp(&b.name));
    plans
}

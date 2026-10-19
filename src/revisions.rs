//! A task's revisions: `base`, then `fix1`, `fix2`, ... with no gap.

use crate::diff_records::DiffDocument;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn validate_revision_name(name: &str) -> Result<u32, String> {
    if name == "base" {
        return Ok(0);
    }
    let n = name.strip_prefix("fix").ok_or_else(|| "revision must be base or fixN".to_string())?;
    let value: u32 = n.parse().map_err(|_| "fix revision requires numeric suffix".to_string())?;
    if value == 0 {
        return Err("fix0 is not canonical".into());
    }
    Ok(value)
}

/// The name of revision `ordinal`: 0 is `base`, n is `fixn`.
pub fn revision_name(ordinal: u32) -> String {
    if ordinal == 0 {
        "base".into()
    } else {
        format!("fix{ordinal}")
    }
}

/// The revisions a task directory holds, by ordinal.
pub fn revisions_in(task_dir: &Path) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = std::fs::read_dir(task_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().to_str().map(str::to_string)).filter_map(|n| validate_revision_name(&n).ok().map(|o| (o, n))).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// The revision a re-run writes: `base` for a new task, else the one after the last.
pub fn next_revision(task_dir: &Path) -> String {
    revision_name(revisions_in(task_dir).last().map(|(o, _)| o + 1).unwrap_or(0))
}

/// The last revision a task holds, if any.
pub fn latest_revision(task_dir: &Path) -> Option<String> {
    revisions_in(task_dir).pop().map(|(_, n)| n)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionHistory {
    pub name: String,
    pub ordinal: u32,
    pub path: PathBuf,
    pub documents: Vec<DiffDocument>,
}

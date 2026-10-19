//! Change records: a file a task added, modified, removed or renamed, with
//! the digests of its bytes before and after.

use crate::diff_records::{digest_hex, DiffDocument, DiffRecordKind};
use crate::TaskHistoryIndex;
use std::path::Path;

/// sha256 of a file's bytes, or `none` when there is no file.
pub fn file_digest(path: &Path) -> String {
    std::fs::read(path).map(|b| digest_hex(&b)).unwrap_or_else(|_| "none".to_string())
}

/// A change record of `relative` (under `tree_root`) from `before_digest` to
/// the file as it is now: `add` when it had none, `remove` when it is gone,
/// else `modify`.
pub fn change_record(tree_root: &Path, relative: &str, before_digest: &str) -> DiffDocument {
    let after = file_digest(&tree_root.join(relative));
    let operation = match (before_digest, after.as_str()) {
        ("none", _) => "add",
        (_, "none") => "remove",
        _ => "modify",
    };
    DiffDocument::record(DiffRecordKind::Change).with("path", relative).with("operation", operation).with("before_digest", before_digest).with("after_digest", after)
}

pub fn find_changes_for_path<'a>(index: &'a TaskHistoryIndex, path: &str) -> Vec<&'a DiffDocument> {
    index.tasks.iter().flat_map(|t| &t.revisions).flat_map(|r| &r.documents).filter(|d| d.kind().ok() == Some(DiffRecordKind::Change) && d.one("path").ok() == Some(path)).collect()
}

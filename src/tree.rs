//! The tree a history describes: the last change of every path must hold in it.

use crate::diff_records::{digest_hex, DiffDocument, DiffRecordKind};
use crate::TaskHistoryIndex;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn verify_terminal_tree_changes(index: &TaskHistoryIndex, tree_root: impl AsRef<Path>) -> Result<usize, String> {
    let tree_root = tree_root.as_ref();
    let mut terminal: BTreeMap<String, &DiffDocument> = BTreeMap::new();
    for task in &index.tasks {
        for revision in &task.revisions {
            for doc in &revision.documents {
                if doc.kind()? == DiffRecordKind::Change {
                    terminal.insert(doc.one("path")?.to_string(), doc);
                }
            }
        }
    }
    let mut checked = 0;
    for (relative, doc) in terminal {
        let path = tree_root.join(&relative);
        let operation = doc.one("operation")?;
        let after = doc.one("after_digest")?;
        match operation {
            "remove" => {
                if path.exists() {
                    return Err(format!("terminal removed path exists: {}", path.display()));
                }
            }
            "add" | "modify" | "rename" => {
                if !path.is_file() {
                    return Err(format!("terminal historical path missing: {}", path.display()));
                }
                let actual = digest_hex(&fs::read(&path).map_err(|e| e.to_string())?);
                if after == "none" || actual != after {
                    return Err(format!("terminal after digest mismatch for {}", path.display()));
                }
            }
            _ => return Err("bad operation".into()),
        }
        checked += 1;
    }
    Ok(checked)
}

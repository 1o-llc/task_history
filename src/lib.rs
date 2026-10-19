#![forbid(unsafe_code)]
//! Typed 1o task-history records.
//!
//! A task's accepted revisions are folders (`base`, `fix1`, `fix2`, ...). Every
//! entry is one `v1.<identity>.1o.diff` file: a task, its intent, a problem, a
//! diagnosis, a solution, each change (a path, its operation, the digests of
//! its bytes before and after), a verification, a result, and the history's
//! index. The history is descriptive engineering state, never fact or proof
//! authority.
//!
//! - [`diff_records`]: one record, its file name, kind, validation and text.
//! - [`changes`]: change records and file digests.
//! - [`revisions`]: `base`/`fixN` lineage.
//! - [`tree`]: the tree a history describes, checked against its last changes.
//!
//! [`load_task_history`] reads a history root; [`write_record`] adds a record
//! with the next sequence; [`write_index`] refreshes the root's index.

pub mod changes;
pub mod diff_records;
pub mod revisions;
pub mod tree;

pub use changes::{change_record, file_digest, find_changes_for_path};
pub use diff_records::{parse_one_o_diff_filename, parse_one_o_diff_text, validate_diff_document, DiffDocument, DiffRecordKind, OneODiffFilename, ONE_O_DIFF_MODIFIER, ONE_O_DIFF_SCHEMA};
pub use revisions::{latest_revision, next_revision, revision_name, validate_revision_name, RevisionHistory};
pub use tree::verify_terminal_tree_changes;

use diff_records::digest_hex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const TASK_HISTORY_SCHEMA: &str = "nodeplan.task_history.9p.v1";

/// The core-1o schema of the `diff` modifier, declared once here.
pub const DIFF_SCHEMA_JSON: &str = include_str!("../schema/diff/v1.schema.json");

/// The index file a history root carries.
pub const INDEX_FILE: &str = "v1.history_index.1o.diff";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHistory {
    pub task_directory: String,
    pub path: PathBuf,
    pub revisions: Vec<RevisionHistory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHistoryIndex {
    pub schema: String,
    pub tasks: Vec<TaskHistory>,
    pub task_count: usize,
    pub revision_count: usize,
    pub document_count: usize,
    pub change_count: usize,
    pub accepted_revision_count: usize,
    pub index_digest: String,
}

fn task_directory_ok(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

/// One revision folder's records, in sequence order; a non-1o file, a
/// record of another revision or a repeated sequence is refused.
fn read_revision(task_directory: &str, rp: &Path, name: &str) -> Result<Vec<DiffDocument>, String> {
    let mut docs = Vec::new();
    for f in fs::read_dir(rp).map_err(|e| e.to_string())? {
        let fp = f.map_err(|e| e.to_string())?.path();
        if !fp.is_file() {
            continue;
        }
        let fname = fp.file_name().and_then(|n| n.to_str()).ok_or("non-utf8 history filename")?;
        if !fname.ends_with(".1o.diff") {
            return Err(format!("non-1o file inside revision: {fname}"));
        }
        let text = fs::read_to_string(&fp).map_err(|e| e.to_string())?;
        let doc = parse_one_o_diff_text(fname, &text)?;
        if doc.one("revision")? != name {
            return Err(format!("revision mismatch in {fname}"));
        }
        docs.push(doc);
    }
    docs.sort_by_key(|d| d.one("sequence").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(u64::MAX));
    let mut seq = BTreeSet::new();
    for d in &docs {
        let n: u64 = d.one("sequence")?.parse().map_err(|_| "bad sequence")?;
        if !seq.insert(n) {
            return Err(format!("duplicate sequence {n} in {task_directory}/{name}"));
        }
    }
    Ok(docs)
}

pub fn load_task_history(root: impl AsRef<Path>) -> Result<TaskHistoryIndex, String> {
    let root = root.as_ref();
    if !root.exists() {
        return Err("history root does not exist".into());
    }
    let mut task_paths = Vec::new();
    for e in fs::read_dir(root).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            task_paths.push(p);
        }
    }
    task_paths.sort();
    let mut tasks = Vec::new();
    let mut document_count = 0;
    let mut change_count = 0;
    let mut accepted = 0;
    for path in task_paths {
        let task_directory = path.file_name().and_then(|n| n.to_str()).ok_or("non-utf8 task directory")?.to_string();
        if !task_directory_ok(&task_directory) {
            return Err(format!("noncanonical task directory {task_directory}"));
        }
        let mut revisions = Vec::new();
        for e in fs::read_dir(&path).map_err(|e| e.to_string())? {
            let rp = e.map_err(|e| e.to_string())?.path();
            if !rp.is_dir() {
                continue;
            }
            let name = rp.file_name().and_then(|n| n.to_str()).ok_or("non-utf8 revision")?.to_string();
            let ordinal = validate_revision_name(&name)?;
            let docs = read_revision(&task_directory, &rp, &name)?;
            for d in &docs {
                if d.kind()? == DiffRecordKind::Change {
                    change_count += 1;
                }
                if d.kind()? == DiffRecordKind::Result && d.one("status")? == "accepted" {
                    accepted += 1;
                }
            }
            document_count += docs.len();
            revisions.push(RevisionHistory { name, ordinal, path: rp, documents: docs });
        }
        revisions.sort_by_key(|r| r.ordinal);
        for (expected, r) in revisions.iter().enumerate() {
            if r.ordinal != expected as u32 {
                return Err(format!("noncontiguous revision lineage in {task_directory}: expected ordinal {expected}, found {}", r.ordinal));
            }
        }
        tasks.push(TaskHistory { task_directory, path, revisions });
    }
    let revision_count = tasks.iter().map(|t| t.revisions.len()).sum();
    let canonical = tasks
        .iter()
        .flat_map(|t| t.revisions.iter().flat_map(move |r| r.documents.iter().map(move |d| format!("{}/{}/{}:{}", t.task_directory, r.name, d.filename.identity, d.document_digest))))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(TaskHistoryIndex { schema: TASK_HISTORY_SCHEMA.into(), task_count: tasks.len(), revision_count, document_count, change_count, accepted_revision_count: accepted, index_digest: digest_hex(canonical.as_bytes()), tasks })
}

pub fn find_records_by_kind(index: &TaskHistoryIndex, kind: DiffRecordKind) -> Vec<&DiffDocument> {
    index.tasks.iter().flat_map(|t| &t.revisions).flat_map(|r| &r.documents).filter(|d| d.kind().ok() == Some(kind)).collect()
}

/// Writes `doc` into `<root>/<task_id>/<revision>/` with the next sequence of
/// that revision, its file named `v1.<kind>.1o.diff` (a change always
/// `v1.change.NNNNNN.1o.diff`, any other kind's second and later
/// `v1.<kind>.NNNNNN.1o.diff`). The record is validated, written whole (a new
/// file only, never over another), and read back. Returns it as read.
pub fn write_record(root: impl AsRef<Path>, task_id: &str, revision: &str, doc: DiffDocument) -> Result<DiffDocument, String> {
    write_record_in(root.as_ref().join(task_id), task_id, revision, doc)
}

/// [`write_record`] into a task directory named otherwise (`000003_task_history_9p`).
pub fn write_record_in(task_dir: impl AsRef<Path>, task_id: &str, revision: &str, mut doc: DiffDocument) -> Result<DiffDocument, String> {
    let task_dir = task_dir.as_ref();
    let dir_name = task_dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !task_directory_ok(dir_name) {
        return Err(format!("noncanonical task directory {dir_name}"));
    }
    validate_revision_name(revision)?;
    let kind = doc.kind()?;
    let dir = task_dir.join(revision);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let existing = read_revision(dir_name, &dir, revision)?;
    let sequence = existing.iter().filter_map(|d| d.one("sequence").ok()?.parse::<u64>().ok()).max().unwrap_or(0) + 1;
    let of_kind = existing.iter().filter(|d| d.kind().ok() == Some(kind)).count();
    let identity = if kind == DiffRecordKind::Change || of_kind > 0 { format!("{}.{:06}", kind.as_str(), of_kind + 1) } else { kind.as_str().to_string() };
    doc.set("schema", ONE_O_DIFF_SCHEMA);
    doc.set("task_id", task_id);
    doc.set("revision", revision);
    doc.set("sequence", sequence.to_string());
    let name = format!("v1.{identity}.1o.diff");
    let text = doc.render();
    let read = parse_one_o_diff_text(&name, &text)?;
    let path = dir.join(&name);
    write_new(&path, text.as_bytes())?;
    Ok(read)
}

/// A new file, written whole: refused when the name is taken.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_file_name(format!(".{}.{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("record"), std::process::id()));
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    f.write_all(bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    f.sync_all().map_err(|e| format!("{}: {e}", tmp.display()))?;
    drop(f);
    if path.exists() {
        return Err(format!("{}: a record of that name is already written", path.display()));
    }
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Refreshes `<root>/v1.history_index.1o.diff` from what the root holds
/// (task_count, revision_count). Returns the index record.
pub fn write_index(root: impl AsRef<Path>) -> Result<DiffDocument, String> {
    let root = root.as_ref();
    let index = load_task_history(root)?;
    let mut doc = DiffDocument::record(DiffRecordKind::Index).with("task_count", index.task_count.to_string()).with("revision_count", index.revision_count.to_string());
    doc.set("schema", ONE_O_DIFF_SCHEMA);
    doc.set("task_id", "history_index");
    doc.set("revision", "base");
    doc.set("sequence", "1");
    let text = doc.render();
    let read = parse_one_o_diff_text(INDEX_FILE, &text)?;
    let path = root.join(INDEX_FILE);
    let tmp = root.join(format!(".{INDEX_FILE}.{}.tmp", std::process::id()));
    fs::write(&tmp, text.as_bytes()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(read)
}

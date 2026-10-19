//! The writer: records written with the next sequence read back through the
//! loader; a change step's records hold against the tree; the digests are
//! sha2's; the diff schema as core-1o reads it.

use sha2::{Digest, Sha256};
use std::fs;
use task_history::*;

#[test]
fn stable_digest_gives_sha2s_hex_byte_for_byte() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/history");
    let mut inputs: Vec<Vec<u8>> = vec![b"".to_vec(), b"abc".to_vec(), vec![0u8; 4096]];
    for e in fs::read_dir(fixture.join("000003_task_history_9p/base")).unwrap().flatten().take(20) {
        inputs.push(fs::read(e.path()).unwrap());
    }
    for bytes in inputs {
        let theirs = format!("{:x}", Sha256::digest(&bytes));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        fs::write(&p, &bytes).unwrap();
        assert_eq!(file_digest(&p), theirs);
    }
    // and the document digests the loader gives are the same (every fixture record)
    let i = load_task_history(&fixture).unwrap();
    for t in &i.tasks {
        for r in &t.revisions {
            for d in &r.documents {
                let text = fs::read(r.path.join(format!("v1.{}.1o.diff", d.filename.identity))).unwrap();
                assert_eq!(d.document_digest, format!("{:x}", Sha256::digest(&text)));
            }
        }
    }
}

fn task(root: &std::path::Path) {
    write_record(root, "demo_task", "base", DiffDocument::record(DiffRecordKind::Task).with("title", "Demo \"quoted\" title").with("parent_milestone", "custodian").with("status", "candidate").with("tag", "a").with("tag", "b")).unwrap();
    write_record(root, "demo_task", "base", DiffDocument::record(DiffRecordKind::Intent).with("intent", "line one\nline two")).unwrap();
}

#[test]
fn written_records_take_the_next_sequence_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    task(dir.path());
    let s1 = write_record(dir.path(), "demo_task", "base", DiffDocument::record(DiffRecordKind::Solution).with("solution", "step one")).unwrap();
    let s2 = write_record(dir.path(), "demo_task", "base", DiffDocument::record(DiffRecordKind::Solution).with("solution", "step two")).unwrap();
    assert_eq!((s1.one("sequence").unwrap(), s2.one("sequence").unwrap()), ("3", "4"));
    assert_eq!((s1.filename.identity.as_str(), s2.filename.identity.as_str()), ("solution", "solution.000002"));
    let i = load_task_history(dir.path()).unwrap();
    assert_eq!((i.task_count, i.revision_count, i.document_count), (1, 1, 4));
    let docs = &i.tasks[0].revisions[0].documents;
    assert_eq!(docs[0].one("title").unwrap(), "Demo \"quoted\" title");
    assert_eq!(docs[0].many("tag"), ["a", "b"]);
    assert_eq!(docs[1].one("intent").unwrap(), "line one\nline two");
    // an invalid record is refused before anything is written
    assert!(write_record(dir.path(), "demo_task", "base", DiffDocument::record(DiffRecordKind::Verification).with("check", "t").with("outcome", "maybe")).is_err());
    assert_eq!(load_task_history(dir.path()).unwrap().document_count, 4);
}

#[test]
fn revisions_follow_base_then_fix_and_the_index_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    let task_dir = dir.path().join("demo_task");
    assert_eq!(next_revision(&task_dir), "base");
    task(dir.path());
    assert_eq!(next_revision(&task_dir), "fix1");
    write_record(dir.path(), "demo_task", "fix1", DiffDocument::record(DiffRecordKind::Intent).with("intent", "again")).unwrap();
    assert_eq!(latest_revision(&task_dir).as_deref(), Some("fix1"));
    assert_eq!(next_revision(&task_dir), "fix2");
    let idx = write_index(dir.path()).unwrap();
    assert_eq!((idx.one("task_count").unwrap(), idx.one("revision_count").unwrap()), ("1", "2"));
    assert!(dir.path().join(INDEX_FILE).is_file());
    // the index file at the root is not a task
    assert_eq!(load_task_history(dir.path()).unwrap().task_count, 1);
}

#[test]
fn a_change_steps_records_hold_against_the_tree() {
    let tree = tempfile::tempdir().unwrap();
    let hist = tempfile::tempdir().unwrap();
    fs::write(tree.path().join("kept.rs"), "fn a() {}\n").unwrap();
    fs::write(tree.path().join("gone.rs"), "fn b() {}\n").unwrap();
    let before_kept = file_digest(&tree.path().join("kept.rs"));
    let before_gone = file_digest(&tree.path().join("gone.rs"));
    task(hist.path());
    // the step: kept.rs modified, new.rs added, gone.rs moved aside (renamed, not deleted)
    fs::write(tree.path().join("kept.rs"), "fn a() { 1; }\n").unwrap();
    fs::write(tree.path().join("new.rs"), "fn c() {}\n").unwrap();
    fs::rename(tree.path().join("gone.rs"), tree.path().join("gone.rs.old")).unwrap();
    for (path, before) in [("kept.rs", before_kept.as_str()), ("new.rs", "none"), ("gone.rs", before_gone.as_str())] {
        write_record(hist.path(), "demo_task", "base", change_record(tree.path(), path, before)).unwrap();
    }
    let i = load_task_history(hist.path()).unwrap();
    assert_eq!(i.change_count, 3);
    let ops: Vec<&str> = find_records_by_kind(&i, DiffRecordKind::Change).iter().map(|d| d.one("operation").unwrap()).collect();
    assert_eq!(ops, vec!["modify", "add", "remove"]);
    assert_eq!(verify_terminal_tree_changes(&i, tree.path()).unwrap(), 3);
    // a later edit the history does not record breaks it
    fs::write(tree.path().join("kept.rs"), "fn a() { 2; }\n").unwrap();
    assert!(verify_terminal_tree_changes(&i, tree.path()).is_err());
}

#[test]
fn the_diff_schema_as_core_1o_reads_it() {
    let schema: serde_json::Value = serde_json::from_str(DIFF_SCHEMA_JSON).unwrap();
    let props = &schema["properties"]["diff"]["properties"];
    let kinds: Vec<&str> = props["record_kind"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(kinds, DiffRecordKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>());
    for k in DiffRecordKind::ALL {
        for f in k.required() {
            assert!(props.get(*f).is_some(), "{f} of {} is declared", k.as_str());
        }
    }
    // a record wrapped as core-1o reads a document ({ diff { ... } }) validates against it
    let rec = DiffDocument::record(DiffRecordKind::Verification).with("check", "tests").with("outcome", "pass");
    let dir = tempfile::tempdir().unwrap();
    task(dir.path());
    let written = write_record(dir.path(), "demo_task", "base", rec).unwrap();
    let text = written.render();
    let wrapped = format!("{{\n{text}}}\n");
    let nodes = core_1o::engine::Parser::new(core_1o::engine::Lexer::new(&wrapped).tokenize()).parse().unwrap();
    let mut diags = Vec::new();
    core_1o::engine::validate_ast(&nodes, &schema, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    let bad = wrapped.replace("\"pass\"", "\"maybe\"");
    let nodes = core_1o::engine::Parser::new(core_1o::engine::Lexer::new(&bad).tokenize()).parse().unwrap();
    let mut diags = Vec::new();
    core_1o::engine::validate_ast(&nodes, &schema, &mut diags);
    assert!(diags.iter().any(|d| d.message.contains("allowed enum values")), "{diags:?}");
    // the file as it is written (diff { ... }, no outer block) is not a document core-1o's parser reads
    let nodes = core_1o::engine::Parser::new(core_1o::engine::Lexer::new(&text).tokenize()).parse().unwrap();
    assert!(nodes.is_empty(), "core-1o reads a top-level block only");
}

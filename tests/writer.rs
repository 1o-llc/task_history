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
    // the file as it is written (diff { ... }, the record at the top level) is a document core-1o 0.2 reads and
    // validates; the older wrapped form ({ diff { ... } }) reads the same
    let rec = DiffDocument::record(DiffRecordKind::Verification).with("check", "tests").with("outcome", "pass");
    let dir = tempfile::tempdir().unwrap();
    task(dir.path());
    let written = write_record(dir.path(), "demo_task", "base", rec).unwrap();
    let text = written.render();
    let validate = |t: &str| {
        let nodes = core_1o::engine::Parser::new(core_1o::engine::Lexer::new(t).tokenize()).parse().unwrap();
        let mut diags = Vec::new();
        core_1o::engine::validate_ast(&nodes, &schema, &mut diags);
        (nodes, diags.into_iter().map(|d| d.message).collect::<Vec<_>>())
    };
    let (bare, found) = validate(&text);
    assert!(found.is_empty(), "{found:?}");
    let (wrapped, found) = validate(&format!("{{\n{text}}}\n"));
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(bare.len(), 1);
    assert_eq!(bare[0].key, wrapped[0].key);
    let (_, found) = validate(&text.replace("\"pass\"", "\"maybe\""));
    assert!(found.iter().any(|m| m.contains("allowed enum values")), "{found:?}");
    // each kind's keys are the schema's to say (x-1o-required-when), as DiffRecordKind::required says them
    let when = &schema["properties"]["diff"]["x-1o-required-when"]["record_kind"];
    for k in DiffRecordKind::ALL {
        let said: Vec<&str> = when[k.as_str()].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(said, k.required(), "{}", k.as_str());
    }
    let (_, found) = validate(&text.replace("  outcome \"pass\"\n", ""));
    assert_eq!(found, vec!["Missing required key 'outcome' for record_kind 'verification'".to_string()]);
}

/// The loader reads a record through core-1o: escapes, comments and the wrapped form read as the hand parser
/// read the plain form, and a key written twice that takes one value is refused by the schema.
#[test]
fn a_record_is_read_by_core_1o() {
    let name = "v1.intent.1o.diff";
    let plain = "diff {\n  schema \"nodeplan.1o.diff.v1\"\n  record_kind \"intent\"\n  task_id \"t\"\n  revision \"base\"\n  sequence 2\n  intent \"Who reads \\\"NODEPLAN_*\\\"?\\nA second line.\"\n}\n";
    let doc = parse_one_o_diff_text(name, plain).unwrap();
    assert_eq!(doc.one("intent").unwrap(), "Who reads \"NODEPLAN_*\"?\nA second line.");
    assert_eq!(doc.one("sequence").unwrap(), "2");
    assert_eq!(parse_one_o_diff_text(name, &doc.render()).unwrap().fields, doc.fields);
    let commented = plain.replace("  sequence 2\n", "  // the second record\n  sequence 2 /* of two */\n");
    assert_eq!(parse_one_o_diff_text(name, &commented).unwrap().fields, doc.fields);
    assert_eq!(parse_one_o_diff_text(name, &format!("{{\n{plain}}}\n")).unwrap().fields, doc.fields);
    let twice = plain.replace("  sequence 2\n", "  sequence 2\n  sequence 3\n");
    let e = parse_one_o_diff_text(name, &twice).unwrap_err();
    assert!(e.contains("field sequence must appear exactly once"), "{e}");
}

/// What 0.1.0 read, 0.1.2 reads: a diagnosis naming a change spec's operation (custodian_kit's exercise writes
/// one), which the schema's `operation` enum (the change record's) would refuse. 0.1.1 checked the schema on read
/// and refused it.
#[test]
fn a_record_the_schema_is_stricter_about_still_reads() {
    let rec = DiffDocument::record(DiffRecordKind::Diagnosis).with("diagnosis", "d").with("code", "F1").with("operation", "add_field").with("unit", "item");
    let dir = tempfile::tempdir().unwrap();
    task(dir.path());
    let written = write_record(dir.path(), "demo_task", "base", rec).unwrap();
    assert_eq!(written.one("operation").unwrap(), "add_field");
}

/// Every record under `TASK_HISTORY_GOLDEN_CORPUS` (a copy of a project's `.custodian`) reads, and its fields,
/// rendered and read again, are the same.
#[test]
fn every_record_of_a_corpus_reads() {
    let Ok(root) = std::env::var("TASK_HISTORY_GOLDEN_CORPUS") else { return };
    let mut n = 0;
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(".1o.diff") {
                let text = fs::read_to_string(&p).unwrap();
                let name = p.file_name().unwrap().to_str().unwrap();
                let doc = parse_one_o_diff_text(name, &text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                assert_eq!(parse_one_o_diff_text(name, &doc.render()).unwrap().fields, doc.fields, "{}", p.display());
                n += 1;
            }
        }
    }
    eprintln!("{n} records read");
    assert!(n > 0);
}

/// A record 0.1.0 read that 1o cannot carry (a string holding a bare double quote, written before strings were
/// escaped; a `#` line) is read as 0.1.0 read it, so every history already written still loads the same.
#[test]
fn a_record_written_before_escapes_reads_as_it_did() {
    let name = "v1.solution.1o.diff";
    let head = "diff {\n  schema \"nodeplan.1o.diff.v1\"\n  record_kind \"solution\"\n  task_id \"t\"\n  revision \"base\"\n  sequence 3\n";
    let bare = format!("{head}  solution \"use \"{{}}\", p.problem_id\"\n}}\n");
    assert_eq!(parse_one_o_diff_text(name, &bare).unwrap().one("solution").unwrap(), "use \"{}\", p.problem_id");
    let hashed = format!("{head}  # an old label\n  solution \"s\"\n}}\n");
    assert_eq!(parse_one_o_diff_text(name, &hashed).unwrap().one("solution").unwrap(), "s");
    // written again, the record is 1o core-1o reads
    let doc = parse_one_o_diff_text(name, &bare).unwrap();
    let nodes = core_1o::engine::Parser::new(core_1o::engine::Lexer::new(&doc.render()).tokenize()).parse().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(parse_one_o_diff_text(name, &doc.render()).unwrap().fields, doc.fields);
}

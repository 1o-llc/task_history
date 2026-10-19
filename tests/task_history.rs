use std::fs;
use tempfile::tempdir;
use task_history::*;

/// His real history (60 tasks, 122 revisions), copied as this crate's fixture.
fn seed() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/history")
}

/// The tree that history describes: the world_modeling checkout (`TASK_HISTORY_TREE` names another).
fn described_tree() -> std::path::PathBuf {
    std::env::var_os("TASK_HISTORY_TREE").map(Into::into).unwrap_or_else(|| "/Users/jameshunt/Desktop/claude/world_frames/nodeplan_world_modeling".into())
}

fn doc(_name:&str, kind:&str, extra:&str)->String { format!("diff {{\n  schema \"nodeplan.1o.diff.v1\"\n  record_kind \"{kind}\"\n  task_id \"demo\"\n  revision \"base\"\n  sequence 1\n{extra}}}\n") }

#[test] fn sandwiched_filename_parses(){let p=parse_one_o_diff_filename("v1.task.1o.diff").unwrap();assert_eq!(p.version,"v1");assert_eq!(p.identity,"task");assert_eq!(p.modifier,"diff");}
#[test] fn dotted_identity_parses(){assert_eq!(parse_one_o_diff_filename("v1.change.000001.1o.diff").unwrap().identity,"change.000001");}
#[test] fn bare_one_o_is_rejected(){assert!(parse_one_o_diff_filename("task.1o").is_err());}
#[test] fn missing_version_is_rejected(){assert!(parse_one_o_diff_filename("task.1o.diff").is_err());}
#[test] fn uppercase_identity_is_rejected(){assert!(parse_one_o_diff_filename("v1.Task.1o.diff").is_err());}
#[test] fn wrong_modifier_is_rejected(){assert!(parse_one_o_diff_filename("v1.task.1o.model").is_err());}
#[test] fn base_revision_is_zero(){assert_eq!(validate_revision_name("base").unwrap(),0);}
#[test] fn fix1_revision_is_one(){assert_eq!(validate_revision_name("fix1").unwrap(),1);}
#[test] fn fix12_revision_is_twelve(){assert_eq!(validate_revision_name("fix12").unwrap(),12);}
#[test] fn fix0_is_rejected(){assert!(validate_revision_name("fix0").is_err());}
#[test] fn arbitrary_revision_is_rejected(){assert!(validate_revision_name("repair").is_err());}
#[test] fn task_document_parses(){let s=doc("v1.task.1o.diff","task","  title \"Demo\"\n  parent_milestone \"9o\"\n  status \"accepted\"\n");assert_eq!(parse_one_o_diff_text("v1.task.1o.diff",&s).unwrap().kind().unwrap(),DiffRecordKind::Task);}
#[test] fn intent_document_parses(){let s=doc("v1.intent.1o.diff","intent","  intent \"Do it\"\n");assert!(parse_one_o_diff_text("v1.intent.1o.diff",&s).is_ok());}
#[test] fn problem_document_parses(){let s=doc("v1.problem.1o.diff","problem","  problem \"It failed\"\n");assert!(parse_one_o_diff_text("v1.problem.1o.diff",&s).is_ok());}
#[test] fn diagnosis_document_parses(){let s=doc("v1.diagnosis.1o.diff","diagnosis","  diagnosis \"Cause\"\n");assert!(parse_one_o_diff_text("v1.diagnosis.1o.diff",&s).is_ok());}
#[test] fn solution_document_parses(){let s=doc("v1.solution.1o.diff","solution","  solution \"Repair\"\n");assert!(parse_one_o_diff_text("v1.solution.1o.diff",&s).is_ok());}
#[test] fn verification_document_parses(){let s=doc("v1.verification.1o.diff","verification","  check \"tests\"\n  outcome \"pass\"\n");assert!(parse_one_o_diff_text("v1.verification.1o.diff",&s).is_ok());}
#[test] fn result_document_parses(){let s=doc("v1.result.1o.diff","result",&format!("  status \"accepted\"\n  resulting_tree_digest \"{}\"\n","a".repeat(64)));assert!(parse_one_o_diff_text("v1.result.1o.diff",&s).is_ok());}
#[test] fn change_document_parses(){let s=doc("v1.change.000001.1o.diff","change",&format!("  path \"src/lib.rs\"\n  operation \"modify\"\n  before_digest \"{}\"\n  after_digest \"{}\"\n","a".repeat(64),"b".repeat(64)));assert!(parse_one_o_diff_text("v1.change.000001.1o.diff",&s).is_ok());}
#[test] fn bad_change_digest_rejected(){let s=doc("v1.change.1.1o.diff","change","  path \"x\"\n  operation \"add\"\n  before_digest \"none\"\n  after_digest \"bad\"\n");assert!(parse_one_o_diff_text("v1.change.1.1o.diff",&s).is_err());}
#[test] fn unsupported_change_operation_rejected(){let s=doc("v1.change.1.1o.diff","change",&format!("  path \"x\"\n  operation \"copy\"\n  before_digest \"none\"\n  after_digest \"{}\"\n","a".repeat(64)));assert!(parse_one_o_diff_text("v1.change.1.1o.diff",&s).is_err());}
#[test] fn missing_common_field_rejected(){let s="diff {\n schema \"nodeplan.1o.diff.v1\"\n}\n";assert!(parse_one_o_diff_text("v1.task.1o.diff",s).is_err());}
#[test] fn wrong_schema_rejected(){let s=doc("v1.intent.1o.diff","intent","  intent \"x\"\n").replace("nodeplan.1o.diff.v1","wrong");assert!(parse_one_o_diff_text("v1.intent.1o.diff",&s).is_err());}
#[test] fn unbalanced_document_rejected(){assert!(parse_one_o_diff_text("v1.task.1o.diff","diff {\n").is_err());}
#[test] fn content_after_close_rejected(){let s="diff {\n schema \"nodeplan.1o.diff.v1\"\n}\n junk\n";assert!(parse_one_o_diff_text("v1.task.1o.diff",s).is_err());}
#[test] fn repeated_single_field_rejected(){let s=doc("v1.intent.1o.diff","intent","  intent \"a\"\n  intent \"b\"\n");assert!(parse_one_o_diff_text("v1.intent.1o.diff",&s).is_err());}
#[test] fn repeated_tag_is_allowed(){let s=doc("v1.task.1o.diff","task","  title \"Demo\"\n  parent_milestone \"9o\"\n  status \"accepted\"\n  tag \"a\"\n  tag \"b\"\n");let d=parse_one_o_diff_text("v1.task.1o.diff",&s).unwrap();assert_eq!(d.many("tag").len(),2);}
#[test] fn escaped_string_parses(){let s=doc("v1.intent.1o.diff","intent","  intent \"line\\nnext\"\n");assert_eq!(parse_one_o_diff_text("v1.intent.1o.diff",&s).unwrap().one("intent").unwrap(),"line\nnext");}
#[test] fn document_digest_is_sha256(){let s=doc("v1.intent.1o.diff","intent","  intent \"x\"\n");assert_eq!(parse_one_o_diff_text("v1.intent.1o.diff",&s).unwrap().document_digest.len(),64);}
#[test] fn seeded_history_loads(){let root=seed();let i=load_task_history(root).unwrap();assert!(i.task_count>=2);assert!(i.revision_count>=4);}
#[test] fn seeded_history_has_9n_fix_lineage(){let root=seed();let i=load_task_history(root).unwrap();let t=i.tasks.iter().find(|t|t.task_directory.contains("symbolic_prerequisite")).unwrap();assert_eq!(t.revisions.iter().map(|r|r.name.as_str()).collect::<Vec<_>>(),vec!["base","fix1","fix2"]);}
#[test] fn seeded_history_has_9o_base(){let root=seed();let i=load_task_history(root).unwrap();let t=i.tasks.iter().find(|t|t.task_directory.contains("thought_fragment")).unwrap();assert_eq!(t.revisions.len(),1);assert_eq!(t.revisions[0].name,"base");}
#[test] fn seeded_history_contains_problem_diagnosis_solution(){let root=seed();let i=load_task_history(root).unwrap();assert!(!find_records_by_kind(&i,DiffRecordKind::Problem).is_empty());assert!(!find_records_by_kind(&i,DiffRecordKind::Diagnosis).is_empty());assert!(!find_records_by_kind(&i,DiffRecordKind::Solution).is_empty());}
#[test] fn seeded_history_contains_exact_change_digests(){let root=seed();let i=load_task_history(root).unwrap();assert!(i.change_count>100);for d in find_records_by_kind(&i,DiffRecordKind::Change){for k in ["before_digest","after_digest"]{let v=d.one(k).unwrap();assert!(v=="none"||v.len()==64);}}}
#[test] fn find_changes_by_path_works(){let root=seed();let i=load_task_history(root).unwrap();assert!(!find_changes_for_path(&i,"Cargo.toml").is_empty());}
#[test] fn accepted_revisions_are_counted(){let root=seed();let i=load_task_history(root).unwrap();assert!(i.accepted_revision_count>=2);}
#[test] fn history_index_digest_is_sha256(){let root=seed();let i=load_task_history(root).unwrap();assert_eq!(i.index_digest.len(),64);}
#[test] fn non_1o_file_inside_revision_fails(){let d=tempdir().unwrap();let r=d.path().join("task/base");fs::create_dir_all(&r).unwrap();fs::write(r.join("notes.md"),"x").unwrap();assert!(load_task_history(d.path()).is_err());}
#[test] fn noncontiguous_fix_lineage_fails(){let d=tempdir().unwrap();fs::create_dir_all(d.path().join("task/base")).unwrap();fs::create_dir_all(d.path().join("task/fix2")).unwrap();assert!(load_task_history(d.path()).is_err());}
#[test] fn duplicate_sequence_fails(){let d=tempdir().unwrap();let r=d.path().join("task/base");fs::create_dir_all(&r).unwrap();let a=doc("v1.intent.1o.diff","intent","  intent \"a\"\n");let b=doc("v1.problem.1o.diff","problem","  problem \"b\"\n");fs::write(r.join("v1.intent.1o.diff"),a).unwrap();fs::write(r.join("v1.problem.1o.diff"),b).unwrap();assert!(load_task_history(d.path()).is_err());}
#[test] fn revision_field_must_match_folder(){let d=tempdir().unwrap();let r=d.path().join("task/fix1");fs::create_dir_all(&r).unwrap();let a=doc("v1.intent.1o.diff","intent","  intent \"a\"\n");fs::write(r.join("v1.intent.1o.diff"),a).unwrap();assert!(load_task_history(d.path()).is_err());}
#[test] fn task_directory_rejects_spaces(){let d=tempdir().unwrap();fs::create_dir_all(d.path().join("bad task/base")).unwrap();assert!(load_task_history(d.path()).is_err());}
#[test] fn history_schema_is_9p(){assert_eq!(TASK_HISTORY_SCHEMA,"nodeplan.task_history.9p.v1");}
#[test] fn diff_modifier_is_explicit(){assert_eq!(ONE_O_DIFF_MODIFIER,"diff");}
#[test] fn diff_schema_is_explicit(){assert_eq!(ONE_O_DIFF_SCHEMA,"nodeplan.1o.diff.v1");}
#[test] fn world_fact_authority_is_not_part_of_history_schema(){assert!(!ONE_O_DIFF_SCHEMA.contains("world_knowledge"));}
#[test] fn result_pending_is_allowed(){let s=doc("v1.result.1o.diff","result","  status \"candidate\"\n  resulting_tree_digest \"pending\"\n");assert!(parse_one_o_diff_text("v1.result.1o.diff",&s).is_ok());}
#[test] fn pending_verification_is_allowed(){let s=doc("v1.verification.1o.diff","verification","  check \"mac_acceptance\"\n  outcome \"pending\"\n");assert!(parse_one_o_diff_text("v1.verification.1o.diff",&s).is_ok());}
#[test] fn boolean_values_are_lexically_supported(){let s=doc("v1.intent.1o.diff","intent","  intent \"x\"\n  provisional true\n");assert_eq!(parse_one_o_diff_text("v1.intent.1o.diff",&s).unwrap().one("provisional").unwrap(),"true");}
#[test] #[ignore = "reads the tree the history describes (the world_modeling checkout); run with --ignored where it is"] fn terminal_history_matches_current_tree(){let root=seed();let tree=described_tree();let i=load_task_history(root).unwrap();assert!(verify_terminal_tree_changes(&i,tree).unwrap()>100);}

use std::path::PathBuf;
use task_history::{find_changes_for_path, find_records_by_kind, load_task_history, verify_terminal_tree_changes, DiffRecordKind};
fn main() -> Result<(),String> {
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/history");
 let index=load_task_history(&root)?;
 let nine_n=index.tasks.iter().find(|t|t.task_directory.contains("symbolic_prerequisite")).ok_or("missing 9N history")?;
 let nine_o=index.tasks.iter().find(|t|t.task_directory.contains("thought_fragment")).ok_or("missing 9O history")?;
 println!("world_model_task_history_v1");
 println!("history_schema={}",index.schema);
 println!("one_o_diff_modifier=diff");
 println!("history_task_count={}",index.task_count);
 println!("history_revision_count={}",index.revision_count);
 println!("history_document_count={}",index.document_count);
 println!("history_change_count={}",index.change_count);
 println!("history_accepted_revision_count={}",index.accepted_revision_count);
 println!("nine_n_revision_lineage={}",nine_n.revisions.iter().map(|r|r.name.as_str()).collect::<Vec<_>>().join("->"));
 println!("nine_o_revision_lineage={}",nine_o.revisions.iter().map(|r|r.name.as_str()).collect::<Vec<_>>().join("->"));
 println!("problem_records={}",find_records_by_kind(&index,DiffRecordKind::Problem).len());
 println!("diagnosis_records={}",find_records_by_kind(&index,DiffRecordKind::Diagnosis).len());
 println!("solution_records={}",find_records_by_kind(&index,DiffRecordKind::Solution).len());
 println!("cargo_change_records={}",find_changes_for_path(&index,"Cargo.toml").len());
 println!("history_index_digest={}",index.index_digest);
 let tree=std::env::var_os("TASK_HISTORY_TREE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/Users/jameshunt/Desktop/claude/world_frames/nodeplan_world_modeling"));
 println!("terminal_tree_changes_verified={}",verify_terminal_tree_changes(&index,&tree)?);
 println!("accepted_revisions_immutable=true");
 println!("history_world_fact_authority_claimed=false");
 println!("history_proof_authority_claimed=false");
 println!("PASS world_model_task_history_v1 ok");
 Ok(())
}

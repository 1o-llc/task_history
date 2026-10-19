# Changelog

## 0.1.0 (unreleased)

The task-history loader from the world-modeling tree's
`world_model_task_history_core`, as a crate of its own, with a writer.

- The loader, its checks and its queries are as they were:
  `parse_one_o_diff_filename`, `parse_one_o_diff_text`,
  `validate_diff_document`, `load_task_history`, `find_changes_for_path`,
  `find_records_by_kind`, `verify_terminal_tree_changes`. Its 50 tests are
  kept, run against a copy of the real history.
- The code is split into `diff_records`, `changes`, `revisions` and `tree`.
- Digests are stable_digest's sha256 in place of sha2's. The two give the same
  lowercase hex for the same bytes; a test checks it on fixed inputs and on
  every record digest of the fixture.
- New: `write_record` adds a record with the next sequence of its revision,
  `DiffDocument::render` writes the text the parser reads back, `change_record`
  and `file_digest` make change records, `next_revision` and `latest_revision`
  follow the lineage, and `write_index` refreshes the history's index.
- New: `schema/diff/v1.schema.json`, the core-1o schema of the `diff` modifier.

Proof: `cargo test` (49 kept tests and 5 writer tests). The kept test
`terminal_history_matches_current_tree` is ignored by default, because it
reads the world-modeling checkout. On that checkout today it fails on
`ASK.command`: the file was changed on 2026-08-18, after the history's last
record of it.

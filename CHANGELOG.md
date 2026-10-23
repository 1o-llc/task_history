# Changelog

## 0.1.1 (2026-10-23)

core-1o 0.2 reads what this crate writes: a record at the top level, strings with
`\\ \" \n \t \r` escaped, and a schema that can say which keys each record kind
needs. So the loader no longer needs a parser of its own for the records 1o can
carry, and the schema says what before only the loader could. Nothing is removed
from the API (the rustdoc JSON of 0.1.0 and 0.1.1 name the same public items).
Links are pinned to 59022f9; the lines they replace to df736bf (0.1.0).

- **Records are read by core-1o.** `parse_one_o_diff_text`
  ([diff_records.rs L294](https://github.com/1o-llc/task_history/blob/59022f93c50d81544608c35d8f7da956f0bdfcdb/src/diff_records.rs#L294)) parses the file as
  written with core-1o, checks it against `schema/diff/v1.schema.json`, then
  `validate_diff_document` as before. It was a line-by-line parser of its own
  ([diff_records.rs L194](https://github.com/1o-llc/task_history/blob/df736bfcbacb86fad2061880e195e1a536d5929e/src/diff_records.rs#L194),
  [L219](https://github.com/1o-llc/task_history/blob/df736bfcbacb86fad2061880e195e1a536d5929e/src/diff_records.rs#L219)). Test: `a_record_is_read_by_core_1o`
  (tests/writer.rs): escapes, `//` and `/* */` comments and the wrapped form read
  as the plain form, and a single key written twice is refused by the schema.
- **The schema says each kind's keys.** `x-1o-required-when` on `record_kind`
  ([v1.schema.json L154](https://github.com/1o-llc/task_history/blob/59022f93c50d81544608c35d8f7da956f0bdfcdb/schema/diff/v1.schema.json#L154)),
  `enumDescriptions` for the kinds ([L42](https://github.com/1o-llc/task_history/blob/59022f93c50d81544608c35d8f7da956f0bdfcdb/schema/diff/v1.schema.json#L42)),
  `tag` as `type: array` and every other key written once (`x-1o-single-keys`).
  The language server now refuses a `change` record without its `after_digest`
  where it is written. Test: `the_diff_schema_as_core_1o_reads_it`, which also
  checks the schema's lists against `DiffRecordKind::required` kind by kind.
- **Every record already written still loads the same.** Three records of the
  fixture history hold a string with a bare double quote (written before strings
  were escaped) or a `#` line; 1o cannot carry either. `written_before_escapes`
  ([diff_records.rs L226](https://github.com/1o-llc/task_history/blob/59022f93c50d81544608c35d8f7da956f0bdfcdb/src/diff_records.rs#L226)) finds that form and
  `legacy_fields` ([L243](https://github.com/1o-llc/task_history/blob/59022f93c50d81544608c35d8f7da956f0bdfcdb/src/diff_records.rs#L243)) reads it as 0.1.0 did.
  Test: `a_record_written_before_escapes_reads_as_it_did`. Over the fixture
  history (3,492 records) and a copy of the kit's `.custodian` (11,061 records),
  0.1.0 and 0.1.1 read the same fields and the same digest for every record; the
  corpus test is `every_record_of_a_corpus_reads` with
  `TASK_HISTORY_GOLDEN_CORPUS` set.
- core-1o moves from a dev-dependency to a dependency, at 0.2.

Proof: `cargo test` (49 kept tests, 8 writer tests).

## 0.1.0 (2026-10-19)

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

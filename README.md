# task_history

task_history keeps a task's history as typed 1o records. A task's revisions
are folders, `base` then `fix1`, `fix2`, with no gap. Each record is one file,
`v1.<identity>.1o.diff`:

```text
diff {
  schema "nodeplan.1o.diff.v1"
  record_kind "change"
  task_id "symbolic_prerequisite_reasoning_9n"
  revision "base"
  sequence 4
  path "SYMBOLIC_PREREQ_9N_9M_CARGO_NORMALIZED_SHA256.txt"
  operation "add"
  before_digest "none"
  after_digest "9650b497c9c90398589fec6e85689e74d317cb118b450a791590ede274297b20"
}
```

| record | what it carries |
|---|---|
| `task` | title, parent_milestone, status, tags |
| `intent` | what the task means to do |
| `problem`, `diagnosis` | what was wrong, and why |
| `solution` | what was done |
| `change` | a path, its operation (add, modify, remove, rename), the sha256 of its bytes before and after (or `none`) |
| `verification` | a check and its outcome (pass, fail, pending, superseded) |
| `result` | the revision's status and the digest of the tree it leaves |
| `index` | the history's task and revision counts |

Every record carries schema, record_kind, task_id, revision and sequence. The
history describes engineering work; it claims no fact or proof authority.

## Use

```rust,no_run
use task_history::*;

let index = load_task_history("history")?;
println!("{} tasks, {} revisions, {} changes", index.task_count, index.revision_count, index.change_count);
for d in find_changes_for_path(&index, "Cargo.toml") {
    println!("{} {}", d.one("revision")?, d.one("operation")?);
}
// the last change of every path holds in the tree
verify_terminal_tree_changes(&index, ".")?;

// a record written with the next sequence of its revision
write_record("history", "my_task", "base", DiffDocument::record(DiffRecordKind::Intent).with("intent", "say what changed"))?;
write_index("history")?;
# Ok::<(), String>(())
```

## Modules

- `diff_records`: one record, its file name, kind, validation and text.
- `changes`: change records and file digests.
- `revisions`: the `base`/`fixN` lineage.
- `tree`: a tree checked against a history's last changes.

## The schema

`schema/diff/v1.schema.json` (`DIFF_SCHEMA_JSON`) declares the `diff`
modifier for core-1o, so language-server-1o can check a record. Two things it
cannot say: which keys each kind needs (core-1o's schemas have no
conditionals; the loader checks them), and the file's own shape. A record is
`diff { ... }` with escapes and `#` comments, and core-1o's parser reads only a
top-level `{ ... }` block with no escapes, so a record file is checked once it
is wrapped as `{ diff { ... } }`.

## The fixture

`tests/fixtures/history` is a copy of the real history this format was made
for: 60 tasks, 122 revisions, 3,492 records, 2,647 of them changes.

License: MIT OR Apache-2.0

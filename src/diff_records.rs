//! One record: a `v1.<identity>.1o.diff` file, its fields, its kind, its
//! validation, and its text as the writer renders it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ONE_O_DIFF_SCHEMA: &str = "nodeplan.1o.diff.v1";
pub const ONE_O_DIFF_MODIFIER: &str = "diff";

/// sha256 of the bytes, lowercase hex (stable_digest's; the same bytes sha2's `{:x}` gives).
pub(crate) fn digest_hex(bytes: &[u8]) -> String {
    stable_digest::StableDigest::sha256(bytes).as_str().to_string()
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneODiffFilename {
    pub version: String,
    pub identity: String,
    pub modifier: String,
}

pub fn parse_one_o_diff_filename(name: &str) -> Result<OneODiffFilename, String> {
    let stem = name.strip_suffix(".1o.diff").ok_or_else(|| "1o diff filename must end in .1o.diff".to_string())?;
    let (version, identity) = stem.split_once('.').ok_or_else(|| "1o diff filename requires vN.<identity>.1o.diff".to_string())?;
    if version.len() < 2 || !version.starts_with('v') || !version[1..].bytes().all(|b| b.is_ascii_digit()) {
        return Err("1o diff filename version must be vN".into());
    }
    if identity.is_empty() || identity.starts_with('.') || identity.ends_with('.') {
        return Err("1o diff identity is empty or malformed".into());
    }
    if !identity.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')) {
        return Err("1o diff identity contains unsupported characters".into());
    }
    Ok(OneODiffFilename { version: version.into(), identity: identity.into(), modifier: ONE_O_DIFF_MODIFIER.into() })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffRecordKind {
    Task,
    Intent,
    Problem,
    Diagnosis,
    Solution,
    Change,
    Verification,
    Result,
    Index,
}

impl DiffRecordKind {
    pub const ALL: &'static [DiffRecordKind] = &[Self::Task, Self::Intent, Self::Problem, Self::Diagnosis, Self::Solution, Self::Change, Self::Verification, Self::Result, Self::Index];

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "task" => Ok(Self::Task),
            "intent" => Ok(Self::Intent),
            "problem" => Ok(Self::Problem),
            "diagnosis" => Ok(Self::Diagnosis),
            "solution" => Ok(Self::Solution),
            "change" => Ok(Self::Change),
            "verification" => Ok(Self::Verification),
            "result" => Ok(Self::Result),
            "index" => Ok(Self::Index),
            _ => Err(format!("unsupported diff record_kind {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Intent => "intent",
            Self::Problem => "problem",
            Self::Diagnosis => "diagnosis",
            Self::Solution => "solution",
            Self::Change => "change",
            Self::Verification => "verification",
            Self::Result => "result",
            Self::Index => "index",
        }
    }

    /// The fields a record of this kind must carry, beyond the common five.
    pub fn required(self) -> &'static [&'static str] {
        match self {
            Self::Task => &["title", "parent_milestone", "status"],
            Self::Intent => &["intent"],
            Self::Problem => &["problem"],
            Self::Diagnosis => &["diagnosis"],
            Self::Solution => &["solution"],
            Self::Change => &["path", "operation", "before_digest", "after_digest"],
            Self::Verification => &["check", "outcome"],
            Self::Result => &["status", "resulting_tree_digest"],
            Self::Index => &["task_count", "revision_count"],
        }
    }
}

/// The fields every record carries.
pub const COMMON_FIELDS: &[&str] = &["schema", "record_kind", "task_id", "revision", "sequence"];
/// A change's operations.
pub const OPERATIONS: &[&str] = &["add", "modify", "remove", "rename"];
/// A verification's outcomes.
pub const OUTCOMES: &[&str] = &["pass", "fail", "pending", "superseded"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffDocument {
    pub filename: OneODiffFilename,
    pub fields: BTreeMap<String, Vec<String>>,
    pub document_digest: String,
}

impl DiffDocument {
    pub fn one(&self, key: &str) -> Result<&str, String> {
        let values = self.fields.get(key).ok_or_else(|| format!("missing field {key}"))?;
        if values.len() != 1 {
            return Err(format!("field {key} must appear exactly once"));
        }
        Ok(values[0].as_str())
    }

    pub fn many(&self, key: &str) -> &[String] {
        self.fields.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn kind(&self) -> Result<DiffRecordKind, String> {
        DiffRecordKind::parse(self.one("record_kind")?)
    }

    /// A record of `kind` to write: its own fields added with [`DiffDocument::with`];
    /// the writer fills the schema, task, revision, sequence, file name and digest.
    pub fn record(kind: DiffRecordKind) -> DiffDocument {
        let mut fields = BTreeMap::new();
        fields.insert("record_kind".to_string(), vec![kind.as_str().to_string()]);
        DiffDocument { filename: OneODiffFilename { version: "v1".into(), identity: kind.as_str().into(), modifier: ONE_O_DIFF_MODIFIER.into() }, fields, document_digest: String::new() }
    }

    /// One more value of `key` (a key given twice is a list, as `tag`).
    pub fn with(mut self, key: &str, value: impl Into<String>) -> DiffDocument {
        self.fields.entry(key.to_string()).or_default().push(value.into());
        self
    }

    pub(crate) fn set(&mut self, key: &str, value: impl Into<String>) {
        self.fields.insert(key.to_string(), vec![value.into()]);
    }

    /// The record's text: `diff { ... }`, the common fields first, then the
    /// rest by key; a value is unquoted when it is an unsigned integer or a
    /// bool, else quoted with `\\ \" \n \t \r` escaped.
    pub fn render(&self) -> String {
        let mut out = String::from("diff {\n");
        let mut line = |k: &str, v: &str| {
            let bare = (!v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())) || v == "true" || v == "false";
            if bare {
                out.push_str(&format!("  {k} {v}\n"));
            } else {
                let mut q = String::with_capacity(v.len() + 2);
                for ch in v.chars() {
                    match ch {
                        '\\' => q.push_str("\\\\"),
                        '"' => q.push_str("\\\""),
                        '\n' => q.push_str("\\n"),
                        '\t' => q.push_str("\\t"),
                        '\r' => q.push_str("\\r"),
                        c => q.push(c),
                    }
                }
                out.push_str(&format!("  {k} \"{q}\"\n"));
            }
        };
        for k in COMMON_FIELDS {
            for v in self.many(k) {
                line(k, v);
            }
        }
        for (k, vs) in &self.fields {
            if COMMON_FIELDS.contains(&k.as_str()) {
                continue;
            }
            for v in vs {
                line(k, v);
            }
        }
        out.push_str("}\n");
        out
    }
}

/// A string value as task_history 0.1.0 read it: the line's text from its first
/// quote to its last, the escapes `\\ \" \n \t \r` read, any other quote kept.
fn parse_quoted(input: &str) -> Result<String, String> {
    if !input.starts_with('"') || !input.ends_with('"') || input.len() < 2 {
        return Err("string value must be quoted".into());
    }
    let inner = &input[1..input.len() - 1];
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let esc = chars.next().ok_or_else(|| "trailing escape".to_string())?;
        match esc {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            _ => return Err(format!("unsupported escape \\{esc}")),
        }
    }
    Ok(out)
}

/// Whether a record is in a form only 0.1.0's line reading takes: a line's string
/// value holds a double quote with no backslash before it (written before strings
/// were escaped; core-1o ends the string at that quote), or a `#` line (0.1.0 skipped
/// it; 1o's comments are `//` and `/* */`). Such a record is read as 0.1.0 read it,
/// line by line ([`legacy_fields`]); every other record is core-1o's.
fn written_before_escapes(text: &str) -> bool {
    text.lines().any(|l| l.trim_start().starts_with('#')) || text.lines().filter_map(|l| l.trim().split_once(char::is_whitespace)).any(|(_, v)| {
        let v = v.trim();
        let Some(inner) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else { return false };
        let mut backslashes = 0;
        for c in inner.chars() {
            match c {
                '"' if backslashes % 2 == 0 => return true,
                '\\' => backslashes += 1,
                _ => backslashes = 0,
            }
        }
        false
    })
}

/// A record's fields read line by line, as task_history 0.1.0 read every record.
fn legacy_fields(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut opened = false;
    let mut closed = false;
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !opened {
            if line != "diff {" {
                return Err(format!("line {}: expected 'diff {{'", line_no + 1));
            }
            opened = true;
            continue;
        }
        if line == "}" {
            closed = true;
            continue;
        }
        if closed {
            return Err(format!("line {}: content after closing brace", line_no + 1));
        }
        let split = line.find(char::is_whitespace).ok_or_else(|| format!("line {}: expected key value", line_no + 1))?;
        let key = &line[..split];
        let raw_value = line[split..].trim();
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return Err(format!("line {}: invalid key", line_no + 1));
        }
        let value = if raw_value.starts_with('"') {
            parse_quoted(raw_value)?
        } else if raw_value == "true" || raw_value == "false" || raw_value.bytes().all(|b| b.is_ascii_digit()) {
            raw_value.to_string()
        } else {
            return Err(format!("line {}: value must be quoted string, bool, or unsigned integer", line_no + 1));
        };
        fields.entry(key.to_string()).or_default().push(value);
    }
    if !opened || !closed {
        return Err("diff document must contain balanced diff { ... }".into());
    }
    Ok(fields)
}

/// A record's text read by core-1o (`diff { ... }`, the record at the top level, or
/// wrapped as `{ diff { ... } }`) and checked against the shipped schema
/// ([`crate::DIFF_SCHEMA_JSON`]: each kind's keys by `x-1o-required-when`, a key
/// that is not `tag` written once); then the record's own rules
/// ([`validate_diff_document`]). A record written before strings were escaped
/// ([`written_before_escapes`]) is read as 0.1.0 read it. A value is kept as its text: a string as
/// written, an unsigned integer in decimal, a bool as `true`/`false`.
pub fn parse_one_o_diff_text(filename: &str, text: &str) -> Result<DiffDocument, String> {
    use core_1o::engine::{validate_ast, EngineDiagnostic, Lexer, NodeValue, Parser};
    let parsed_name = parse_one_o_diff_filename(filename)?;
    if written_before_escapes(text) {
        let doc = DiffDocument { filename: parsed_name, fields: legacy_fields(text)?, document_digest: digest_hex(text.as_bytes()) };
        validate_diff_document(&doc)?;
        return Ok(doc);
    }
    let at = |offset: usize| text.chars().take(offset).filter(|c| *c == '\n').count() + 1;
    let nodes = Parser::new(Lexer::new(text).tokenize()).parse().map_err(|e| {
        let d = EngineDiagnostic::from(&e);
        format!("line {}: {}", at(d.span.start), d.message)
    })?;
    let schema: serde_json::Value = serde_json::from_str(crate::DIFF_SCHEMA_JSON).map_err(|e| e.to_string())?;
    let mut diags = Vec::new();
    validate_ast(&nodes, &schema, &mut diags);
    if let Some(d) = diags.first() {
        return Err(format!("line {}: {}", at(d.span.start), d.message));
    }
    let [record] = nodes.as_slice() else { return Err("diff document must contain one diff { ... }".into()) };
    let NodeValue::Block(entries) = &record.value else { return Err("diff document must contain one diff { ... }".into()) };
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in entries {
        let value = match &e.value {
            NodeValue::String(s) => s.clone(),
            NodeValue::Number(n) => n.to_string(),
            NodeValue::Bool(b) => b.to_string(),
            _ => return Err(format!("line {}: value must be quoted string, bool, or unsigned integer", at(e.span.start))),
        };
        fields.entry(e.key.clone()).or_default().push(value);
    }
    let doc = DiffDocument { filename: parsed_name, fields, document_digest: digest_hex(text.as_bytes()) };
    validate_diff_document(&doc)?;
    Ok(doc)
}

fn require(doc: &DiffDocument, keys: &[&str]) -> Result<(), String> {
    for key in keys {
        doc.one(key)?;
    }
    Ok(())
}

pub fn validate_diff_document(doc: &DiffDocument) -> Result<(), String> {
    require(doc, COMMON_FIELDS)?;
    if doc.one("schema")? != ONE_O_DIFF_SCHEMA {
        return Err("wrong 1o diff schema".into());
    }
    let _seq: u64 = doc.one("sequence")?.parse().map_err(|_| "sequence must be an unsigned integer".to_string())?;
    crate::revisions::validate_revision_name(doc.one("revision")?)?;
    let kind = doc.kind()?;
    require(doc, kind.required())?;
    match kind {
        DiffRecordKind::Change => {
            if !OPERATIONS.contains(&doc.one("operation")?) {
                return Err("unsupported change operation".into());
            }
            for key in ["before_digest", "after_digest"] {
                let v = doc.one(key)?;
                if v != "none" && !is_sha256(v) {
                    return Err(format!("{key} must be sha256 or none"));
                }
            }
            Ok(())
        }
        DiffRecordKind::Verification => {
            if !OUTCOMES.contains(&doc.one("outcome")?) {
                return Err("unsupported verification outcome".into());
            }
            Ok(())
        }
        DiffRecordKind::Result => {
            let d = doc.one("resulting_tree_digest")?;
            if d != "pending" && !is_sha256(d) {
                return Err("resulting_tree_digest must be sha256 or pending".into());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

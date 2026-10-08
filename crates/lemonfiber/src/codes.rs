//! The code registry as it is published: `contract/codes.json`, and the error-code
//! reference rendered from that file.
//!
//! `just codes` writes both. The comparisons live in tests, so a stale artefact fails
//! the build rather than the program that emits it. The reference is rendered from the
//! committed file rather than from the registry, so that what a reader of the page sees
//! and what a client generating from the file reads cannot be two different lists.

use std::fmt::Write as _;

use lemonfiber_core::error::codes::{every_declared, families, RETIRED};
use serde_json::{json, Value};

/// Where the published registry is kept, relative to the workspace root.
pub const CODES_JSON: &str = "contract/codes.json";

/// Where the reference rendered from it is kept, relative to the workspace root.
pub const CODES_PATH: &str = "reference/error-codes.md";

/// What the reference opens with, before the first family.
const PREAMBLE: &str = "\
# `lemonfiber` — error codes

Rendered from `contract/codes.json`, which is generated from the registry every code is
declared in. Run `just codes` to rewrite both.

Every code lemonfiber can raise, and nothing else. A code is a family and a number,
it is never recycled, and it is the token to search for. What each one means, and
what to do about it, is in `contract/codes.json` and written for operators at
<https://docs.lemonfiber.app/fixing/every-error-by-code/>.

";

/// The registry as `contract/codes.json` holds it.
#[must_use]
pub fn render_json() -> String {
    let families: Vec<Value> = families()
        .iter()
        .map(|family| json!({ "prefix": family.prefix(), "covers": family.covers() }))
        .collect();
    let codes: Vec<Value> = every_declared()
        .into_iter()
        .map(|declared| {
            let code = declared.code().as_str();
            json!({
                "code": code,
                "family": code.rsplit_once('-').map_or(code, |(family, _)| family),
                "name": declared.name(),
                "severity": declared.severity(),
                "exit": declared.leaves().exit(),
                "status": declared.status(),
                "summary": declared.description(),
                "meaning": declared.meaning(),
                "remedy": declared.remedy(),
                "since": declared.since(),
            })
        })
        .collect();
    let retired: Vec<Value> = RETIRED.iter().map(|code| json!(code)).collect();
    let mut out = String::from("{\n");
    listed(&mut out, "families", &families, ",");
    listed(&mut out, "codes", &codes, ",");
    listed(&mut out, "retired", &retired, "");
    out.push_str("}\n");
    out
}

/// One list of the registry, an entry to a line.
///
/// A line per entry rather than a line per field: written out field by field the
/// registry is several thousand lines, past what a reader takes in, and a change to
/// one code is a change to one line of the diff that shows it.
fn listed(out: &mut String, key: &str, entries: &[Value], after: &str) {
    let _ = writeln!(out, "  \"{key}\": [");
    let last = entries.len().saturating_sub(1);
    for (at, entry) in entries.iter().enumerate() {
        let comma = if at == last { "" } else { "," };
        let _ = writeln!(
            out,
            "    {}{comma}",
            serde_json::to_string(entry).unwrap_or_default()
        );
    }
    let _ = writeln!(out, "  ]{after}");
}

/// The reference, rendered from the text of `contract/codes.json`.
///
/// # Errors
///
/// Says why, where the text is not a registry this renders.
pub fn render_reference(published: &str) -> Result<String, String> {
    let published: Value = serde_json::from_str(published).map_err(|error| error.to_string())?;
    let field = |value: &Value, key: &str| -> Result<String, String> {
        match value.get(key) {
            Some(Value::String(text)) => Ok(text.clone()),
            Some(Value::Number(number)) => Ok(number.to_string()),
            _ => Err(format!("an entry has no {key}")),
        }
    };
    let listed = |key: &str| -> Result<&Vec<Value>, String> {
        published
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("no {key} list"))
    };

    let mut out = String::from(PREAMBLE);
    for family in listed("families")? {
        let prefix = field(family, "prefix")?;
        let _ = write!(out, "## `{prefix}` — {}\n\n", field(family, "covers")?);
        out.push_str("| Code | Name | Severity | Exit | Status | Since | Summary |\n");
        out.push_str("| ---- | ---- | -------- | ---- | ------ | ----- | ------- |\n");
        for code in listed("codes")? {
            if field(code, "family")? != prefix {
                continue;
            }
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} | {} | {} | {} | {} |",
                field(code, "code")?,
                field(code, "name")?,
                field(code, "severity")?,
                field(code, "exit")?,
                field(code, "status")?,
                field(code, "since")?,
                field(code, "summary")?,
            );
        }
        out.push('\n');
    }
    let retired: Vec<String> = listed("retired")?
        .iter()
        .filter_map(Value::as_str)
        .map(|code| format!("`{code}`"))
        .collect();
    let _ = writeln!(
        out,
        "## Retired\n\nPublished once, raised by nothing, and never given to another problem: {}.",
        retired.join(", ")
    );
    Ok(out)
}

#[cfg(test)]
mod tests;

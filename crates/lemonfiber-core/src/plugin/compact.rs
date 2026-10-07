//! A published artefact written so it stays short enough to read.
//!
//! Pretty-printed JSON gives every value a line of its own, so a schema doubles in
//! length on lists of names and on objects holding nothing but scalars. Those are
//! written on one line here instead, and everything holding a nested object or list
//! keeps the indented layout, in the order the serialiser wrote it.

/// A pretty-printed document with every innermost list or object of scalars on one
/// line, or nothing where what it was handed is not a JSON document.
#[must_use]
pub fn collapsed(pretty: &str) -> Option<String> {
    let mut lines: Vec<(String, bool)> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for line in pretty.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('}') || trimmed.starts_with(']') {
            let start = open.pop()?;
            let members = lines.get(start + 1..)?;
            if members.iter().all(|(_, scalar)| *scalar) {
                let inner = members
                    .iter()
                    .map(|(text, _)| text.trim())
                    .collect::<Vec<_>>()
                    .join(" ");
                let (head, _) = lines.get(start)?.clone();
                lines.truncate(start);
                lines.push((format!("{head}{inner}{trimmed}"), false));
                continue;
            }
            lines.push((line.to_owned(), false));
        } else if trimmed.ends_with('{') || trimmed.ends_with('[') {
            open.push(lines.len());
            lines.push((line.to_owned(), false));
        } else {
            lines.push((line.to_owned(), !empty_container(trimmed)));
        }
    }
    let mut text: String = lines
        .into_iter()
        .map(|(line, _)| line)
        .collect::<Vec<_>>()
        .join("\n");
    if pretty.ends_with('\n') {
        text.push('\n');
    }
    serde_json::from_str::<serde_json::Value>(pretty).ok()?;
    Some(text)
}

/// Whether a line's value is an empty list or object, which `serde_json` writes on
/// its own line and which is a container rather than a scalar.
fn empty_container(trimmed: &str) -> bool {
    let value = trimmed.strip_suffix(',').unwrap_or(trimmed);
    value.ends_with("[]") || value.ends_with("{}")
}

#[cfg(test)]
mod tests;

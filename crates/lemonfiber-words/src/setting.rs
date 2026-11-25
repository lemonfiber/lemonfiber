//! How a value written into a hand-edited settings file reads.

/// A recorded value with the whitespace and surrounding quotes a person might
/// add stripped, its case left alone — so a hand-edited `"https://IP.example"`
/// keeps its path but loses the quotes that would otherwise reach the reader.
#[must_use]
pub fn unquoted(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_owned()
}

/// The same, folded to lower case, so `"on"` and ` on ` both read as `on`.
#[must_use]
pub fn bare(value: &str) -> String {
    unquoted(value).to_ascii_lowercase()
}

/// Whether a recorded setting reads as switched on.
///
/// Generous about spelling because this is a file people edit by hand, and a
/// setting that silently means "no" because it says `yes` rather than `on`
/// would be indistinguishable from lemonfiber ignoring it.
#[must_use]
pub fn reads_as_on(value: &str) -> bool {
    matches!(bare(value).as_str(), "on" | "true" | "yes" | "1")
}

/// Whether a recorded setting reads as switched off.
///
/// The counterpart to [`reads_as_on`] for a setting that also accepts a value:
/// an explicit off switches the feature off, where absence or an affirmative
/// would leave it on.
#[must_use]
pub fn reads_as_off(value: &str) -> bool {
    matches!(bare(value).as_str(), "off" | "false" | "no" | "0" | "")
}

#[cfg(test)]
mod tests;

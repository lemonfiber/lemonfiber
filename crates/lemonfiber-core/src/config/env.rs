//! The environment file, read and written without disturbing it.
//!
//! This is a file the operator edits by hand. It arrives with seventy-eight
//! lines of comments explaining what each setting costs, and those comments are
//! the documentation for the whole stack — a writer that emitted keys and values
//! would delete them the first time anything changed a setting.
//!
//! So the file is held as the lines it is made of. Changing one value rewrites
//! one line; everything else survives byte for byte, including trailing
//! whitespace and whether the last line ends in a newline.

use std::fmt::Write as _;

/// One line of an environment file, as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// A setting, and the exact text it was written as.
    Entry {
        /// The key, trimmed.
        key: String,
        /// The value, as written, with no unquoting.
        value: String,
        /// Anything before the key on the line, such as indentation.
        indent: String,
    },
    /// A comment or blank line, kept verbatim.
    Verbatim(String),
}

/// An environment file: its settings, and everything around them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvFile {
    lines: Vec<Line>,
    /// Whether the text ended with a newline, so rendering can put it back.
    trailing_newline: bool,
}

/// Whether text can be written as part of one line of this file.
///
/// A setting is a line, and rendering writes the key and the value into one with
/// nothing between them and the next. So a value carrying a break does not
/// produce a setting with a break in it — it produces that setting and then
/// however many more the rest of the text spells, which the stack reads as
/// settings the operator wrote.
///
/// Several of the values that reach a settings file are read out of a file one of
/// the containers wrote about itself, so a break here is not a shape only a hand
/// edit could put there. What refuses it is the writer, because that is the one
/// place every source passes through.
#[must_use]
pub(crate) fn is_one_line(text: &str) -> bool {
    !text.contains(['\n', '\r'])
}

impl EnvFile {
    /// Read an environment file.
    ///
    /// Nothing is rejected. A line that is not a setting is kept as it is, which
    /// is what lets an unfamiliar or malformed file round-trip rather than being
    /// silently normalised.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);

        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n').map(Line::parse).collect()
        };

        Self {
            lines,
            trailing_newline,
        }
    }

    /// Render the file back to text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            match line {
                Line::Entry { key, value, indent } => {
                    // `write!` to a String cannot fail; the result is discarded
                    // rather than unwrapped so no panic path exists here.
                    let _ = write!(out, "{indent}{key}={value}");
                }
                Line::Verbatim(text) => out.push_str(text),
            }
        }
        if self.trailing_newline && !self.lines.is_empty() {
            out.push('\n');
        }
        out
    }

    /// The value of a setting, as written.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry { key: k, value, .. } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    /// Set a value, in place where the key already exists.
    ///
    /// Rewriting in place is what keeps a setting underneath the comment that
    /// explains it. A new key is appended, because there is nowhere better to
    /// put it that would not be a guess. The *last* occurrence is rewritten, so
    /// a hand-edited file with a duplicated key changes the one Compose and
    /// [`Self::get`] actually read rather than an earlier one they ignore.
    pub fn set(&mut self, key: &str, value: &str) {
        for line in self.lines.iter_mut().rev() {
            if let Line::Entry {
                key: k, value: v, ..
            } = line
            {
                if k == key {
                    value.clone_into(v);
                    return;
                }
            }
        }

        self.lines.push(Line::Entry {
            key: key.to_owned(),
            value: value.to_owned(),
            indent: String::new(),
        });
        self.trailing_newline = true;
    }

    /// Remove a setting, the last occurrence [`Self::set`] would have rewritten.
    ///
    /// The inverse of appending a key: the one line [`Self::set`] added goes, and
    /// the lines around it — the comments that document the file — stay. A key that
    /// is not there is left as it is, so this is safe to run against a file that
    /// never had the key. What it does not restore is the file-level shape a set may
    /// have changed — appending a key forces a trailing newline that removing it
    /// does not take back — because the change record holds a key and value, not
    /// whether the file ended in a newline before.
    pub fn remove(&mut self, key: &str) {
        let last = self
            .lines
            .iter()
            .rposition(|line| matches!(line, Line::Entry { key: k, .. } if k == key));
        if let Some(index) = last {
            self.lines.remove(index);
        }
    }

    /// Every key the file sets, in the order it sets them.
    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Entry { key, .. } => Some(key.as_str()),
                Line::Verbatim(_) => None,
            })
            .collect()
    }
}

impl Line {
    /// Read one line, treating anything that is not `KEY=value` as verbatim.
    fn parse(text: &str) -> Self {
        let trimmed = text.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            return Self::Verbatim(text.to_owned());
        }

        let Some((before, value)) = text.split_once('=') else {
            return Self::Verbatim(text.to_owned());
        };

        let key = before.trim();
        let looks_like_a_key =
            !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_');
        if !looks_like_a_key {
            return Self::Verbatim(text.to_owned());
        }

        let indent: String = before.chars().take_while(|c| c.is_whitespace()).collect();

        Self::Entry {
            key: key.to_owned(),
            value: value.to_owned(),
            indent,
        }
    }
}

#[cfg(test)]
mod tests;

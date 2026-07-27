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
mod tests {
    use super::EnvFile;

    /// The file this stack ships as its documented starting point.
    const EXAMPLE: &str = include_str!("../../../../assets/media-stack/.env.example");

    #[test]
    fn the_stack_s_own_example_round_trips_byte_for_byte() {
        assert_eq!(EnvFile::parse(EXAMPLE).render(), EXAMPLE);
    }

    #[test]
    fn reads_every_setting_the_example_declares() {
        let file = EnvFile::parse(EXAMPLE);
        assert_eq!(file.keys().len(), 39);
        assert_eq!(file.get("DATA_ROOT"), Some("./data"));
        assert_eq!(file.get("TZ"), Some("Europe/Amsterdam"));
        // Six of the thirty-nine carry a digit. A key pattern of letters and
        // underscores alone would read them as prose and drop them silently.
        assert_eq!(file.get("UN_SONARR_0_URL"), Some(""));
    }

    #[test]
    fn an_empty_value_is_a_value_rather_than_an_absence() {
        let file = EnvFile::parse(EXAMPLE);
        assert_eq!(
            file.get("WIREGUARD_PRIVATE_KEY"),
            Some(""),
            "the key is declared and waiting to be filled in"
        );
        assert_eq!(file.get("NOT_IN_THE_FILE"), None);
    }

    #[test]
    fn changing_a_value_leaves_everything_around_it_alone() {
        let mut file = EnvFile::parse(EXAMPLE);
        file.set("TZ", "Pacific/Auckland");

        let rendered = file.render();
        assert_eq!(
            EnvFile::parse(&rendered).get("TZ"),
            Some("Pacific/Auckland")
        );
        assert_eq!(
            rendered.lines().count(),
            EXAMPLE.lines().count(),
            "no line was added or removed"
        );
        assert!(
            rendered.contains("# The user that owns everything under DATA_ROOT"),
            "the comments explaining the settings survive"
        );
    }

    #[test]
    fn a_setting_stays_under_the_comment_that_explains_it() {
        let text = "# why this matters\nKEY=old\n# something else\nOTHER=x\n";
        let mut file = EnvFile::parse(text);
        file.set("KEY", "new");
        assert_eq!(
            file.render(),
            "# why this matters\nKEY=new\n# something else\nOTHER=x\n"
        );
    }

    #[test]
    fn a_new_setting_is_appended() {
        let mut file = EnvFile::parse("A=1\n");
        file.set("B", "2");
        assert_eq!(file.render(), "A=1\nB=2\n");
    }

    #[test]
    fn a_file_with_no_trailing_newline_keeps_not_having_one() {
        assert_eq!(EnvFile::parse("A=1").render(), "A=1");
        assert_eq!(EnvFile::parse("A=1\n").render(), "A=1\n");
        assert_eq!(EnvFile::parse("").render(), "");
    }

    #[test]
    fn anything_that_is_not_a_setting_is_kept_as_it_was() {
        // A value containing `=`, an indented key, a bare word, a line that is
        // only a comment: none of these should be normalised into something
        // tidier, because tidying is how a hand-edited file loses its meaning.
        let text = "  SPACED=1\nVALUE=a=b\nnot a setting\n#\n\n";
        assert_eq!(EnvFile::parse(text).render(), text);

        let file = EnvFile::parse(text);
        assert_eq!(file.get("SPACED"), Some("1"));
        assert_eq!(file.get("VALUE"), Some("a=b"));
        assert_eq!(file.keys(), vec!["SPACED", "VALUE"]);
    }

    #[test]
    fn a_repeated_key_reads_as_the_last_one_set() {
        // Which is what a shell would do with the same file.
        let file = EnvFile::parse("A=first\nA=second\n");
        assert_eq!(file.get("A"), Some("second"));
    }

    #[test]
    fn a_repeated_key_is_changed_where_it_takes_effect() {
        // The last occurrence is the one the shell and `get` read, so that is the
        // one `set` rewrites — otherwise the change is a silent no-op.
        let mut file = EnvFile::parse("A=first\nA=second\n");
        file.set("A", "only");
        assert_eq!(file.render(), "A=first\nA=only\n");
        assert_eq!(file.get("A"), Some("only"));
    }

    #[test]
    fn a_key_that_is_not_a_key_is_left_alone() {
        let text = "not-a-key=value\n";
        let file = EnvFile::parse(text);
        assert_eq!(file.keys(), Vec::<&str>::new());
        assert_eq!(file.render(), text);
    }
}

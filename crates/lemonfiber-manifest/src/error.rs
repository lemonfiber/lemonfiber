//! What can go wrong reading a manifest.

use thiserror::Error;

use crate::Violation;

/// A manifest could not be read.
///
/// The four refusals are separate variants rather than one "invalid manifest"
/// because they have nothing to do with each other: an unreadable *format*, a
/// stack that needs a newer binary, a word this build has never heard of, and a
/// file whose *contents* are wrong each need a different response from whoever
/// hit it.
#[derive(Debug, Error)]
pub enum Error {
    /// The file is not valid TOML, or does not have the shape of a manifest.
    #[error("the manifest could not be parsed: {0}")]
    Syntax(#[from] toml::de::Error),

    /// The manifest declares a schema generation this build cannot read.
    #[error("the manifest declares schema version {found}, and this build reads {supported:?}")]
    UnsupportedSchema {
        /// The version the manifest declared.
        found: u32,
        /// The versions this build can read.
        supported: Vec<u32>,
    },

    /// The manifest declares something by a name this build does not know.
    ///
    /// Every one of them, because they are found by asking each declaration on its
    /// own rather than by reading the file into types: a reader that stops at the
    /// first unknown word turns adapting a fork into one mistake per run.
    #[error("the manifest declares names this build does not know:{}", each(.0))]
    Unrecognised(
        /// What was declared, and what the name would have had to be.
        Vec<Violation>,
    ),

    /// The stack requires a newer `lemonfiber` than the one running.
    #[error("the stack requires lemonfiber {required} or newer, and this is {running}")]
    BinaryTooOld {
        /// The minimum version the stack declared.
        required: String,
        /// The version actually running.
        running: String,
    },
}

/// Each refusal on its own indented line, so a list of them reads as a list.
fn each(found: &[Violation]) -> String {
    found.iter().fold(String::new(), |mut listed, one| {
        listed.push_str("\n  ");
        listed.push_str(&one.to_string());
        listed
    })
}

#[cfg(test)]
mod tests {
    use super::Error;
    use crate::Violation;

    #[test]
    fn a_syntax_error_keeps_the_parser_s_own_words() {
        let message = toml::from_str::<toml::Value>("= not toml")
            .err()
            .map(|parse| Error::Syntax(parse).to_string())
            .unwrap_or_default();
        assert!(
            message.starts_with("the manifest could not be parsed:"),
            "got: {message}"
        );
    }

    #[test]
    fn an_unsupported_schema_names_both_sides() {
        let message = Error::UnsupportedSchema {
            found: 7,
            supported: vec![1],
        }
        .to_string();
        assert!(
            message.contains('7'),
            "the found version is named: {message}"
        );
        assert!(
            message.contains('1'),
            "the supported set is named: {message}"
        );
    }

    #[test]
    fn an_unrecognised_name_reports_every_one_of_them() {
        let message = Error::Unrecognised(vec![
            Violation {
                location: "service jellyfin".to_owned(),
                message: "api.kind: unknown variant `plex`".to_owned(),
            },
            Violation {
                location: "profile tv".to_owned(),
                message: "protocol: unknown variant `ftp`".to_owned(),
            },
        ])
        .to_string();
        assert!(message.contains("service jellyfin"), "got: {message}");
        assert!(message.contains("`plex`"), "got: {message}");
        assert!(message.contains("profile tv"), "got: {message}");
        assert!(message.contains("`ftp`"), "got: {message}");
    }

    #[test]
    fn an_old_binary_is_told_which_version_it_needs() {
        let message = Error::BinaryTooOld {
            required: "0.4.0".into(),
            running: "0.1.0".into(),
        }
        .to_string();
        assert!(
            message.contains("0.4.0"),
            "names the requirement: {message}"
        );
        assert!(
            message.contains("0.1.0"),
            "names what is running: {message}"
        );
    }
}

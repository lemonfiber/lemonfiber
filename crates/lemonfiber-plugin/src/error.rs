//! What can go wrong reading a plugin manifest.

use thiserror::Error;

use crate::recognising::Violation;

/// A plugin manifest could not be read.
///
/// The three refusals are separate variants rather than one "invalid manifest"
/// because they have nothing to do with each other: an unreadable *format*, a plugin
/// written against a generation this build does not carry, and a word this build has
/// never heard of each need a different response from whoever hit it.
#[derive(Debug, Error)]
pub enum Error {
    /// The file is not valid TOML, or does not have the shape of a manifest.
    #[error("the plugin manifest could not be parsed: {0}")]
    Syntax(#[from] toml::de::Error),

    /// The manifest declares a schema generation this build cannot read.
    #[error(
        "the plugin manifest declares schema version {found}, and this build reads {supported:?}"
    )]
    UnsupportedSchema {
        /// The version the manifest declared.
        found: u32,
        /// The versions this build can read.
        supported: Vec<u32>,
    },

    /// The manifest declares something by a name this build does not know.
    ///
    /// Every one of them, because a third-party manifest is far likelier to carry
    /// several faults than a first-party one, and fixing a manifest one error per run
    /// is a guessing game.
    #[error("the plugin manifest declares names this build does not know:{}", each(.0))]
    Unrecognised(
        /// What was declared, and what the name would have had to be.
        Vec<Violation>,
    ),
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
    use crate::recognising::Violation;

    #[test]
    fn a_syntax_error_keeps_the_parser_s_own_words() {
        let message = toml::from_str::<toml::Value>("= not toml")
            .err()
            .map(|parse| Error::Syntax(parse).to_string())
            .unwrap_or_default();
        assert!(
            message.starts_with("the plugin manifest could not be parsed:"),
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
                location: "service komga".to_owned(),
                message: "criticality: unknown variant `critical`".to_owned(),
            },
            Violation {
                location: "service komga".to_owned(),
                message: "bind: unknown variant `wan`".to_owned(),
            },
        ])
        .to_string();
        assert!(message.contains("`critical`"), "got: {message}");
        assert!(message.contains("`wan`"), "got: {message}");
    }
}

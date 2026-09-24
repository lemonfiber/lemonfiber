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

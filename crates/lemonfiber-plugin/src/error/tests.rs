use super::Error;
use crate::conforming::Violation;

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
fn a_nonconforming_manifest_reports_every_fault_in_it() {
    let message = Error::Nonconforming(vec![
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

use lemonfiber_manifest::KeySource;

use super::reader;

/// What the reader for `source` takes out of `text`, where there is one.
fn read(source: KeySource, text: &str) -> Option<String> {
    reader(source).and_then(|read| read(text))
}

#[test]
fn a_yaml_key_is_the_one_under_auth_and_not_one_under_another_section() {
    let written = "analytics:\n  apikey: not-this-one\nauth:\n  type: null\n  - stray\n  apikey: 'own-key'\nsonarr:\n  apikey: someone-elses\n";

    assert_eq!(
        read(KeySource::ConfigYaml, written).as_deref(),
        Some("own-key")
    );
    assert_eq!(
        read(KeySource::ConfigYaml, "sonarr:\n  apikey: someone-elses\n"),
        None
    );
    assert_eq!(
        read(KeySource::ConfigYaml, "# auth:\n  apikey: commented\n"),
        None
    );
    assert_eq!(read(KeySource::ConfigYaml, "auth:\n  apikey: ''\n"), None);
}

#[test]
fn an_ini_key_is_the_api_key_entry_alone() {
    let written = "[misc]\nnzb_key = not-this-one\n# api_key = commented\napi_key = own-key\n";

    assert_eq!(
        read(KeySource::ConfigIni, written).as_deref(),
        Some("own-key")
    );
    assert_eq!(read(KeySource::ConfigIni, "[misc]\napi_key = \n"), None);
    assert_eq!(read(KeySource::ConfigIni, "[misc]\nnzb_key = k\n"), None);
}

#[test]
fn xml_and_json_keys_are_read_as_the_curators_and_the_request_services_are() {
    assert_eq!(
        read(
            KeySource::ConfigXml,
            "<Config><ApiKey>own-key</ApiKey></Config>"
        )
        .as_deref(),
        Some("own-key")
    );
    assert_eq!(
        read(KeySource::ConfigJson, r#"{"main":{"apiKey":"own-key"}}"#).as_deref(),
        Some("own-key")
    );
}

#[test]
fn a_key_read_over_the_api_minted_or_absent_has_no_file_reader() {
    for source in [
        KeySource::ApiSettings,
        KeySource::Generated,
        KeySource::None,
    ] {
        assert!(reader(source).is_none(), "{source:?}");
    }
}

use lemonfiber_manifest::KeySource;

use lemonfiber_manifest::container::Format;

use super::{at, reader};

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
    assert_eq!(
        read(
            KeySource::ConfigYaml,
            "auth:\n  no entry here\n  apikey: own-key\n"
        )
        .as_deref(),
        Some("own-key")
    );
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
fn an_xml_key_is_trimmed_and_one_not_generated_yet_is_none() {
    assert_eq!(
        read(
            KeySource::ConfigXml,
            "<Config><ApiKey>\n    a1b2c3d4e5\n  </ApiKey></Config>"
        )
        .as_deref(),
        Some("a1b2c3d4e5")
    );
    for unwritten in [
        "<Config><ApiKey></ApiKey></Config>",
        "<Config><ApiKey>   </ApiKey></Config>",
        "<Config><Port>8989</Port></Config>",
        "<Config><ApiKey>café☃clé</ApiKey></Config>",
        "<Config><ApiKey>unclosed",
        "<Config><ApiKey>${WIREGUARD_PRIVATE_KEY}</ApiKey></Config>",
    ] {
        assert_eq!(read(KeySource::ConfigXml, unwritten), None, "{unwritten}");
    }
}

#[test]
fn a_json_key_not_written_yet_or_unreadable_is_none() {
    for unwritten in [
        r#"{"main":{"apiKey":""}}"#,
        r#"{"main":{}}"#,
        r#"{"main":{"apiKey":7}}"#,
        "{}",
        "not json at all",
    ] {
        assert_eq!(read(KeySource::ConfigJson, unwritten), None, "{unwritten}");
    }
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

#[test]
fn a_dotted_path_is_followed_through_each_format() {
    assert_eq!(
        at(
            Format::Xml,
            "Settings.Auth.Key",
            "<Settings><Port>1</Port><Auth><Key> k1 </Key></Auth></Settings>"
        )
        .as_deref(),
        Some("k1")
    );
    assert_eq!(
        at(Format::Json, "a.b.c", r#"{"a":{"b":{"c":"k2"},"c":"no"}}"#).as_deref(),
        Some("k2")
    );
    assert_eq!(
        at(
            Format::Yaml,
            "server.auth.token",
            "server:\n  port: 1\n  auth:\n    token: \"k3\"\nauth:\n  token: no\n"
        )
        .as_deref(),
        Some("k3")
    );
    assert_eq!(
        at(
            Format::Ini,
            "server.key",
            "key = no\n[other]\nkey = no\n[server]\nkey = k4\n"
        )
        .as_deref(),
        Some("k4")
    );
}

#[test]
fn a_path_the_file_does_not_hold_is_none() {
    assert_eq!(
        at(
            Format::Xml,
            "Config.Missing",
            "<Config><ApiKey>k</ApiKey></Config>"
        ),
        None
    );
    assert_eq!(
        at(
            Format::Json,
            "main.apiKey.deeper",
            r#"{"main":{"apiKey":"k"}}"#
        ),
        None
    );
    assert_eq!(
        at(
            Format::Yaml,
            "auth.apikey",
            "other:\n  auth:\n    apikey: k\n"
        ),
        None
    );
    assert_eq!(
        at(Format::Ini, "misc.api_key", "[servers]\napi_key = k\n"),
        None
    );
    assert_eq!(at(Format::Ini, "api_key", "api_key = k\n"), None);
}

#[test]
fn a_path_with_an_empty_segment_reads_nothing() {
    for key in ["", ".", "main.", ".apiKey", "main..apiKey"] {
        assert_eq!(
            at(Format::Json, key, r#"{"main":{"apiKey":"k"}}"#),
            None,
            "{key}"
        );
    }
}

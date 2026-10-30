use lemonfiber_manifest::container::Format;

use super::at;

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
            "; written by the service\nkey = no\n[other]\nkey = no\n[server]\nkey = k4\n"
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
    assert_eq!(at(Format::Xml, "Config.ApiKey", "<Config><ApiKey>k"), None);
    assert_eq!(
        at(Format::Json, "main.apiKey", r#"{"main":{"apiKey":7}}"#),
        None
    );
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

#[test]
fn a_yaml_key_is_read_past_comments_list_items_bare_lines_and_closed_sections() {
    let written = "analytics:\n  nested:\n    apikey: not-this-one\n# auth:\n- stray\nauth:\n  \
                   no entry here\n  apikey: 'own-key'\n";
    assert_eq!(
        at(Format::Yaml, "auth.apikey", written).as_deref(),
        Some("own-key")
    );
}

use super::{joined, Locations};

const BASE: &str = "https://house.local:8920";

#[test]
fn a_plain_path_is_joined_onto_its_base_as_written() {
    for path in [
        "/Items/0123456789abcdef0123456789abcdef",
        "/Videos/a/master.m3u8?MediaSourceId=a&VideoCodec=h264,hevc",
        "/",
    ] {
        assert_eq!(joined(BASE, path), Some(format!("{BASE}{path}")), "{path}");
    }
}

/// Inputs URL parsers read differently, each refused rather than read one way.
#[test]
fn a_path_another_reader_could_take_elsewhere_or_rewrite_is_refused() {
    for path in [
        "//elsewhere.example/x",
        "/\\elsewhere.example/x",
        "\\\\elsewhere.example/x",
        "/\\/elsewhere.example",
        "http://elsewhere.example/x",
        "elsewhere.example/x",
        "/a/../b",
        "/a/./b",
        "/a/%2e%2e/b",
        "/a/%2E/b",
        "/a/.%2e/b",
        "/a\tb",
        "/a\nb",
        "/a\rb",
        "/a b",
        "/a\u{ff0e}\u{ff0e}/b",
        "/a/\u{2024}\u{2024}/b",
        "/a{b}",
        "/a\"b",
        "/a<b>",
        "/a#b?c",
        "/a\\b",
        " /a",
        "/a\u{0}",
    ] {
        assert_eq!(joined(BASE, path), None, "{path:?}");
    }
}

#[test]
fn a_base_carrying_a_path_query_or_credential_joins_nothing() {
    for base in [
        "https://house.local:8920/under",
        "https://house.local:8920/?x=1",
        "not a url",
    ] {
        assert_eq!(joined(base, "/a"), None, "{base}");
    }
    assert_eq!(
        joined("https://user@house.local:8920", "/a"),
        None,
        "a base naming a user"
    );
}

#[test]
fn a_location_is_its_template_on_the_household_address_with_the_item_in_place() {
    assert_eq!(
        Locations::located("{base}/films/{item}/poster", BASE, "f1"),
        Some(format!("{BASE}/films/f1/poster"))
    );
    assert_eq!(
        Locations::located("https://elsewhere.example/{item}", BASE, "f1"),
        None
    );
    assert_eq!(
        Locations::located("{base}/films/{item}", BASE, "../x"),
        None
    );
}

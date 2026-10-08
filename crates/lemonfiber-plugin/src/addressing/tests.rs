//! A call's address, built one way, and its text read one way.

use super::{address, carries, named, pieces, queried, unplain, Piece, Toward, Unaddressed};

#[test]
fn a_plain_absolute_path_with_a_query_is_one() {
    for path in [
        "/",
        "/api/v1/login",
        "/:/prefs",
        "/api/claim?token={{code}}&kind=x",
        "/a.b/c..d/.well-known/x",
        "/library/sections?type=1",
    ] {
        assert_eq!(unplain(path), None, "{path}");
    }
}

#[test]
fn every_way_a_path_could_name_another_host_is_refused_naming_it() {
    for (path, why) in [
        ("api/x", "exactly one `/`"),
        ("", "exactly one `/`"),
        ("//elsewhere.example/x", "exactly one `/`"),
        ("@elsewhere.example/x", "exactly one `/`"),
        ("/x@elsewhere.example", "an `@`"),
        ("/x\\..\\y", "a backslash"),
        ("/x#frag", "a `#`"),
        ("/x?y=1#frag", "a `#`"),
        ("/a b", "whitespace"),
        ("/a\tb", "whitespace"),
        ("/a\u{7}b", "control character"),
        ("/%2fevil", "`/` percent-encoded"),
        ("/%2Fevil", "`/` percent-encoded"),
        ("/%5cevil", "a backslash percent-encoded"),
        ("/%40evil", "`@` percent-encoded"),
        ("/%23evil", "`#` percent-encoded"),
        ("/%2e%2e/x", "`.` percent-encoded"),
        ("/a/../b", "`..` segment"),
        ("/a/./b", "`.` or `..` segment"),
        ("/a/..", "`..` segment"),
    ] {
        assert!(
            unplain(path).is_some_and(|said| said.contains(why)),
            "{path:?}: {:?}",
            unplain(path)
        );
    }
}

/// Each query value as the validator sets it: as written.
fn written(value: &str) -> String {
    value.to_owned()
}

#[test]
fn a_call_to_the_stack_is_addressed_on_this_machine_at_its_port() {
    let built = address("/api/claim?token=x&kind=y", Toward::Stack(8096), &written);
    assert_eq!(
        built.map(String::from),
        Ok("http://127.0.0.1:8096/api/claim?token=x&kind=y".to_owned())
    );
}

#[test]
fn a_call_outside_is_addressed_over_https_on_its_own_port() {
    let built = address(
        "/v1/register",
        Toward::Outside("metadata.example.org"),
        &written,
    );
    assert_eq!(
        built.map(String::from),
        Ok("https://metadata.example.org/v1/register".to_owned())
    );
}

/// Only a query value goes through what is given for it; the names and the path stay
/// as written.
#[test]
fn every_query_value_and_nothing_else_is_put_through_what_sets_it() {
    let built = address("/a/b?one=x&two&three=y", Toward::Stack(1), &|value| {
        format!("<{value}>")
    });
    assert_eq!(
        built.map(String::from),
        Ok("http://127.0.0.1:1/a/b?one=%3Cx%3E&two&three=%3Cy%3E".to_owned())
    );
    assert_eq!(
        address("/a", Toward::Stack(1), &|_| "never".to_owned()).map(String::from),
        Ok("http://127.0.0.1:1/a".to_owned())
    );
}

#[test]
fn a_path_not_plain_is_refused_as_a_path() {
    for path in [
        "//elsewhere.example/x",
        "/x@elsewhere.example",
        "/x\\y",
        "/%2e%2e/x",
    ] {
        assert!(
            matches!(
                address(path, Toward::Outside("api.example.org"), &written),
                Err(Unaddressed::Path(_))
            ),
            "{path}"
        );
    }
}

/// Every host an address would carry spelled another way, or not at all, is refused
/// rather than followed: the address would otherwise reach somewhere the manifest does
/// not name.
#[test]
fn a_host_an_address_carries_otherwise_is_refused_saying_what_it_becomes() {
    let mut wrong = Vec::new();
    for (host, said) in [
        ("0x7f.1", "as \"127.0.0.1\""),
        ("0177.0.0.1", "as \"127.0.0.1\""),
        ("2130706433", "as \"127.0.0.1\""),
        ("API.example.org", "as \"api.example.org\""),
        ("bücher.example", "as \"xn--bcher-kva.example\""),
        ("evil.example\\@api.example.org", "cannot carry"),
        ("user@api.example.org", "cannot carry"),
        ("api.example.org:8443", "as \"api.example.org\""),
        ("api.example.org/x", "cannot carry"),
        ("foo.123", "cannot carry"),
        ("[fe80::1%25eth0]", "cannot carry"),
        ("api%2eexample.org", "as \"api.example.org\""),
        ("", "cannot carry"),
    ] {
        let built = address("/x", Toward::Outside(host), &written);
        if !matches!(&built, Err(Unaddressed::Host(why)) if why.contains(said)) {
            wrong.push(format!("{host:?}: {built:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_substitution_runs_from_its_braces_to_the_next_closing_ones() {
    assert_eq!(
        pieces("Bearer {{token}}"),
        vec![Piece::Written("Bearer "), Piece::Named("token")]
    );
    assert_eq!(
        pieces("{{ a }}-{{b}}"),
        vec![Piece::Named(" a "), Piece::Written("-"), Piece::Named("b")]
    );
    assert_eq!(
        pieces("{{a{{b}}"),
        vec![Piece::Written("{{"), Piece::Written("a"), Piece::Named("b")]
    );
    assert_eq!(
        pieces("{{open"),
        vec![Piece::Written("{{"), Piece::Written("open")]
    );
    assert_eq!(pieces(""), Vec::new());
    assert_eq!(
        named("x {{ a }} {{b}} {{c").collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(Piece::Written("x").name(), None);
}

#[test]
fn only_a_query_value_is_read_for_what_it_carries() {
    assert_eq!(
        queried("/{{p}}?{{n}}=1&k={{v}}&bare&w=a{{x}}b").collect::<Vec<_>>(),
        vec!["v", "x"]
    );
    assert_eq!(queried("/no/query").count(), 0);
}

/// What is about to be sent goes exactly toward its destination: its scheme, its host,
/// its port, and nothing in front of the host or after the query.
#[test]
fn a_written_address_is_held_to_exactly_its_destination() {
    assert!(carries("http://127.0.0.1:8989/x?a=1", Toward::Stack(8989)));
    assert!(carries("https://plex.tv/x", Toward::Outside("plex.tv")));
    for (written, toward) in [
        ("http://127.0.0.1:8990/x", Toward::Stack(8989)),
        ("http://127.0.0.1/x", Toward::Stack(8989)),
        ("https://127.0.0.1:8989/x", Toward::Stack(8989)),
        ("http://evil.example:8989/x", Toward::Stack(8989)),
        ("https://plex.tv:8443/x", Toward::Outside("plex.tv")),
        ("http://plex.tv/x", Toward::Outside("plex.tv")),
        ("https://evil.example/x", Toward::Outside("plex.tv")),
        ("https://user@plex.tv/x", Toward::Outside("plex.tv")),
        ("https://user:pass@plex.tv/x", Toward::Outside("plex.tv")),
        ("https://:pass@plex.tv/x", Toward::Outside("plex.tv")),
        ("https://plex.tv/x#frag", Toward::Outside("plex.tv")),
        ("not an address", Toward::Outside("plex.tv")),
    ] {
        assert!(!carries(written, toward), "{written}");
    }
}

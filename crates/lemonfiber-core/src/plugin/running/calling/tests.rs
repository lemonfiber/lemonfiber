use std::collections::BTreeMap;

use lemonfiber_plugin::{Recipe, StepCall};

use super::{request, Built, Unbuilt, Whither};
use crate::plugin::running::bounding::Bounds;
use crate::ports::http::{Method, Request};

/// A call as a manifest writes one.
fn call(method: &str, to: &str, path: &str) -> StepCall {
    StepCall {
        method: method.to_owned(),
        to: to.to_owned(),
        path: path.to_owned(),
        headers: None,
        body: None,
    }
}

/// What every test's run holds.
fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("token".to_owned(), "abc".to_owned()),
        ("code".to_owned(), "a b&c=d/é".to_owned()),
    ])
}

/// A recipe whose pairs carry every value of [`values`] to each destination these tests
/// call, and whose `key` is the credential lemonfiber holds for `sonarr`.
fn paired() -> Recipe {
    let pairs: String = ["komga", "plex.tv", "sonarr", "radarr"]
        .iter()
        .flat_map(|to| {
            ["token", "code", "key"]
                .map(|value| format!("[[pair]]\nvalue = \"{value}\"\nto = \"{to}\"\n"))
        })
        .collect();
    toml::from_str(&format!(
        "id = \"r\"\ntitle = \"R\"\nwhy = \"Held\"\n\
         [[input]]\nname = \"key\"\norigin = \"credential-store\"\nof = \"sonarr\"\n{pairs}"
    ))
    .unwrap_or_else(|_| Recipe {
        id: "unread".to_owned(),
        title: String::new(),
        why: String::new(),
        on: lemonfiber_plugin::On::Install,
        inputs: Vec::new(),
        steps: Vec::new(),
        pairs: Vec::new(),
    })
}

/// Every pair outside the stack these tests carry, approved.
fn approved() -> Vec<String> {
    vec!["token@plex.tv".to_owned(), "code@plex.tv".to_owned()]
}

/// The request one call makes, under every pair and approval these tests declare.
fn made(
    call: &StepCall,
    whither: Whither,
    values: &BTreeMap<String, String>,
) -> Result<Request, Unbuilt> {
    let recipe = paired();
    let approved = approved();
    request(call, whither, values, &Bounds::of(&recipe, &approved, &[])).map(|built| built.request)
}

#[test]
fn a_service_in_the_stack_is_called_on_this_machine_at_the_port_it_publishes() {
    let built = made(
        &call("POST", "komga", "/api/v1/login"),
        Whither::Stack(25600),
        &values(),
    );
    assert_eq!(
        built.map(|one| (one.method, one.url, one.pinned.is_none())),
        Ok((
            Method::Post,
            "http://127.0.0.1:25600/api/v1/login".to_owned(),
            true
        ))
    );
}

#[test]
fn a_host_outside_is_called_over_https_by_its_name() {
    let built = made(
        &call("PATCH", "plex.tv", "/api/claim"),
        Whither::Outside,
        &values(),
    );
    assert_eq!(
        built.map(|one| (one.method, one.url)),
        Ok((Method::Patch, "https://plex.tv/api/claim".to_owned()))
    );
}

#[test]
fn a_header_and_a_body_carry_a_value_as_it_is() {
    let mut written = call("POST", "komga", "/x");
    written.headers = Some(BTreeMap::from([(
        "Authorization".to_owned(),
        "Bearer {{token}}".to_owned(),
    )]));
    written.body = Some("{\"code\":\"{{ code }}\"}".to_owned());
    let built = made(&written, Whither::Stack(1), &values());
    assert_eq!(
        built.as_ref().map(|one| one.headers.clone()),
        Ok(vec![("Authorization".to_owned(), "Bearer abc".to_owned())])
    );
    assert_eq!(
        built.map(|one| one.body),
        Ok(Some("{\"code\":\"a b&c=d/é\"}".to_owned()))
    );
}

/// A query value is percent-encoded, so what it carries can never end the parameter
/// it stands in; the path and the query's names are left as written.
#[test]
fn a_query_value_carries_a_value_percent_encoded() {
    let built = made(
        &call("GET", "plex.tv", "/a/claim?token={{code}}&kind=x&flag"),
        Whither::Outside,
        &values(),
    );
    assert_eq!(
        built.map(|one| one.url),
        Ok("https://plex.tv/a/claim?token=a%20b%26c%3Dd%2F%C3%A9&kind=x&flag".to_owned())
    );
}

/// A name nothing holds, and braces that never close, are carried as written, and only
/// what was put in is said to be carried.
#[test]
fn a_substitution_nothing_holds_is_carried_as_it_was_written() {
    let mut written = call("POST", "komga", "/x?a={{gone}}");
    written.body = Some("{{missing}} and {{token}} and {{open".to_owned());
    let recipe = paired();
    let built = request(
        &written,
        Whither::Stack(1),
        &values(),
        &Bounds::of(&recipe, &[], &[]),
    );
    assert_eq!(
        built.map(|Built { request, carried }| (request.url, request.body, carried)),
        Ok((
            "http://127.0.0.1:1/x?a={{gone}}".to_owned(),
            Some("{{missing}} and abc and {{open".to_owned()),
            [("token".to_owned(), "abc".to_owned())].into()
        ))
    );
}

#[test]
fn a_method_the_port_cannot_carry_makes_no_request() {
    assert_eq!(
        made(&call("TRACE", "komga", "/x"), Whither::Stack(1), &values()),
        Err(Unbuilt::Method)
    );
}

/// A path reading the manifest refuses is refused here by the same function, and no
/// path ever moves the address off its destination.
#[test]
fn no_path_moves_the_address_off_its_destination() {
    for path in [
        "//evil.example/x",
        "@evil.example/x",
        "/x@evil.example",
        "/\\\\evil.example/x",
    ] {
        for whither in [Whither::Stack(8989), Whither::Outside] {
            let built = made(&call("GET", "plex.tv", path), whither, &values());
            assert!(
                matches!(&built, Err(Unbuilt::Unaddressed(why)) if why.contains("not a plain absolute path")),
                "{path}: {built:?}"
            );
        }
    }
}

/// A value substituted into a query cannot add a parameter, a fragment or a host.
#[test]
fn a_substituted_value_cannot_end_the_query_it_stands_in() {
    let values = BTreeMap::from([("code".to_owned(), "x&admin=1#@evil.example/".to_owned())]);
    let built = made(
        &call("GET", "plex.tv", "/claim?token={{code}}"),
        Whither::Outside,
        &values,
    );
    let parsed = built.ok().and_then(|one| url::Url::parse(&one.url).ok());
    assert_eq!(
        parsed.as_ref().and_then(url::Url::host_str),
        Some("plex.tv")
    );
    assert_eq!(parsed.as_ref().and_then(url::Url::fragment), None);
    assert_eq!(
        parsed.map(|one| one
            .query_pairs()
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>()),
        Some(vec![(
            "token".to_owned(),
            "x&admin=1#@evil.example/".to_owned()
        )])
    );
}

/// A destination an address would carry as something else, or not at all, makes no
/// request, saying what the address made of it.
#[test]
fn a_destination_that_is_not_carried_as_itself_makes_no_request() {
    for (to, said) in [("bad host", "cannot carry"), ("0x7f.1", "as \"127.0.0.1\"")] {
        let built = made(&call("GET", to, "/x"), Whither::Outside, &values());
        assert!(
            matches!(&built, Err(Unbuilt::Unaddressed(why)) if why.contains(said)),
            "{to}: {built:?}"
        );
    }
}

/// Every value a call would carry is held to where it may go before anything is built:
/// a credential goes back only to its own service, a value only where a pair names, and
/// outside only where this act approved it. Each refusal names the value and where it
/// was going, never what it holds.
#[test]
fn a_value_that_may_not_go_where_the_call_goes_withholds_the_call() {
    let mut values = values();
    values.insert("key".to_owned(), "s3cret".to_owned());
    let recipe = paired();
    let approved = approved();
    let bounds = Bounds::of(&recipe, &approved, &[]);
    for (to, whither, path, said) in [
        (
            "radarr",
            Whither::Stack(7878),
            "/x?k={{key}}",
            "key is the credential lemonfiber holds for sonarr",
        ),
        (
            "plex.tv",
            Whither::Outside,
            "/x?k={{key}}",
            "key is the credential lemonfiber holds for sonarr",
        ),
        (
            "lidarr",
            Whither::Stack(8686),
            "/x?t={{token}}",
            "no pair of the recipe declares",
        ),
        (
            "radarr",
            Whither::Stack(7878),
            "/x?n={{code}}&k={{key}}",
            "key is the credential lemonfiber holds for sonarr",
        ),
    ] {
        let built = request(&call("GET", to, path), whither, &values, &bounds);
        assert!(
            matches!(&built, Err(Unbuilt::Withheld(why)) if why.contains(said) && why.contains(to) && !why.contains("s3cret")),
            "{to} {path}: {built:?}"
        );
    }
    let unapproved = Bounds::of(&recipe, &[], &[]);
    let built = request(
        &call("GET", "plex.tv", "/x?t={{token}}"),
        Whither::Outside,
        &values,
        &unapproved,
    );
    assert!(
        matches!(&built, Err(Unbuilt::Withheld(why)) if why.contains("token@plex.tv was not approved")),
        "{built:?}"
    );
    let home = request(
        &call("GET", "sonarr", "/x?k={{key}}"),
        Whither::Stack(8989),
        &values,
        &bounds,
    );
    assert_eq!(
        home.map(|built| built.request.url),
        Ok("http://127.0.0.1:8989/x?k=s3cret".to_owned())
    );
}

/// A plugin with one recipe whose one step calls `path` at `to`, written as TOML
/// literal strings so every byte reaches the reader as written.
fn probing(to: &str, path: &str) -> String {
    format!(
        r#"schema_version = 1

[plugin]
id          = "probe"
name        = "Probe"
version     = "1.0.0"
description = "Calls one address"
without_it  = "Nothing is called"
upstream    = "https://example.org/probe"
license     = "MIT"
forms       = ["library"]

[[recipe]]
id    = "probe"
title = "Probe"
why   = "To see where a call goes"

[[recipe.step]]
id   = "probe"
call = {{ method = "GET", to = '{to}', path = '{path}' }}

[requires]
capabilities = ["recipe.run"]
"#
    )
}

/// Whether reading the manifest lets this step's call through: no refusal at the step.
fn passes_reading(to: &str, path: &str) -> bool {
    lemonfiber_plugin::Manifest::from_toml(&probing(to, path)).is_ok_and(|manifest| {
        !lemonfiber_plugin::refusals(&manifest, &[])
            .iter()
            .any(|refused| refused.location.starts_with("recipe probe.step probe.call"))
    })
}

/// Reading a manifest and making its call build the address with one function, and
/// this holds them to it with targets chosen to make two readers disagree: a user part,
/// backslashes, a doubled slash, separators written percent-encoded, an international
/// name and its punycode, an IPv6 zone, a trailing dot, numeric hosts written in
/// decimal, octal and hexadecimal, and a numeric last label. Whatever reading lets
/// through, the call is built for and sends to exactly that host on https's own port;
/// whatever the call builds goes nowhere else. A redirect is not a target here: the
/// transport follows none, and a `3xx` is judged as an answer.
#[test]
fn reading_and_calling_agree_on_where_every_call_goes() {
    let hosts = [
        "api.example.org",
        "xn--bcher-kva.example",
        "bücher.example",
        "API.example.org",
        "api.example.org.",
        "user@api.example.org",
        "api.example.org@evil.example",
        "evil.example\\api.example.org",
        "api.example.org%2fevil.example",
        "api%2eexample.org",
        "[fe80::1%25en0]",
        "2130706433",
        "0177.0.0.1",
        "0x7f.1",
        "foo.123",
        "api.example.org:8443",
    ];
    let paths = [
        "/x",
        "/x?q=1",
        "//evil.example/x",
        "/x@evil.example",
        "/\\evil.example/x",
        "/%2fevil.example",
        "/%2e%2e/x",
        "/x#@evil.example",
        "/a/../b",
    ];
    let recipe = paired();
    let bounds = Bounds::of(&recipe, &[], &[]);
    for to in hosts {
        for path in paths {
            let read = passes_reading(to, path);
            let built = request(
                &call("GET", to, path),
                Whither::Outside,
                &BTreeMap::new(),
                &bounds,
            );
            let landed = built
                .as_ref()
                .ok()
                .and_then(|one| url::Url::parse(&one.request.url).ok());
            let exactly = landed.as_ref().is_some_and(|url| {
                url.scheme() == "https"
                    && url.host_str() == Some(to)
                    && url.port().is_none()
                    && url.username().is_empty()
                    && url.fragment().is_none()
            });
            assert!(
                !read || exactly,
                "reading passed {to:?} {path:?}, the call built {built:?}"
            );
            assert!(
                built.is_err() || exactly,
                "{to:?} {path:?} was built for {landed:?}"
            );
        }
    }
    assert!(
        passes_reading("api.example.org", "/x"),
        "the probe itself reads clean"
    );
}

/// What is handed to the transport is held to its destination once more, so nothing
/// between building a call and sending it can move it.
#[test]
fn a_request_about_to_be_sent_is_held_to_exactly_its_destination() {
    let sent = |url: &str| Request {
        method: Method::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    };
    assert!(super::sending(
        sent("http://127.0.0.1:8989/x"),
        Whither::Stack(8989),
        "sonarr"
    )
    .is_ok());
    assert!(super::sending(sent("https://plex.tv/x?a=1"), Whither::Outside, "plex.tv").is_ok());
    for (url, whither) in [
        ("http://127.0.0.1:8990/x", Whither::Stack(8989)),
        ("http://evil.example:8989/x", Whither::Stack(8989)),
        ("https://127.0.0.1:8989/x", Whither::Stack(8989)),
        ("https://evil.example/x", Whither::Outside),
        ("https://plex.tv:8443/x", Whither::Outside),
        ("http://plex.tv/x", Whither::Outside),
        ("https://user@plex.tv/x", Whither::Outside),
        ("https://plex.tv/x#frag", Whither::Outside),
        ("not an address", Whither::Outside),
    ] {
        assert!(
            super::sending(sent(url), whither, "plex.tv").is_err_and(|why| why.contains(url)),
            "{url}"
        );
    }
}

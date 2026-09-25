use super::{kept, line, Recording, KEPT};
use crate::ports::http::{Http, Method, Request};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;

/// The fixture hands back a shared handle and a decorator wraps what it is
/// given, so the handle is what gets wrapped. A newtype here rather than an
/// implementation on `Arc` in the port, because this is a fact about how the
/// fixture is built and not about what a transport is.
struct Shared(std::sync::Arc<Fake>);

#[async_trait::async_trait]
impl Http for Shared {
    async fn send(
        &self,
        request: &Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        self.0.send(request).await
    }
}

/// A request carrying everything a real one does, credential included.
fn asking() -> Request {
    Request {
        method: Method::Get,
        url: "https://indexer.example/api?apikey=the-indexer-key&q=something".to_owned(),
        headers: vec![("X-Api-Key".to_owned(), "the-indexer-key".to_owned())],
        body: Some("{\"password\":\"hunter2\"}".to_owned()),
    }
}

/// The line says where it went and what came back, and nothing else.
///
/// The claim this is bought for. A record holding the credential a service
/// authenticates with would be the thing the feature exists to prevent: an
/// operator who switched every outbound request off would be left with a file
/// full of their own keys.
#[test]
fn what_is_written_down_is_where_it_went_and_not_what_it_carried() {
    let said = line(1_700_000_000, &asking(), Some(200));

    assert!(
        said.contains("indexer.example"),
        "where the request went did not survive"
    );
    assert!(said.contains("200"), "what answered did not survive");
    assert!(
        !said.contains("the-indexer-key"),
        "the indexer key survived into the record"
    );
    assert!(
        !said.contains("hunter2"),
        "what the request carried survived into the record"
    );
    assert!(
        !said.contains("X-Api-Key"),
        "the header the credential travels in was named in the record"
    );
    assert!(
        !said.contains("q=something"),
        "what was asked for survived into the record"
    );
}

/// A login written in front of a host is not written into the record.
///
/// Nothing in this stack hands one out — an indexer of the Torznab and Newznab
/// families authenticates by a query parameter, which is why the key is a
/// setting of its own — but an operator whose indexer sits behind a proxy that
/// asks for a login can write one into `INDEXER_URL`, and the client this stack
/// sends with will use it. The same address is scrubbed where it is *shown*, on
/// the outbound listing; a file the operator is invited to read to check that
/// listing is the last place it may survive.
///
/// The account keeps its name. An operator whose login is refused needs to see
/// which one it was, and a username is not what the URI syntax calls a password.
#[test]
fn a_login_written_in_front_of_a_host_is_not_written_down() {
    // Assembled rather than written out: a run that reads as a real password in
    // this source is a secret scanner's finding for as long as the commit exists.
    let password = ["hunter", "2"].concat();
    for url in [
        format!("https://someone:{password}@indexer.example/api"),
        format!("https://someone:{password}@indexer.example/api?t=search"),
    ] {
        let asked = Request {
            method: Method::Get,
            url,
            headers: Vec::new(),
            body: None,
        };
        let said = line(1_700_000_000, &asked, Some(200));

        assert!(
            !said.contains(&password),
            "the operator's login survived into the record"
        );
        assert!(said.contains("someone"), "the account was not named");
        assert!(
            said.contains("indexer.example"),
            "where the request went did not survive"
        );
    }
}

/// A request nothing answered is recorded as one.
///
/// The absence is the interesting half: an operator checking what left this
/// machine is owed the attempt as well as the answer, and a record of only what
/// succeeded would be a record of a quieter machine than the real one.
#[test]
fn a_request_nothing_answered_is_written_down_as_that() {
    let said = line(1, &asking(), None);
    assert!(
        said.contains("nothing answered"),
        "a request nothing answered was not written down as one"
    );
}

/// A query is withheld whether or not it holds a credential, and an address
/// carrying none is written exactly as it was.
///
/// Withheld rather than cut away: what was asked for is the operator's business
/// and not this file's, and a line that dropped the question mark with it would
/// describe a request nobody made.
#[test]
fn the_part_that_says_what_was_asked_for_is_taken_off() {
    // Not quoted either, for the same reason as above: which parameter of a query
    // holds a key belongs to whoever wrote the service, so a recorded line is
    // treated as though one of them does.
    let said = line(1, &at("https://a.example/x?y=z"), Some(200));
    assert!(
        said.contains("https://a.example/x?"),
        "the address and the fact a query was sent did not both survive"
    );
    assert!(!said.contains("y=z"), "the query survived into the record");

    let plain = line(1, &at("https://a.example/x"), Some(200));
    assert!(
        plain.contains("https://a.example/x "),
        "an address carrying no query was not written exactly as it was"
    );
}

/// A GET of one address, for the tests that care about the address alone.
fn at(url: &str) -> Request {
    Request {
        method: Method::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
    }
}

/// The record is bounded, and it is the oldest that goes.
#[test]
fn the_record_keeps_what_is_recent_rather_than_growing_for_ever() {
    let existing = (0..KEPT + 10)
        .map(|n| format!("line {n}"))
        .collect::<Vec<String>>()
        .join("\n");
    let after = kept(&existing, "the newest");

    let lines: Vec<&str> = after.lines().collect();
    assert_eq!(lines.len(), KEPT);
    assert_eq!(lines.last(), Some(&"the newest"));
    assert!(!after.contains("line 0\n"), "the oldest went");
}

/// What went out is written down where the run was told to write it.
///
/// The other half of the pair below, and the half that had never been driven from
/// here: the module's own tests watched a run with nowhere to write and never
/// watched one that had somewhere. A decorator is generic over what it wraps, so
/// "somewhere else drives it" is not the same as this being measured — each
/// instantiation is counted on its own.
///
/// **The record goes in a directory of its own.** Writing a private file makes its
/// *parent* owner-only, so a test pointing at the shared temporary directory would
/// take everyone else's out from under them.
#[tokio::test]
async fn a_request_that_went_somewhere_is_written_down_there() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("recorded");
    let _ = std::fs::remove_dir_all(&dir);
    let at = dir.join("outbound.log");

    let transport = Recording::around(
        Shared(Fake::always(Answer::Reply(200, String::new()))),
        Some(at.clone()),
        Stopped::at(1),
    );

    let answered = transport.send(&asking()).await;
    assert_eq!(answered.map(|response| response.status), Ok(200));

    let written = std::fs::read_to_string(&at).unwrap_or_default();
    assert!(
        written.starts_with("1 Get "),
        "the stamp and the verb did not open the line"
    );
    assert!(
        written.trim_end().ends_with(" 200"),
        "what came back was not written at the end of the line"
    );
    assert!(
        !written.contains("the-indexer-key"),
        "the indexer key reached the file on disk"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Nowhere to write is not somewhere to fail.
///
/// A machine that will not say where its own files go still has to be able to
/// make the request the operator asked for.
#[tokio::test]
async fn a_run_with_nowhere_to_write_still_sends() {
    let transport = Recording::around(
        Shared(Fake::always(Answer::Reply(200, String::new()))),
        None,
        Stopped::at(1),
    );

    let answered = transport.send(&asking()).await;
    assert_eq!(answered.map(|response| response.status), Ok(200));
}

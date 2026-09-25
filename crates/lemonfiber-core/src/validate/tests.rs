use std::sync::Arc;

use async_trait::async_trait;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{Credential, Live, Validation, Validator};
use crate::ports::nntp::{Endpoint, Nntp};

/// A validator whose transport answers with the given body at 200.
fn answering(body: &str) -> Live {
    Live::new(Fake::always(Answer::reply(200, body.to_owned())))
}

/// The indexer credential the tests prove; the URL and key are immaterial to a
/// scripted transport.
fn indexer() -> Credential {
    Credential::Indexer {
        url: "http://indexer.test/api".to_owned(),
        key: "abc".to_owned(),
    }
}

#[tokio::test]
async fn what_an_indexer_says_about_a_key_comes_back_without_the_key_in_it() {
    // An indexer refuses in its own words, with the key it was given in hand, and
    // those words are carried into the outcome verbatim. The outcome is serialised
    // into the wizard's report during first-run setup — the minutes in which the
    // key is being entered — and printed to a terminal beside it.
    let key = ["the", "indexer", "key"].join("-");
    let body = format!("<error code=\"100\" description=\"apikey={key} has expired\"/>");

    let outcome = format!("{:?}", answering(&body).validate(&indexer()).await);

    assert!(
        !outcome.contains(&key),
        "the key the indexer quoted back survived into the outcome"
    );
    // And the reason survives, which is the whole of what the operator is given:
    // a refusal with its refusal withheld says only that something went wrong.
    assert!(
        outcome.contains("the indexer refused the key"),
        "the reason the key was refused went with the key"
    );
    assert!(
        outcome.contains("has expired"),
        "what the indexer said about the key went with the key"
    );
}

#[tokio::test]
async fn a_well_formed_feed_proves_the_key_and_reports_what_it_held() {
    let feed = "<?xml version=\"1.0\"?><rss><channel><item>a</item><item>b</item></channel></rss>";
    let outcome = answering(feed).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("2 result")
    ));
}

#[tokio::test]
async fn a_key_proven_over_plaintext_http_is_named_as_exposed() {
    // The default indexer URL is http://, so a proven key carries the caveat
    // that it rode the wire in the clear, pointing the operator at https.
    let feed = "<?xml version=\"1.0\"?><rss><channel><item>a</item></channel></rss>";
    let outcome = answering(feed).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed }
            if observed.contains("plaintext http") && observed.contains("https")
    ));
}

#[tokio::test]
async fn a_key_proven_over_https_carries_no_plaintext_caveat() {
    let feed = "<?xml version=\"1.0\"?><rss><channel><item>a</item></channel></rss>";
    let secure = Credential::Indexer {
        url: "https://indexer.test/api".to_owned(),
        key: "abc".to_owned(),
    };
    let outcome = answering(feed).validate(&secure).await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if !observed.contains("plaintext")
    ));
}

#[tokio::test]
async fn an_error_element_for_a_bad_key_is_a_refusal_with_the_reason() {
    let body = "<error code=\"100\" description=\"Incorrect user credentials\"/>";
    let outcome = answering(body).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("Incorrect user credentials")
    ));
}

#[tokio::test]
async fn a_rate_limit_error_is_degraded_and_transient_not_a_refusal() {
    let body = "<error code=\"500\" description=\"Request limit reached\"/>";
    let outcome = answering(body).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Degraded { detail } if detail.contains("rate-limiting")
    ));
}

#[test]
fn a_service_that_kept_not_answering_is_told_apart_from_one_that_was_busy() {
    // The transport's own words lead either way — they are the only account of
    // what happened — and what lemonfiber adds is whether it kept happening.
    let once = crate::ports::http::Unreachable::once("http://sonarr", "connection refused");
    assert_eq!(super::persisting(&once), "connection refused");

    let persisted = crate::ports::http::Unreachable {
        attempts: crate::retry::ATTEMPTS,
        ..once
    };
    assert_eq!(
        super::persisting(&persisted),
        "connection refused — still failing after 3 attempts"
    );
}

/// The whitespace around a pasted key is a paste artefact; the whitespace inside
/// it is not, and a value that legitimately carries one keeps it.
#[test]
fn the_whitespace_around_a_pasted_key_is_not_part_of_the_key() {
    assert_eq!(super::pasted("  abc123\n"), "abc123");
    assert_eq!(super::pasted("\tabc123 "), "abc123");
    assert_eq!(super::pasted("abc123"), "abc123");
    assert_eq!(super::pasted("ab c123"), "ab c123");
    assert_eq!(super::pasted("   "), "");
}

/// A certificate that was not trusted is a different problem from a connection
/// that was refused, and has a different remedy. Collapsed together, the operator
/// is sent to check a hostname and a port that were never wrong.
#[test]
fn a_certificate_that_was_not_trusted_is_told_apart_from_a_refused_connection() {
    let refused = crate::ports::http::Unreachable::once("https://indexer", "connection refused");
    let plain = super::persisting(&refused);
    assert!(!plain.contains("certificate"), "{plain}");

    let untrusted = crate::ports::http::Unreachable::once(
        "https://indexer",
        "invalid peer certificate: UnknownIssuer",
    );
    let named = super::persisting(&untrusted);
    // The transport's own words still lead — they are the account of what happened.
    assert!(named.contains("invalid peer certificate"), "{named}");
    assert!(named.contains("was not trusted"), "{named}");
    assert!(named.contains("trusted for that host"), "{named}");
}

/// A transport that fails every attempt with the given reason.
struct Failing(&'static str);

/// A transport that answers each attempt in turn: a reason to fail with, or an
/// empty one to answer a search successfully.
struct InTurn(std::sync::Mutex<std::collections::VecDeque<&'static str>>);

impl InTurn {
    fn of(reasons: &[&'static str]) -> Arc<Self> {
        Arc::new(Self(std::sync::Mutex::new(
            reasons.iter().copied().collect(),
        )))
    }
}

#[async_trait]
impl crate::ports::http::Http for InTurn {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        let next = self
            .0
            .lock()
            .ok()
            .and_then(|mut queued| queued.pop_front())
            .unwrap_or_default();
        if next.is_empty() {
            return Ok(crate::ports::http::Response {
                status: 200,
                headers: Vec::new(),
                body: "<?xml version=\"1.0\"?><rss><channel><item>a</item></channel></rss>"
                    .to_owned(),
            });
        }
        Err(crate::ports::http::Unreachable::once(&request.url, next))
    }
}

#[async_trait]
impl crate::ports::http::Http for Failing {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        Err(crate::ports::http::Unreachable::once(&request.url, self.0))
    }
}

/// How long a service was given to answer is part of what could not be reached.
/// A wait that ran to the bound and an instant refusal are different facts, and
/// "unreachable" alone does not tell the operator which one happened.
#[tokio::test]
async fn a_service_that_did_not_answer_says_how_long_it_was_waited_for() {
    let outcome = Live::new(Arc::new(Failing("connection refused")))
        .validate(&indexer())
        .await;
    let said = format!("{outcome:?}");
    assert!(said.contains("Unreachable"), "{said}");
    // The transport's own words still lead.
    assert!(said.contains("connection refused"), "{said}");
    assert!(said.contains("after "), "{said}");
    // A refused connection is the service, not the machine — no outage is claimed.
    assert!(!said.contains("Nothing on this machine"), "{said}");
}

/// One outage is one thing that went wrong. Said against every credential in turn
/// it reads as several bad keys, and sends the operator checking the ones that
/// were never the problem.
#[tokio::test]
async fn a_network_that_is_down_is_said_once_and_not_against_each_credential() {
    let validator = Live::new(Arc::new(Failing(
        "dns error: failed to lookup address information",
    )));

    let first = format!("{:?}", validator.validate(&indexer()).await);
    assert!(first.contains("dns error"), "{first}");
    assert!(first.contains("Nothing on this machine"), "{first}");

    let second = format!("{:?}", validator.validate(&indexer()).await);
    assert!(second.contains("still down"), "{second}");
    assert!(!second.contains("Nothing on this machine"), "{second}");
}

/// Said once for one outage — not once for the life of the validator.
///
/// A served surface holds one validator across any number of outages. Latching the
/// flag would mean the first outage of the process is the only one ever explained,
/// and every later one — hours apart, with the network long since back and gone
/// again — is waved at an outage that ended.
#[tokio::test]
async fn an_outage_that_ended_is_not_the_outage_reported_before_it() {
    const DOWN: &str = "dns error: failed to lookup address information";
    let validator = Live::new(InTurn::of(&[DOWN, DOWN, "", DOWN]));

    let first = format!("{:?}", validator.validate(&indexer()).await);
    assert!(first.contains("Nothing on this machine"), "{first}");

    let same = format!("{:?}", validator.validate(&indexer()).await);
    assert!(same.contains("still down"), "{same}");

    // The network answers, so the outage is over.
    let back = format!("{:?}", validator.validate(&indexer()).await);
    assert!(back.contains("Valid"), "{back}");

    // A new outage is a new thing to explain, not the one already reported.
    let again = format!("{:?}", validator.validate(&indexer()).await);
    assert!(again.contains("Nothing on this machine"), "{again}");
    assert!(!again.contains("still down"), "{again}");
}

/// A connection that died mid-handshake says nothing about the certificate, and
/// naming one sends the operator to fix what was never wrong.
#[test]
fn a_handshake_that_failed_is_not_reported_as_an_untrusted_certificate() {
    let broken = crate::ports::http::Unreachable::once("https://indexer", "tls handshake eof");
    let said = super::persisting(&broken);
    assert!(said.contains("tls handshake eof"), "{said}");
    assert!(!said.contains("certificate was not trusted"), "{said}");
}

/// A failure that reached something clears the outage, so the next one is explained
/// in full even though nothing succeeded in between.
#[tokio::test]
async fn a_refusal_between_two_outages_ends_the_first_one() {
    const DOWN: &str = "dns error: failed to lookup address information";
    let validator = Live::new(InTurn::of(&[DOWN, "connection refused", DOWN]));

    let first = format!("{:?}", validator.validate(&indexer()).await);
    assert!(first.contains("Nothing on this machine"), "{first}");

    let reached = format!("{:?}", validator.validate(&indexer()).await);
    assert!(reached.contains("connection refused"), "{reached}");

    let again = format!("{:?}", validator.validate(&indexer()).await);
    assert!(again.contains("Nothing on this machine"), "{again}");
}

/// An indexer refusing a key quotes it back inside its own description, and that
/// sentence is carried here verbatim. It must not reach the outcome: the outcome
/// is serialised into a report and printed to a terminal, both during first-run
/// setup, which is the moment those credentials are being entered.
#[tokio::test]
async fn a_service_that_quotes_the_key_back_does_not_carry_it_into_the_outcome() {
    let refusal = "apikey=the-secret-key is not a valid key";
    let body = format!("<error code=\"100\" description=\"{refusal}\"/>");
    let outcome = answering(&body).validate(&indexer()).await;
    let said = format!("{outcome:?}");
    // Deliberately not printing the outcome here: a guard that reports the leak by
    // repeating it is the thing it watches for.
    assert!(
        !said.contains("the-secret-key"),
        "the key reached the outcome"
    );
    // Withheld where the value was, rather than the sentence being dropped whole —
    // the operator still needs to know which key was refused and why.
    assert!(said.contains("apikey"), "{said}");
    assert!(said.contains("not a valid key"), "{said}");
}

#[tokio::test]
async fn an_error_without_a_description_still_refuses_rather_than_panics() {
    let body = "<error code=\"101\"/>";
    let outcome = answering(body).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("no reason given")
    ));
}

#[tokio::test]
async fn a_refusing_status_with_no_error_element_is_still_a_refusal() {
    let outcome = Live::new(Fake::always(Answer::reply(401, String::new())))
        .validate(&indexer())
        .await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("401")
    ));
}

#[tokio::test]
async fn a_page_that_is_not_an_indexer_points_at_the_url_not_the_key() {
    let outcome = answering("<html><body>Sign in</body></html>")
        .validate(&indexer())
        .await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("check the URL")
    ));
}

#[tokio::test]
async fn an_error_whose_code_is_not_quoted_is_not_read_as_a_refusal() {
    // A reader that only understands the fixed, quoted shape both specs use must
    // not mistake a malformed attribute for a code — it reads no code, so the
    // answer falls through to "not an indexer" rather than a false refusal.
    let outcome = answering("<error code=100/>").validate(&indexer()).await;
    assert!(matches!(outcome, Validation::Unreachable { detail } if detail.contains("URL")));
}

#[tokio::test]
async fn nothing_answering_at_all_is_unreachable_with_the_transports_reason() {
    let outcome = Live::new(Fake::silent()).validate(&indexer()).await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("connection refused")
    ));
}

/// The service credential the tests prove.
fn service() -> Credential {
    Credential::Service {
        url: "http://sonarr.test:8989/api/v3/system/status".to_owned(),
        key: "abc".to_owned(),
    }
}

/// An NNTP transport that answers `converse` with scripted reply lines, or
/// with nothing at all where the connection could not be made.
struct Dialogue(Result<Vec<String>, crate::ports::nntp::Unreachable>);

#[async_trait]
impl Nntp for Dialogue {
    async fn converse(
        &self,
        _endpoint: &Endpoint,
        _commands: &[String],
    ) -> Result<Vec<String>, crate::ports::nntp::Unreachable> {
        self.0.clone()
    }
}

/// A validator that reaches Usenet through a transport scripted to reply with
/// `lines` (greeting, then a reply per command).
fn dialling(lines: &[&str]) -> Live {
    let replies = lines.iter().map(|line| (*line).to_owned()).collect();
    Live::with_nntp(
        Fake::always(Answer::reply(200, String::new())),
        Arc::new(Dialogue(Ok(replies))),
    )
}

/// The Usenet credential the tests prove; host and port are immaterial to a
/// scripted transport.
fn usenet() -> Credential {
    Credential::Usenet {
        host: "news.provider.test".to_owned(),
        port: 563,
        secure: true,
        user: "person".to_owned(),
        pass: "secret".to_owned(),
    }
}

#[tokio::test]
async fn a_provider_that_accepts_the_login_proves_the_account() {
    // Greeting, reply to USER, then 281 accepted to PASS.
    let outcome = dialling(&["200 welcome", "381 more", "281 authenticated"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("accepted the login")
    ));
}

#[tokio::test]
async fn a_provider_that_accepts_at_the_username_step_needs_no_password() {
    // 281 to AUTHINFO USER means the login is already accepted; the password
    // reply that still follows must not be mistaken for a refusal.
    let outcome = dialling(&["200 welcome", "281 authenticated", "482 out of sequence"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("accepted the login")
    ));
}

#[tokio::test]
async fn a_provider_that_refuses_the_login_is_rejected() {
    let outcome = dialling(&["200 welcome", "381 more", "481 authentication failed"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("username or password")
    ));
}

#[tokio::test]
async fn a_provider_at_its_connection_limit_at_the_greeting_is_degraded() {
    // 502 at the greeting turns the connection away before the login.
    let outcome = dialling(&["502 too many connections"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Degraded { detail } if detail.contains("connection limit")
    ));
}

#[tokio::test]
async fn a_login_refused_for_no_permission_is_degraded_not_a_wrong_password() {
    let outcome = dialling(&["200 welcome", "381 more", "502 no permission"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Degraded { detail } if detail.contains("connection limit")
    ));
}

#[tokio::test]
async fn an_out_of_sequence_login_reads_as_a_refusal() {
    let outcome = dialling(&["200 welcome", "381 more", "482 out of sequence"])
        .validate(&usenet())
        .await;
    assert!(matches!(outcome, Validation::Rejected { .. }));
}

#[tokio::test]
async fn an_unexpected_login_code_is_refused_carrying_the_number() {
    let outcome = dialling(&["200 welcome", "381 more", "400 service discontinued"])
        .validate(&usenet())
        .await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("400")
    ));
}

#[tokio::test]
async fn a_login_reply_without_a_code_cannot_be_read() {
    let outcome = dialling(&["200 welcome", "381 more", "not a coded line"])
        .validate(&usenet())
        .await;
    assert!(matches!(outcome, Validation::Unreachable { .. }));
}

#[tokio::test]
async fn an_exchange_that_did_not_finish_is_unreachable() {
    // Only a greeting came back — the login never completed.
    let outcome = dialling(&["200 welcome"]).validate(&usenet()).await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("did not complete")
    ));
}

#[tokio::test]
async fn a_provider_that_cannot_be_reached_is_unreachable() {
    let live = Live::with_nntp(
        Fake::always(Answer::reply(200, String::new())),
        Arc::new(Dialogue(Err(crate::ports::nntp::Unreachable {
            host: "news.provider.test".to_owned(),
            reason: "connection refused".to_owned(),
        }))),
    );
    assert!(matches!(
        live.validate(&usenet()).await,
        Validation::Unreachable { detail } if detail.contains("connection refused")
    ));
}

#[tokio::test]
async fn a_usenet_credential_with_no_transport_is_unproven_not_pretended() {
    // The HTTP-only validator has no NNTP transport, so a Usenet credential
    // cannot be proven — reported unreachable rather than passed.
    let outcome = answering(String::new().as_str()).validate(&usenet()).await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("no Usenet transport")
    ));
}

#[tokio::test]
async fn a_plaintext_provider_is_left_unproven_rather_than_exposing_the_password() {
    // The transport is scripted to accept the login, so were the password sent
    // the outcome would be Valid. Over a non-TLS endpoint it is refused before
    // anything is sent — unreachable, naming TLS as the fix — so the password
    // never reaches the wire.
    let insecure = Credential::Usenet {
        host: "news.provider.test".to_owned(),
        port: 119,
        secure: false,
        user: "person".to_owned(),
        pass: "secret".to_owned(),
    };
    let outcome = dialling(&["200 welcome", "381 more", "281 authenticated"])
        .validate(&insecure)
        .await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail }
            if detail.contains("TLS") && detail.contains("in the clear")
    ));
}

#[tokio::test]
async fn a_service_that_answers_its_identity_is_proven_and_names_itself() {
    let body = r#"{"instanceName":"Sonarr","version":"4.0.1"}"#;
    let outcome = answering(body).validate(&service()).await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("Sonarr") && observed.contains("4.0.1")
    ));
}

#[tokio::test]
async fn a_service_that_answers_without_naming_itself_is_still_proven() {
    // A 2xx with no recognisable identity still proves the key was accepted.
    let outcome = answering("{}").validate(&service()).await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("accepted the key")
    ));
}

#[tokio::test]
async fn a_service_that_gives_only_a_version_reports_that() {
    let outcome = answering(r#"{"version":"4.0.1"}"#)
        .validate(&service())
        .await;
    assert!(matches!(
        outcome,
        Validation::Valid { observed } if observed.contains("version 4.0.1")
    ));
}

#[tokio::test]
async fn a_service_that_refuses_the_key_is_rejected() {
    let outcome = Live::new(Fake::always(Answer::reply(401, String::new())))
        .validate(&service())
        .await;
    assert!(matches!(
        outcome,
        Validation::Rejected { detail } if detail.contains("401")
    ));
}

#[tokio::test]
async fn a_service_url_that_answers_as_something_else_points_at_the_url() {
    let outcome = Live::new(Fake::always(Answer::reply(
        404,
        "<html>not found</html>".to_owned(),
    )))
    .validate(&service())
    .await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("check the URL")
    ));
}

#[tokio::test]
async fn a_service_that_does_not_answer_is_unreachable() {
    let outcome = Live::new(Fake::silent()).validate(&service()).await;
    assert!(matches!(
        outcome,
        Validation::Unreachable { detail } if detail.contains("connection refused")
    ));
}

#[tokio::test]
async fn a_service_is_reached_with_its_key_in_the_api_header() {
    let recording = Fake::always(Answer::reply(200, r#"{"instanceName":"Radarr"}"#));
    let outcome = Live::new(recording.clone()).validate(&service()).await;

    assert!(matches!(outcome, Validation::Valid { .. }));
    let asked = recording.request();
    assert!(
        asked
            .as_ref()
            .is_some_and(|request| request.url.contains("system/status")),
        "the identity endpoint is reached"
    );
    assert!(
        asked.is_some_and(|request| request
            .headers
            .iter()
            .any(|(name, value)| name == "X-Api-Key" && value == "abc")),
        "the key authenticates the request as a header, not a query param"
    );
}

#[tokio::test]
async fn the_search_carries_the_key_and_keeps_an_existing_query_intact() {
    let recording = Fake::always(Answer::reply(200, "<rss><channel></channel></rss>"));
    let credential = Credential::Indexer {
        url: "http://indexer.test/api?limit=1".to_owned(),
        key: "secret".to_owned(),
    };
    let outcome = Live::new(recording.clone()).validate(&credential).await;

    assert!(matches!(outcome, Validation::Valid { .. }));
    let url = recording.request().map(|request| request.url);
    assert!(
        url.as_deref().is_some_and(|url| url.contains("t=search")),
        "a real search is issued"
    );
    assert!(
        url.as_deref()
            .is_some_and(|url| url.contains("apikey=secret")),
        "the key authenticates the query"
    );
    assert!(
        url.as_deref().is_some_and(|url| url.contains("?limit=1&")),
        "an existing query is joined with & not a second ?"
    );
}

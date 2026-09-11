//! An HTTP transport that writes down what left this machine.
//!
//! The enumeration beside it says what *would* be sent — which requests exist,
//! where each goes, what travels and how to stop it. That is a description of the
//! program, and an operator checking whether they were told the truth needs the
//! other thing: a record of what actually went, made as it went.
//!
//! Wrapped around whatever really speaks HTTP rather than written into each
//! caller, for the reason the retry policy beside it is: a record kept at fifteen
//! call sites is fifteen records, and the request that goes unrecorded will be the
//! one at the site somebody forgot.
//!
//! **What is written down is deliberately less than what was sent.** A request
//! carries the credential a service authenticates with and, where it writes, a
//! body. A log holding either would be the thing this feature exists to prevent —
//! an operator who turned every outbound request off would still have a file full
//! of their own keys. So the line is when, what kind of request, where it went with
//! the query and any userinfo withheld, and what came back. Never a header, never
//! a body.
//!
//! Both, and in that order, because a URL carries a credential in two places and
//! only one of them is a guess. The query goes wholesale — which parameter holds a
//! key belongs to whoever wrote the service — and the password in front of the host
//! goes because the URI syntax itself says everything after that colon is one. The
//! address then goes through the same scrubber a support bundle's text does, so a
//! credential that reached a URL in spite of all this is withheld here too rather
//! than only where somebody remembered.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::http::{Http, Request, Response, Unreachable};
use crate::ports::time::Clock;
use crate::ports::withheld::{withheld, without_credentials};

/// How many lines are kept.
///
/// A record that grows without end is a disk problem somebody meets months later,
/// and one that is trimmed is still an answer to "what has this been doing" — which
/// is the question, rather than "what has it ever done". The oldest go first.
const KEPT: usize = 500;

/// A transport that writes down what it sent.
pub struct Recording<H> {
    /// What actually speaks HTTP.
    inner: H,
    /// Where the record is kept, or nothing where this machine will not say where
    /// its own files go — in which case nothing is written and nothing pretends to
    /// have been.
    at: Option<PathBuf>,
    /// What the time is, asked through the port so a test can say.
    clock: Arc<dyn Clock>,
}

impl<H> Recording<H> {
    /// Wrap a transport so what it sends is written down.
    pub const fn around(inner: H, at: Option<PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Self { inner, at, clock }
    }
}

/// One line of the record.
///
/// Built here rather than at the write, so what a line contains is one function a
/// test can put a request to — and so the rule that a header never reaches it is a
/// property of a value rather than of a habit.
///
/// The address goes through [`without_credentials`] and then [`withheld`], and that
/// order is the whole of it rather than a tidiness. Both places a URL can carry a
/// credential — the query, and the password in front of the host — are withheld by
/// the first; the second is the scrubber a support bundle's text takes, kept after
/// it so a credential shaped like a setting inside the path is caught too. Taking
/// the query off *before* either of them is what this used to do, and it disabled
/// them: the general scrubber reaches the address rule only through a token that
/// still carries a `?…=…`, so a login written in front of the host fell through
/// both and was written down verbatim.
///
/// Withheld rather than deleted, so the line still says a query was sent — a record
/// that quietly drops the fact reads as a smaller request than the one that left.
fn line(at: u64, request: &Request, answered: Option<u16>) -> String {
    let outcome = answered.map_or_else(
        || "nothing answered".to_owned(),
        |status| status.to_string(),
    );
    format!(
        "{at} {:?} {} {outcome}",
        request.method,
        withheld(&without_credentials(&request.url))
    )
}

/// The record as it stands after this line, oldest dropped.
fn kept(existing: &str, added: &str) -> String {
    let mut lines: Vec<&str> = existing.lines().filter(|line| !line.is_empty()).collect();
    lines.push(added);
    let from = lines.len().saturating_sub(KEPT);
    lines.get(from..).unwrap_or_default().join("\n") + "\n"
}

#[async_trait]
impl<H: Http + Send + Sync> Http for Recording<H> {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        send(self, request).await
    }
}

async fn send<H: Http + Send + Sync>(
    recording: &Recording<H>,
    request: &Request,
) -> Result<Response, Unreachable> {
    let answer = recording.inner.send(request).await;
    let Some(at) = recording.at.as_ref() else {
        return answer;
    };
    let when = recording
        .clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let status = answer.as_ref().ok().map(|answered| answered.status);
    let existing = tokio::fs::read_to_string(at).await.unwrap_or_default();
    // A record that could not be written is not worth failing a request over:
    // the operator asked for the thing the request does, and telling them it
    // could not be done because a log was unwritable would be this feature
    // getting in the way of the product it is meant to make trustworthy.
    let _ = crate::config::store::write(at, &kept(&existing, &line(when, request, status)));
    answer
}

#[cfg(test)]
mod tests {
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

        assert!(said.contains("indexer.example"), "{said}");
        assert!(said.contains("200"), "{said}");
        assert!(!said.contains("the-indexer-key"), "no credential: {said}");
        assert!(!said.contains("hunter2"), "no body: {said}");
        assert!(!said.contains("X-Api-Key"), "no header: {said}");
        assert!(
            !said.contains("q=something"),
            "and nothing asked for: {said}"
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

            // None of these quote the line. A failure message is copied into a CI log,
            // which is read by more people and kept longer than the machine that wrote
            // it — so an assertion about a credential not surviving must not be the
            // thing that carries it onward. Each says which half of the claim broke,
            // which is what a reader of the failure needs.
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
        assert!(said.contains("nothing answered"), "{said}");
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
        let dir = std::env::temp_dir().join(format!("lemonfiber-recorded-{}", std::process::id()));
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
            "the stamp and the verb: {written}"
        );
        assert!(
            written.trim_end().ends_with(" 200"),
            "what came back: {written}"
        );
        assert!(
            !written.contains("the-indexer-key"),
            "the credential reached the record: {written}"
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
}

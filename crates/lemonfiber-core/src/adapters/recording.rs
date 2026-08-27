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
//! the query and any userinfo taken off, and what came back. Never a header, never
//! a body.
//!
//! The address goes through the same scrubber a support bundle's does, so a
//! credential that reached a URL in spite of all this is withheld here too rather
//! than only where somebody remembered.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::http::{Http, Request, Response, Unreachable};
use crate::ports::time::Clock;
use crate::ports::withheld::withheld;

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
fn line(at: u64, request: &Request, answered: Option<u16>) -> String {
    let outcome = answered.map_or_else(
        || "nothing answered".to_owned(),
        |status| status.to_string(),
    );
    format!(
        "{at} {:?} {} {outcome}",
        request.method,
        withheld(bare(&request.url))
    )
}

/// The address without the part that carries what was asked for.
///
/// A query string is where a search term, a title, or an indexer key ends up, and
/// none of those is the operator's to have leaked into a file by a check that was
/// meant to reassure them. What is left is where the request went, which is what
/// the question is about.
fn bare(url: &str) -> &str {
    url.split_once('?').map_or(url, |(before, _)| before)
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
        let answer = self.inner.send(request).await;
        let Some(at) = self.at.as_ref() else {
            return answer;
        };
        let when = self
            .clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default();
        let status = answer.as_ref().ok().map(|answered| answered.status);
        let existing = std::fs::read_to_string(at).unwrap_or_default();
        // A record that could not be written is not worth failing a request over:
        // the operator asked for the thing the request does, and telling them it
        // could not be done because a log was unwritable would be this feature
        // getting in the way of the product it is meant to make trustworthy.
        let _ = crate::config::store::write(at, &kept(&existing, &line(when, request, status)));
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::{bare, kept, line, Recording, KEPT};
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

    /// A query is taken off whether or not it holds a credential.
    #[test]
    fn the_part_that_says_what_was_asked_for_is_taken_off() {
        assert_eq!(bare("https://a.example/x?y=z"), "https://a.example/x");
        assert_eq!(bare("https://a.example/x"), "https://a.example/x");
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

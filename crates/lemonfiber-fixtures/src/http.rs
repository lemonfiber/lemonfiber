//! The transport every test drives a service through.
//!
//! Nine test crates each declared their own, and they were the same three ideas written out
//! nine times: answer everything the same way, answer a scripted sequence in order, or
//! answer by what was asked for. Nine copies is nine places for the semantics to drift.
//!
//! One transport with three ways to script it. What it records is the same in every case:
//! every request, in order, so a test can assert on the last one, on all of them, or on
//! whether something was ever asked for at all.
//!
//! A request nothing answers is unreachable rather than a default response. A test that
//! reaches an endpoint it did not script has found something, and quietly handing it a
//! `200` would hide it.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::http::{Http, Method, Request, Response, Unreachable};

/// What the transport answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// A response with this status and body.
    Reply(u16, String),
    /// Nothing at all — the service is not there.
    Silent,
}

impl Answer {
    /// A response, from anything that reads as a body.
    ///
    /// The body is owned because some tests build one, and a borrowed literal
    /// converts into it without ceremony.
    #[must_use]
    pub fn reply(status: u16, body: impl Into<String>) -> Self {
        Self::Reply(status, body.into())
    }
}

/// How a fake decides what to answer.
enum Script {
    /// The same answer to everything.
    Always(Answer),
    /// Each answer in turn, and nothing once they run out — a sequence that ran
    /// short means the code under test asked for more than the test described.
    InTurn(Mutex<VecDeque<Answer>>),
    /// By what was asked for: the first route whose fragment the URL contains,
    /// and whose method matches where one was given.
    ByRoute(Vec<(Option<Method>, &'static str, Replies)>),
}

/// What one route answers, however many times it is asked.
enum Replies {
    /// The same answer every time.
    Same(Answer),
    /// Each answer in turn, the last one repeating — for a route whose answer
    /// changes because something else was written. A service that reports
    /// "not set up" until setup is finished is two answers to one path, and
    /// scripting them in order says which write is supposed to change it.
    ///
    /// The last repeats rather than running out because the number of times a
    /// read happens after the change is the code's business, not the test's.
    InTurn(Mutex<VecDeque<Answer>>),
}

impl Replies {
    /// This route's answer to the request being made now.
    fn answer(&self) -> Answer {
        match self {
            Self::Same(answer) => answer.clone(),
            Self::InTurn(remaining) => remaining.lock().map_or(Answer::Silent, |mut remaining| {
                if remaining.len() > 1 {
                    remaining.pop_front().unwrap_or(Answer::Silent)
                } else {
                    remaining.front().cloned().unwrap_or(Answer::Silent)
                }
            }),
        }
    }
}

/// A transport that answers from a script and remembers what it was asked.
pub struct Fake {
    script: Script,
    seen: Mutex<Vec<Request>>,
}

impl Fake {
    /// One answer, to everything.
    #[must_use]
    pub fn always(answer: Answer) -> Arc<Self> {
        Self::new(Script::Always(answer))
    }

    /// Nothing, to everything: the service is not there.
    #[must_use]
    pub fn silent() -> Arc<Self> {
        Self::always(Answer::Silent)
    }

    /// Each answer in turn.
    #[must_use]
    pub fn in_turn(answers: Vec<Answer>) -> Arc<Self> {
        Self::new(Script::InTurn(Mutex::new(answers.into())))
    }

    /// Each reply in turn, written as the status and body most tests already have.
    ///
    /// The same thing as [`Fake::in_turn`], spelled the way a test that only ever
    /// replies with a status and a body wants to spell it.
    #[must_use]
    pub fn scripted(replies: Vec<(u16, &'static str)>) -> Arc<Self> {
        Self::in_turn(
            replies
                .into_iter()
                .map(|(status, body)| Answer::reply(status, body))
                .collect(),
        )
    }

    /// By what was asked for, where some routes care about the method and others do
    /// not — the general form [`Fake::by_path`] and [`Fake::by_route`] are shorthands
    /// for. The first route whose fragment the URL contains wins, so a table can put
    /// one method-scoped refusal in front of paths that answer whatever the method.
    #[must_use]
    pub fn by_rules(routes: Vec<(Option<Method>, &'static str, Answer)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(method, fragment, answer)| (method, fragment, Replies::Same(answer)))
                .collect(),
        ))
    }

    /// By what the URL contains, whatever the method.
    #[must_use]
    pub fn by_path(routes: Vec<(&'static str, Answer)>) -> Arc<Self> {
        Self::by_rules(
            routes
                .into_iter()
                .map(|(fragment, answer)| (None, fragment, answer))
                .collect(),
        )
    }

    /// By method and what the URL contains — for the services where a read and a
    /// write of the same path are different answers.
    #[must_use]
    pub fn by_route(routes: Vec<(Method, &'static str, Answer)>) -> Arc<Self> {
        Self::by_rules(
            routes
                .into_iter()
                .map(|(method, fragment, answer)| (Some(method), fragment, answer))
                .collect(),
        )
    }

    /// By what the URL contains, where some route answers differently after
    /// something was written — each of its answers in turn, the last repeating.
    ///
    /// The route order still decides which one matches, so a path that changes
    /// and a path that does not can sit in the same table.
    #[must_use]
    pub fn by_path_in_turn(routes: Vec<(&'static str, Vec<Answer>)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(fragment, answers)| {
                    (None, fragment, Replies::InTurn(Mutex::new(answers.into())))
                })
                .collect(),
        ))
    }

    /// By method and what the URL contains, where a route's answer changes — the
    /// shape a write-then-read-back needs, since the same path answers one thing
    /// before the write and another after it.
    #[must_use]
    pub fn by_route_in_turn(routes: Vec<(Method, &'static str, Vec<Answer>)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(method, fragment, answers)| {
                    (
                        Some(method),
                        fragment,
                        Replies::InTurn(Mutex::new(answers.into())),
                    )
                })
                .collect(),
        ))
    }

    /// The last request it was sent, or nothing where it was sent none.
    pub fn request(&self) -> Option<Request> {
        self.seen.lock().ok().and_then(|seen| seen.last().cloned())
    }

    /// Every request it was sent, in the order they came.
    pub fn requests(&self) -> Vec<Request> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// Whether it was ever asked for something whose URL contains this.
    ///
    /// For the reads that make more than one request, where asserting on the last
    /// one would be asserting on whichever happened to come second.
    pub fn asked_for(&self, fragment: &str) -> bool {
        self.seen
            .lock()
            .is_ok_and(|seen| seen.iter().any(|request| request.url.contains(fragment)))
    }

    /// How many requests it was sent.
    pub fn asked(&self) -> usize {
        self.seen.lock().map(|seen| seen.len()).unwrap_or_default()
    }

    fn new(script: Script) -> Arc<Self> {
        Arc::new(Self {
            script,
            seen: Mutex::new(Vec::new()),
        })
    }

    /// What this script says to this request.
    fn answer(&self, request: &Request) -> Answer {
        match &self.script {
            Script::Always(answer) => answer.clone(),
            Script::InTurn(remaining) => remaining
                .lock()
                .ok()
                .and_then(|mut remaining| remaining.pop_front())
                .unwrap_or(Answer::Silent),
            Script::ByRoute(routes) => routes
                .iter()
                .find(|(method, fragment, _)| {
                    request.url.contains(fragment)
                        && method.is_none_or(|wanted| wanted == request.method)
                })
                .map_or(Answer::Silent, |(_, _, replies)| replies.answer()),
        }
    }
}

#[async_trait]
impl Http for Fake {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        match self.answer(request) {
            Answer::Reply(status, body) => Ok(Response { status, body }),
            Answer::Silent => Err(Unreachable {
                url: request.url.clone(),
                reason: "connection refused".to_owned(),
                attempts: 1,
            }),
        }
    }
}

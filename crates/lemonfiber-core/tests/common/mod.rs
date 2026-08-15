//! The transport every integration test drives a service through.
//!
//! Nine of these test crates each declared their own fake HTTP client, and they
//! were the same three ideas written out nine times: answer everything the same
//! way, answer a scripted sequence in order, or answer by what was asked for.
//! Nine copies is nine places for the semantics to drift — and the tenth copy was
//! nearly written before this existed.
//!
//! One transport with three ways to script it. What it records is the same in
//! every case: every request, in order, so a test can assert on the last one, on
//! all of them, or on whether something was ever asked for at all.
//!
//! A request nothing answers is unreachable rather than a default response. A
//! test that reaches an endpoint it did not script has found something, and
//! quietly handing it a `200` would hide it.
//!
//! `dead_code` is allowed here and nowhere else in the workspace: this module is
//! compiled into each test crate separately, and no single one of them uses all
//! three ways to script it. The repository's own rule against suppression exempts
//! test paths for exactly this.

#![allow(dead_code)]

pub mod service;
pub mod tunnel;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::ports::http::{Http, Method, Request, Response, Unreachable};

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
    ByRoute(Vec<(Option<Method>, &'static str, Answer)>),
}

/// A transport that answers from a script and remembers what it was asked.
pub struct Fake {
    script: Script,
    seen: Mutex<Vec<Request>>,
}

impl Fake {
    /// One answer, to everything.
    pub fn always(answer: Answer) -> Arc<Self> {
        Self::new(Script::Always(answer))
    }

    /// Nothing, to everything: the service is not there.
    pub fn silent() -> Arc<Self> {
        Self::always(Answer::Silent)
    }

    /// Each answer in turn.
    pub fn in_turn(answers: Vec<Answer>) -> Arc<Self> {
        Self::new(Script::InTurn(Mutex::new(answers.into())))
    }

    /// By what the URL contains, whatever the method.
    pub fn by_path(routes: Vec<(&'static str, Answer)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(fragment, answer)| (None, fragment, answer))
                .collect(),
        ))
    }

    /// By method and what the URL contains — for the services where a read and a
    /// write of the same path are different answers.
    pub fn by_route(routes: Vec<(Method, &'static str, Answer)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(method, fragment, answer)| (Some(method), fragment, answer))
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
                .map_or(Answer::Silent, |(_, _, answer)| answer.clone()),
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

//! The transport every test drives a service through.
//!
//! One transport, scripted: the same answer to everything, a sequence in order, an answer
//! by what was asked for, or a test's own function for a request carried somewhere in
//! process. What it records is the same in every case: every request, in order, and the
//! octets of each that carried them, so a test can assert on the last one, on all of them,
//! or on whether something was ever asked for at all.
//!
//! A request nothing answers is unreachable rather than a default response. A test that
//! reaches an endpoint it did not script has found something, and quietly handing it a
//! `200` would hide it.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::http::{Fetched, Http, Method, Request, Response, Unreachable};

/// What the transport answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// A response with this status and body.
    Reply(u16, String),
    /// The same, said to have been served as this media type.
    ///
    /// For the code that reads what came back rather than only what it says. A
    /// service answering XML unless a caller asks for JSON is the ordinary case, and
    /// a fake that could not say what it served could not exercise the reading of it
    /// — which would leave the reading tested only against the answer it was already
    /// expecting.
    Served {
        /// The status.
        status: u16,
        /// What the `content-type` header carried.
        content_type: String,
        /// The body.
        body: String,
    },
    /// A response carrying these headers, for the code that reads a header out of what
    /// came back.
    Headed {
        /// The status.
        status: u16,
        /// Every header, in the order it is sent.
        headers: Vec<(String, String)>,
        /// The body.
        body: String,
    },
    /// Nothing at all — the service is not there.
    Silent,
    /// No answer, ever: the request is still waiting when whoever sent it gives up.
    Hanging,
    /// Bytes that need not be text, to a fetch; a send is answered by nothing, as a body
    /// that is not text cannot be carried as one.
    Octets {
        /// The status.
        status: u16,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// No answer, in the transport's own words.
    Unreached {
        /// Why nothing answered.
        reason: String,
        /// Whether a connection was made before the silence.
        connected: bool,
    },
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

    /// No answer, because no connection could be made, for this reason.
    #[must_use]
    pub fn unreached(reason: impl Into<String>) -> Self {
        Self::Unreached {
            reason: reason.into(),
            connected: false,
        }
    }

    /// No answer from a service that took the connection and then said nothing, for this
    /// reason.
    #[must_use]
    pub fn went_quiet(reason: impl Into<String>) -> Self {
        Self::Unreached {
            reason: reason.into(),
            connected: true,
        }
    }

    /// A response, said to have been served as this media type.
    #[must_use]
    pub fn served(status: u16, content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self::Served {
            status,
            content_type: content_type.into(),
            body: body.into(),
        }
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
    ByRoute(Vec<Route>),
    /// Handed to a test's own function, for a transport that carries a request somewhere
    /// in process rather than answering from a table.
    Handed(Handler),
}

/// What a transport handed to a test's own function answers with.
pub type Handled = Pin<Box<dyn Future<Output = Result<Response, Unreachable>> + Send>>;

/// A test's own function answering each request.
type Handler = Box<dyn Fn(Request) -> Handled + Send + Sync>;

/// What answers one request: a table's answer, or a test's own function.
enum Resolved<'a> {
    /// The answer a table gives.
    Answer(Answer),
    /// The function a test handed the request to.
    Handed(&'a Handler),
}

/// One route of a fake answering by what was asked for.
struct Route {
    /// The method it answers, where it answers only one.
    method: Option<Method>,
    /// What the URL it answers contains.
    fragment: &'static str,
    /// What the body it answers contains, where it answers only some bodies.
    body: Option<&'static str>,
    /// What it answers.
    replies: Replies,
}

impl Route {
    /// Whether it answers `request`.
    fn answers(&self, request: &Request) -> bool {
        request.url.contains(self.fragment)
            && self.method.is_none_or(|wanted| wanted == request.method)
            && self.body.is_none_or(|wanted| {
                request
                    .body
                    .as_deref()
                    .is_some_and(|body| body.contains(wanted))
            })
    }
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
    /// Whether a request carrying no octets is refused, as a push distributor refuses it.
    octets_only: bool,
    seen: Mutex<Vec<Request>>,
    octets: Mutex<Vec<(Request, Vec<u8>)>>,
}

/// What a fake taking only octets says to a request carrying none.
const ONLY_OCTETS: &str = "only octets are taken here";

/// What a fake answering bytes says to a send, which carries only text.
const ONLY_FETCHED: &str = "only fetched";

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

    /// No answer to anything, ever.
    #[must_use]
    pub fn hanging() -> Arc<Self> {
        Self::always(Answer::Hanging)
    }

    /// Each request handed to `handler`, which answers it.
    #[must_use]
    pub fn handing(handler: impl Fn(Request) -> Handled + Send + Sync + 'static) -> Arc<Self> {
        Self::new(Script::Handed(Box::new(handler)))
    }

    /// A push distributor: octets answered by what the URL contains, the first route
    /// matching, and every request carrying none refused.
    #[must_use]
    pub fn distributing(routes: Vec<(&'static str, Answer)>) -> Arc<Self> {
        Arc::new(Self {
            octets_only: true,
            ..Self::built(Script::ByRoute(
                routes
                    .into_iter()
                    .map(|(fragment, answer)| Route {
                        method: None,
                        fragment,
                        body: None,
                        replies: Replies::Same(answer),
                    })
                    .collect(),
            ))
        })
    }

    /// Each answer in turn.
    #[must_use]
    pub fn in_turn(answers: Vec<Answer>) -> Arc<Self> {
        Self::new(Script::InTurn(Mutex::new(answers.into())))
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
                .map(|(method, fragment, answer)| Route {
                    method,
                    fragment,
                    body: None,
                    replies: Replies::Same(answer),
                })
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
                .map(|(fragment, answers)| Route {
                    method: None,
                    fragment,
                    body: None,
                    replies: Replies::InTurn(Mutex::new(answers.into())),
                })
                .collect(),
        ))
    }

    /// By what the URL contains and, where a route names one, what the body contains,
    /// each route's answers in turn, the last repeating — for a contract posting every
    /// call of one operation to one address, where only what is posted tells one title
    /// or one page from another.
    #[must_use]
    pub fn by_call(routes: Vec<(&'static str, Option<&'static str>, Vec<Answer>)>) -> Arc<Self> {
        Self::new(Script::ByRoute(
            routes
                .into_iter()
                .map(|(fragment, body, answers)| Route {
                    method: None,
                    fragment,
                    body,
                    replies: Replies::InTurn(Mutex::new(answers.into())),
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
                .map(|(method, fragment, answers)| Route {
                    method: Some(method),
                    fragment,
                    body: None,
                    replies: Replies::InTurn(Mutex::new(answers.into())),
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

    /// Every request sent with octets, each beside the octets it carried, in order.
    pub fn octets(&self) -> Vec<(Request, Vec<u8>)> {
        self.octets
            .lock()
            .map(|octets| octets.clone())
            .unwrap_or_default()
    }

    fn new(script: Script) -> Arc<Self> {
        Arc::new(Self::built(script))
    }

    fn built(script: Script) -> Self {
        Self {
            script,
            octets_only: false,
            seen: Mutex::new(Vec::new()),
            octets: Mutex::new(Vec::new()),
        }
    }

    /// What answers this request; `octets` says whether it carries them.
    fn resolved(&self, request: &Request, octets: bool) -> Resolved<'_> {
        if self.octets_only && !octets {
            return Resolved::Answer(Answer::unreached(ONLY_OCTETS));
        }
        Resolved::Answer(match &self.script {
            Script::Always(answer) => answer.clone(),
            Script::InTurn(remaining) => remaining
                .lock()
                .ok()
                .and_then(|mut remaining| remaining.pop_front())
                .unwrap_or(Answer::Silent),
            Script::ByRoute(routes) => routes
                .iter()
                .find(|route| route.answers(request))
                .map_or(Answer::Silent, |route| route.replies.answer()),
            Script::Handed(handler) => return Resolved::Handed(handler),
        })
    }
}

#[async_trait]
impl Http for Fake {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        crate::noted(&self.seen, request.clone());
        responding(request, self.resolved(request, false)).await
    }

    async fn send_octets(&self, request: &Request, octets: &[u8]) -> Result<Response, Unreachable> {
        crate::noted(&self.seen, request.clone());
        crate::noted(&self.octets, (request.clone(), octets.to_vec()));
        responding(request, self.resolved(request, true)).await
    }

    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        crate::noted(&self.seen, request.clone());
        let (status, headers, bytes) = match self.resolved(request, false) {
            Resolved::Answer(Answer::Octets { status, bytes }) => (status, Vec::new(), bytes),
            resolved => {
                let response = responding(request, resolved).await?;
                (
                    response.status,
                    response.headers,
                    response.body.into_bytes(),
                )
            }
        };
        Ok(Fetched {
            status,
            headers,
            bytes: Some(bytes).filter(|bytes| bytes.len() <= most),
        })
    }
}

/// The response to this request, from whatever answers it.
async fn responding(request: &Request, resolved: Resolved<'_>) -> Result<Response, Unreachable> {
    match resolved {
        Resolved::Answer(Answer::Hanging) => std::future::pending().await,
        Resolved::Answer(answer) => answered(answer, request),
        Resolved::Handed(handler) => handler(request.clone()).await,
    }
}

/// The answer this fixture was built to give, as the port carries it.
fn answered(answer: Answer, request: &Request) -> Result<Response, Unreachable> {
    match answer {
        Answer::Reply(status, body) => Ok(Response {
            status,
            headers: Vec::new(),
            body,
        }),
        Answer::Served {
            status,
            content_type,
            body,
        } => Ok(Response {
            status,
            headers: vec![("content-type".to_owned(), content_type)],
            body,
        }),
        Answer::Headed {
            status,
            headers,
            body,
        } => Ok(Response {
            status,
            headers,
            body,
        }),
        Answer::Octets { .. } => Err(Unreachable::once(&request.url, ONLY_FETCHED)),
        Answer::Unreached { reason, connected } => Err(Unreachable {
            url: request.url.clone(),
            reason,
            attempts: 1,
            connected,
        }),
        Answer::Silent | Answer::Hanging => Err(Unreachable {
            url: request.url.clone(),
            reason: "connection refused".to_owned(),
            attempts: 1,
            connected: false,
        }),
    }
}

#[cfg(test)]
mod tests;

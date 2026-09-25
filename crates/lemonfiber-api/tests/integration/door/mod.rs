//! What every test about the door is built from.
//!
//! A shared module rather than a copy per file: these settle what a surface is, what
//! a request carries and what a household answers, and two copies of that would
//! answer the same question differently the first time one of them was updated.
pub(crate) use std::fs;
pub(crate) use std::path::PathBuf;
pub(crate) use std::sync::atomic::{AtomicBool, Ordering};
pub(crate) use std::sync::Arc;
pub(crate) use std::time::{Duration, SystemTime};

pub(crate) use axum::body::{to_bytes, Body};
pub(crate) use axum::extract::FromRequestParts as _;
pub(crate) use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
pub(crate) use lemonfiber_api::admission::sessions::Opened;
pub(crate) use lemonfiber_api::admission::{Admitting, Caller, Knocking, RETRY_AFTER, SESSION};
pub(crate) use lemonfiber_api::events::live::Live;
pub(crate) use lemonfiber_api::events::Streaming;
pub(crate) use lemonfiber_api::guard::{Binding, Token, TOKEN_HEADER};
pub(crate) use lemonfiber_api::jobs::Jobs;
pub(crate) use lemonfiber_api::router::{routes, Serving};
pub(crate) use lemonfiber_core::admission::{self as credential, Credential};
pub(crate) use lemonfiber_core::app::Ctx;
pub(crate) use lemonfiber_core::config::Settings;
pub(crate) use lemonfiber_core::ports::service::{
    Allowed, Certificate, Failure, Held, Household, Invited, Member, NamedLibrary,
};
pub(crate) use lemonfiber_fixtures::http::Fake;
pub(crate) use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
pub(crate) use lemonfiber_fixtures::support::a_password;
use tower::ServiceExt as _;

/// The second the stopped clock reads.
pub(crate) const NOW: u64 = 1_700_000_000;

/// The port this surface says it is listening on.
pub(crate) const PORT: u16 = 8471;

/// Serving this machine and nowhere else.
pub(crate) fn bound() -> Binding {
    Binding::here(PORT)
}

/// What a request has to say to have come from here.
pub(crate) fn from_here() -> Vec<(&'static str, String)> {
    vec![("host", format!("127.0.0.1:{PORT}"))]
}

/// The moment every one of these runs at.
pub(crate) fn moment() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(NOW)
}

/// A directory of this test's own, emptied first so a rerun starts fresh.
pub(crate) fn a_directory(named: &str) -> PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("door-{named}")).kept();
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// The password the operator chose, built rather than written down.
pub(crate) fn chosen() -> String {
    a_password()
}

/// The credential that password makes.
pub(crate) fn a_credential(password: &str) -> Credential {
    let Ok(held) = Credential::set(password, &Chance::cycling()) else {
        unreachable!("a full-length password and a source that answers make a record")
    };
    held
}

/// A machine keeping that password, and where it keeps it.
pub(crate) fn keeping(named: &str) -> PathBuf {
    let path = a_directory(named).join("admission.json");
    let held = a_credential(&chosen());
    let Ok(()) = credential::keep(&path, &held) else {
        unreachable!("a scratch directory can be written")
    };
    path
}

/// The world these requests run against: a stopped clock, and randomness a test chose.
pub(crate) fn world(admission: Option<PathBuf>, random: Chance) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::new(Idle))
        .clock(Stopped::at(NOW))
        .over(lemonfiber_testing::nowhere())
        .settings(Settings {
            admission,
            ..Settings::default()
        })
        .build()
        .with_http(Fake::silent())
        .with_random(Arc::new(random))
}

/// The whole surface over that world, and the register it admits from.
pub(crate) fn surface(ctx: Ctx, admitting: &Arc<Admitting>) -> (axum::Router, Arc<Token>) {
    let Some(token) = Token::mint(&Chance::cycling()).map(Arc::new) else {
        unreachable!("a cycling source always mints one")
    };
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let serving = Serving {
        ctx: Arc::new(ctx),
        token: Arc::clone(&token),
        bound: bound(),
        jobs: Jobs::default(),
        admitting: Arc::clone(admitting),
        live: Arc::clone(&live),
    };
    let streaming = Arc::new(Streaming {
        token: Arc::clone(&token),
        bound: bound(),
        admitting: Arc::clone(admitting),
        live,
        clock: Stopped::at(NOW),
    });
    (routes(serving, streaming), token)
}

/// The stream's route alone, merged without the layer that guards the rest.
///
/// **The assembly mistake, staged deliberately.** This route brings its own state
/// and can therefore be merged outside the guard — which is why it checks admission
/// a second time, and why that check cannot be reached through the assembled
/// surface, where the outer guard answers first.
pub(crate) fn stream_alone(admitting: &Arc<Admitting>) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()).map(Arc::new) else {
        unreachable!("a cycling source always mints one")
    };
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    lemonfiber_api::events::routes(Arc::new(Streaming {
        token,
        bound: bound(),
        admitting: Arc::clone(admitting),
        live,
        clock: Stopped::at(NOW),
    }))
}

/// A surface keeping the password at `path`, sharing one register with the test.
pub(crate) fn door(
    path: Option<PathBuf>,
    random: Chance,
) -> (axum::Router, Arc<Token>, Arc<Admitting>) {
    let admitting = Arc::new(Admitting {
        kept: path.clone(),
        ..Admitting::default()
    });
    let (router, token) = surface(world(path, random), &admitting);
    (router, token, admitting)
}

/// What one request was answered with: the status, the body, and how long is left.
pub(crate) struct Answer {
    /// The status it came back under.
    pub(crate) status: StatusCode,
    /// What it said.
    pub(crate) body: String,
    /// The wait it named, where it named one.
    pub(crate) left: Option<String>,
}

/// One request, and what it was answered with.
pub(crate) async fn asked(
    router: axum::Router,
    method: &str,
    path: &str,
    pairs: &[(&str, String)],
    body: &str,
) -> Answer {
    let mut building = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in pairs {
        building = building.header(*name, value);
    }
    let Ok(request) = building.body(Body::from(body.to_owned())) else {
        unreachable!("the request a test writes is one that can be built")
    };
    let Ok(response) = router.oneshot(request).await;
    let status = response.status();
    let left = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let Ok(read) = to_bytes(response.into_body(), 64 * 1024).await else {
        unreachable!("an answer this surface produces is one that can be read")
    };
    Answer {
        status,
        body: String::from_utf8_lossy(&read).into_owned(),
        left,
    }
}

/// The body a password is offered in.
pub(crate) fn offering(password: &str) -> String {
    format!("{{\"password\":{}}}", serde_json::json!(password))
}

/// The secret an answer handed back, where it handed one back.
pub(crate) fn session(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .as_ref()
        .and_then(|opened| opened.pointer("/data/token"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Who the envelope says the session is for, or nothing where it names nobody.
///
/// Read out of the answer rather than asserted against a whole body, because what
/// matters is the one field a client reads to know which application it is drawing.
pub(crate) fn whose(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .as_ref()
        .and_then(|opened| opened.pointer("/data/member"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// One request's headers, carrying the secret offered as the surface's own header.
pub(crate) fn carrying(secret: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(secret) = secret {
        let Ok(value) = HeaderValue::from_str(secret) else {
            return headers;
        };
        headers.insert(TOKEN_HEADER, value);
    }
    headers
}

/// A household that answers one question and refuses the rest.
///
/// Only `whoever` is reached from the door, and a stand-in answering more than the
/// thing under test is one that can pass a test the surface would fail.
pub(crate) struct AHousehold {
    /// Who a name and password prove somebody to be, where it recognises them.
    known: Option<String>,
    /// Whether asking it fails outright, which is a different answer from not
    /// recognising somebody.
    ///
    /// Shared and mutable for the reason `withdrawn` below is: a household that
    /// goes quiet *after* somebody signed in is the sequence worth staging, and it
    /// cannot be staged with two separate households.
    unreachable: AtomicBool,
    /// Whether the account it knows has since been taken off the server.
    ///
    /// Shared and mutable, so one test can sign somebody in and then remove them
    /// behind the same surface — which is the sequence the requirement is written
    /// about, and it cannot be staged with two separate households.
    withdrawn: AtomicBool,
}

impl AHousehold {
    /// One that knows this account and nobody else.
    pub(crate) fn knowing(id: &str) -> Arc<Self> {
        Arc::new(Self {
            known: Some(id.to_owned()),
            unreachable: AtomicBool::new(false),
            withdrawn: AtomicBool::new(false),
        })
    }

    /// Take the account off the server, as an operator removing somebody would.
    pub(crate) fn withdraw(&self) {
        self.withdrawn.store(true, Ordering::SeqCst);
    }

    /// Stop answering at all, as a media server being restarted would.
    pub(crate) fn go_dark(&self) {
        self.unreachable.store(true, Ordering::SeqCst);
    }

    /// One that recognises nobody.
    pub(crate) fn knowing_nobody() -> Arc<Self> {
        Arc::new(Self {
            known: None,
            unreachable: AtomicBool::new(false),
            withdrawn: AtomicBool::new(false),
        })
    }

    /// One that could not be asked at all.
    pub(crate) fn unreachable() -> Arc<Self> {
        Arc::new(Self {
            known: None,
            unreachable: AtomicBool::new(true),
            withdrawn: AtomicBool::new(false),
        })
    }
}

#[async_trait::async_trait]
impl Household for AHousehold {
    async fn whoever(&self, _: &str, _: &str) -> Result<Option<String>, Failure> {
        if self.unreachable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable {
                service: "jellyfin".to_owned(),
            });
        }
        Ok(self.known.clone())
    }

    async fn standing(&self, id: &str) -> Result<bool, Failure> {
        if self.unreachable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable {
                service: "jellyfin".to_owned(),
            });
        }
        if self.withdrawn.load(Ordering::SeqCst) {
            return Ok(false);
        }
        Ok(self.known.as_deref() == Some(id))
    }

    async fn household(&self) -> Result<Vec<Member>, Failure> {
        unreachable!("the door asks this household about one account and nothing else")
    }
    async fn invite(&self, _: &str) -> Result<Member, Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn unclaim(&self, _: &str) -> Result<(), Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn withdraw(&self, _: &str) -> Result<(), Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn when_invited(&self, _: &str) -> Result<Vec<Invited>, Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn holdings(&self, _: &str, _: u32) -> Result<Vec<Held>, Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn ratings(&self) -> Result<Vec<Certificate>, Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
    async fn allow(&self, _: &str, _: &Allowed) -> Result<(), Failure> {
        unreachable!("the door asks this household who somebody is and nothing else")
    }
}

/// A surface with a household behind it, minting its sessions from `random`.
///
/// **The source is named rather than defaulted, and that is load-bearing.**
/// `Chance::cycling()` answers identical bytes for identical widths, and a session
/// and the per-run token are both minted thirty-two bytes wide — so a test that let
/// both come from it would hand back a session string equal to the token, and every
/// request carrying it would be admitted as the machine before the session was ever
/// looked at. A test about a member would then be a test about the machine, and it
/// would pass.
pub(crate) fn door_with(
    path: Option<PathBuf>,
    household: Arc<AHousehold>,
    random: Chance,
) -> (axum::Router, Arc<Token>, Arc<Admitting>) {
    let admitting = Arc::new(Admitting {
        kept: path.clone(),
        household: Some(household),
        ..Admitting::default()
    });
    let (router, token) = surface(world(path, random), &admitting);
    (router, token, admitting)
}

/// A name and a password, as a member sends them.
pub(crate) fn offering_as(name: &str, password: &str) -> String {
    format!(
        "{{\"name\":{},\"password\":{}}}",
        serde_json::json!(name),
        serde_json::json!(password)
    )
}

/// A source that cannot hand out a session equal to the per-run token.
///
/// The token is minted inside the surface from `Chance::cycling()`; anything minting
/// sessions from the same source hands back the same thirty-two bytes, and the guard
/// tries the token first. Every test here would then be admitted as the machine and
/// would prove nothing about a member.
pub(crate) fn not_the_token() -> Chance {
    Chance::exactly(Some(vec![b'm'; 32]))
}

/// A member's own password, built rather than written.
///
/// Built for the reason the operator's is: a scan of this tree looking for a
/// committed credential reads a quoted password as one, and is right to — a rule
/// that made an exception for test files would make the exception exactly where a
/// real one is most likely to be pasted by mistake.
pub(crate) fn hers() -> String {
    ('b'..='m').collect()
}

/// A password that is nobody's, for the refusals.
pub(crate) fn nobodys() -> String {
    ('c'..='n').rev().collect()
}

//! Serving the web surface, and everything settled before it starts.
//!
//! Started when it is asked for and not before. Nothing installs it, nothing
//! keeps it running, and closing it leaves nothing behind — which is what lets a
//! surface that can start, stop and reconfigure the whole stack exist at all.
//!
//! Two things here reach the world at a point a test cannot follow: taking a
//! socket, and asking a desktop to open a browser. Neither is called directly.
//! The browser goes through the port every other program does, so a run that
//! cannot open one is an ordinary reply rather than a fault; and the loop that
//! holds the socket is given the signal that ends it, so it can be started, asked
//! something, and stopped, without a terminal in sight.
//!
//! The words are here and the printing is at the edge, so what an operator is
//! told is proven rather than demonstrated.

#[cfg(test)]
pub(crate) mod fixtures;
pub(crate) mod password;
pub(crate) mod reach;
pub(crate) mod said;

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use lemonfiber::cli::RawUi;
use lemonfiber_api::admission::Admitting;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::saying::Saying;
use lemonfiber_api::events::stepping::Stepping;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::frontend as serving;
use lemonfiber_api::guard::{Binding, Token};
use lemonfiber_api::jobs::{Jobs, LEASE};
use lemonfiber_api::router::{self, Serving};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::error::{Problem, Remedy, Severity, State as Standing};
use lemonfiber_core::frontend::Source;
use lemonfiber_core::platform::HOST_OS;
use lemonfiber_core::PRODUCT;
use tokio::net::TcpListener;

use crate::exit::complain;
use crate::say::say;
use crate::ui::reach::{address, held, permitted, unauthenticated, Offered, Reach};
use crate::ui::said::{announcement, opening, reverted, Browser};

use lemonfiber_core::error::codes::serve::NO_TOKEN;

/// What `ui` was asked for.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Asked {
    /// The port to listen on, or nothing to be given a free one.
    pub port: Option<u16>,
    /// Whether to ask this desktop to open a browser.
    pub browser: bool,
    /// A directory holding a built app, for a build that carries none.
    pub assets: Option<PathBuf>,
    /// Whether to ask for a password for this surface before starting it.
    pub password: bool,
    /// How far this surface was asked to be reachable.
    pub reach: Reach,
}

/// What is said to a request whose port is not a number a machine could listen on.
///
/// Beside the request rather than at the surface that took the word, for the reason a
/// read's own refusals are beside the read: what a port is is a fact about `ui` and
/// not about the screen or the shell that asked for one, and a second sentence
/// written at a second surface is how two surfaces come to refuse the same word
/// differently.
pub(crate) const NOT_A_PORT: &str = "Which port to listen on must be a number from 0 to 65535.";

impl Asked {
    /// What `lemonfiber ui` is given where no flag says otherwise.
    ///
    /// Apart from [`Default`], which is the conservative reading of the same three: a
    /// browser is opened only where something asked for one, and nothing has yet.
    /// This is what the flags come to when none of them is given, and it is held to
    /// them by a test rather than by two lists that agree today.
    pub(crate) const fn unsaid() -> Self {
        Self {
            port: None,
            browser: true,
            assets: None,
            password: false,
            reach: Reach::Machine,
        }
    }

    /// The same, listening on the port a word names.
    ///
    /// Naming nothing is a request in its own right and not an omission: it asks for
    /// whichever port is free, which is what naming no `--port` asks for.
    ///
    /// # Errors
    ///
    /// Returns what is said to a word that is not a port. Whether that port can be
    /// listened on is a different question and is not answered here, because it
    /// cannot be answered anywhere else either: a port is free until something takes
    /// it, so the answer is [`taken`], which takes it and reports what happened.
    pub(crate) fn on_port(&self, said: &str) -> Result<Self, &'static str> {
        let said = said.trim();
        let named = (!said.is_empty()).then_some(said);
        let Ok(port) = named.map(str::parse::<u16>).transpose() else {
            return Err(NOT_A_PORT);
        };
        Ok(Self {
            port,
            ..self.clone()
        })
    }

    /// The same, serving the interface out of the directory a word names.
    ///
    /// Naming nothing is a request here too: it asks for the interface this program
    /// was built with, which is what naming no `--assets` asks for.
    pub(crate) fn serving_from(&self, said: &str) -> Self {
        let said = said.trim();
        Self {
            assets: (!said.is_empty()).then(|| PathBuf::from(said)),
            ..self.clone()
        }
    }

    /// The same, with the browser turned over.
    pub(crate) fn turned(&self) -> Self {
        Self {
            browser: !self.browser,
            ..self.clone()
        }
    }

    /// The same, with the question about a password turned over.
    pub(crate) fn asking(&self) -> Self {
        Self {
            password: !self.password,
            ..self.clone()
        }
    }

    /// The same, with how far it may be reached turned over.
    pub(crate) fn reaching(&self) -> Self {
        Self {
            reach: match self.reach {
                Reach::Machine => Reach::Network,
                Reach::Network => Reach::Machine,
            },
            ..self.clone()
        }
    }
}

impl From<RawUi> for Asked {
    /// What the flags come to.
    ///
    /// Here rather than at the command line, so the one translation from what was
    /// typed into what this surface is given is under the coverage gate — `main.rs`
    /// is the outermost edge and is not.
    fn from(raw: RawUi) -> Self {
        Self {
            port: raw.port,
            browser: !raw.no_browser,
            assets: raw.assets,
            password: raw.set_password,
            reach: if raw.lan {
                Reach::Network
            } else {
                Reach::Machine
            },
        }
    }
}

/// Where the app being served comes from, or nothing where there is none.
///
/// A build carries the app it was built with. Naming a directory instead is for
/// somebody building the app itself, and the surface below this stops being able
/// to tell the difference.
pub(crate) fn app(embedded: Option<Source>, assets: Option<PathBuf>) -> Option<Source> {
    match assets {
        // The path outlives the process and `Source` is Copy, so leaking one
        // allocation at startup buys both — the same trade the stack directory
        // makes.
        Some(path) => Some(Source::External(Box::leak(path.into_boxed_path()))),
        None => embedded,
    }
}

/// Everything this surface answers, with the endpoints guarded and the app not.
///
/// The app itself carries no token: a browser opening a page sends no header of
/// ours, and it is the page that goes on to ask for one. Everything below `/api`
/// is a different matter, and admission is asked once above all of it in the
/// router, so an endpoint added later cannot arrive unguarded.
///
/// The app is merged over the guarded tree rather than under it, because its
/// fallback answers every path the endpoints did not — which is what a client-side
/// router needs, and would swallow the endpoints if it sat beneath them.
pub(crate) fn surface(serving: Serving, streaming: Arc<Streaming>, app: Option<Source>) -> Router {
    serving::routes(app).merge(router::routes(serving, streaming))
}

/// The signal that ends the serving loop.
///
/// Boxed rather than left generic. A generic taken at two call sites is two
/// functions, and the one a real run instantiates is a second copy of the lines
/// below that no test can enter — which reads as untested code and is not.
pub(crate) type Until = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Start the surface and hold it until the signal arrives.
///
/// The signal is taken rather than chosen so this can be started, asked
/// something, and stopped, without a terminal — a real run is handed one that
/// never arrives, and the operator ends the process the way they started it.
pub(crate) async fn run(
    ctx: Ctx,
    asked: Asked,
    answers: &dyn crate::prompt::Answers,
    embedded: Option<Source>,
    until: Until,
) -> ExitCode {
    // Before the socket rather than after it. A run asked for a password and given
    // one it could not keep has not been given what it asked for, and serving anyway
    // would put the surface up under the arrangement the operator was trying to
    // change.
    //
    // Kept out of the serving loop rather than folded into it, because a person at a
    // keyboard is not something a task sent to another thread may hold — and the
    // loop below is spawned. So the asking finishes before the loop begins, which is
    // also the order an operator reads it in.
    match asked.password.then(|| asking(&ctx, answers)).flatten() {
        Some(code) => code,
        None => serving(ctx, asked, embedded, until, LOOK).await,
    }
}

/// Set the password, and say what to exit with where it could not be set.
///
/// Nothing where it was set, because there is nothing to exit with yet: the surface
/// still has to be served, and the words said here are the ones an operator reads
/// above the announcement.
fn asking(ctx: &Ctx, answers: &dyn crate::prompt::Answers) -> Option<ExitCode> {
    match password::set(
        answers,
        ctx.seams.random.as_ref(),
        ctx.settings.admission.as_deref(),
    ) {
        Ok(lines) => {
            for line in lines {
                say!("{line}");
            }
            say!("");
            None
        }
        Err(problem) => Some(complain(&problem)),
    }
}

/// How often a surface offered to a network looks again at whether it may still be.
///
/// Five seconds. What it costs is a look at one small file; what it buys is that a
/// password removed while this is running is a socket given up in the time it takes
/// to notice, rather than at the next restart. It is not what makes the removal
/// immediate — nothing on the network is admitted from the moment the password goes,
/// because every session was opened against it — it is what makes the *binding*
/// follow the authority rather than outliving it.
pub(crate) const LOOK: Duration = Duration::from_secs(5);

/// What ended a run of the serving loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// The signal the operator sends.
    Stopped,
    /// The password this binding rested on went away.
    Revoked,
}

/// Serve, and go on serving until the operator stops it or the policy turns.
///
/// The policy is read once before a socket exists and again while one is held, and it
/// is the same reading both times. What differs is what a `no` means: before, there is
/// nothing to fall back to, so the request is refused; after, refusing outright would
/// take the surface away from the operator too, so it falls back to the address it
/// would have been given and says why.
async fn serving(
    ctx: Ctx,
    asked: Asked,
    embedded: Option<Source>,
    mut until: Until,
    look: Duration,
) -> ExitCode {
    let Some(token) = Token::mint(ctx.seams.random.as_ref()) else {
        return complain(&tokenless());
    };
    // The one gather every listener hears, made before the context so that the
    // waits a command runs into have somewhere to say what they are waiting for:
    // a browser is told the name of the work and nothing else, and everything it
    // learns after that arrives here.
    let live = Arc::new(Live::opening(ctx.seams.clock.as_ref()));
    // A walk's steps go down the same stream, whole rather than rendered: the words
    // are the core's, and a second rendering of them here would be a second copy of
    // the walk's own prose beside the one the terminal draws.
    let (steps, carrying) = Stepping::onto(Arc::clone(&live));
    let ctx = ctx
        .with_narrator(Arc::new(Saying::onto(Arc::clone(&live))))
        .with_steps(Arc::new(steps));
    let (ctx, token) = (Arc::new(ctx), Arc::new(token));
    tokio::spawn(carrying.carrying());
    // Started before anything can ask to hear it, so a client that connects at
    // once is not waiting on a first pass that has not been asked for.
    tokio::spawn(Arc::clone(&live).gathering(Arc::new(
        lemonfiber_api::events::dashboard::Dashboard::against(Arc::clone(&ctx)),
    )));
    let jobs = Jobs::default();
    // Work with no ending of its own is held only while somebody is still asking
    // about it, so a guard whose browser went away is let go rather than left
    // polling a drive until this process stops.
    tokio::spawn(jobs.clone().sweeping(LEASE));
    // One register, shared by the door, the guard over everything else and the
    // stream: two would be a run somebody could be admitted to half of. It reads the
    // password afresh every time it is asked, which is what lets the loop below ask
    // again without anything having to tell it.
    let admitting = Arc::new(Admitting {
        kept: ctx.settings.admission.clone(),
        ..Admitting::default()
    });
    let app = app(embedded, asked.assets.clone());

    let mut reach = asked.reach;
    let mut browsing = asked.browser;
    loop {
        let offered = permitted(reach, admitting.credential().is_some());
        if offered == Offered::Refused {
            return complain(&Box::new(unauthenticated()));
        }
        let sockets = match held(offered, asked.port).await {
            Ok(sockets) => sockets,
            Err(problem) => return complain(&problem),
        };
        let at: Vec<SocketAddr> = sockets.iter().map(|(_, bound)| *bound).collect();
        let bound = Binding {
            port: at.first().map_or(0, SocketAddr::port),
            beyond: offered == Offered::Network,
        };
        let browser = match (browsing, at.first()) {
            (true, Some(first)) => {
                opening(ctx.seams.runner.as_ref(), HOST_OS, &address(*first)).await
            }
            _ => Browser::Unasked,
        };
        // Only ever the first time round: an operator whose binding reverted is
        // already looking at the terminal that said so, and a second window is not
        // what they asked for.
        browsing = false;
        for line in announcement(&at, offered, token.as_str(), browser) {
            say!("{line}");
        }
        let serving = Serving {
            ctx: Arc::clone(&ctx),
            token: Arc::clone(&token),
            bound,
            jobs: jobs.clone(),
            admitting: Arc::clone(&admitting),
            live: Arc::clone(&live),
        };
        let streaming = Arc::new(Streaming {
            token: Arc::clone(&token),
            bound,
            admitting: Arc::clone(&admitting),
            live: Arc::clone(&live),
            clock: Arc::clone(&ctx.seams.clock),
        });
        let surface = surface(serving, streaming, app);
        match holding(sockets, surface, &admitting, offered, &mut until, look).await {
            Ending::Stopped => return ExitCode::SUCCESS,
            Ending::Revoked => {
                for line in reverted() {
                    say!("{line}");
                }
                reach = Reach::Machine;
            }
        }
    }
}

/// Hold every socket that was taken until one of the two endings arrives.
///
/// Whatever ends it, the sockets are given up before this returns, so the next time
/// round the loop is not asking for a port this run is still holding. A fault from
/// accepting on a socket this process already holds means the process is going down
/// around it, and a second message about one event helps nobody.
async fn holding(
    sockets: Vec<(TcpListener, SocketAddr)>,
    surface: Router,
    admitting: &Arc<Admitting>,
    offered: Offered,
    until: &mut Until,
    look: Duration,
) -> Ending {
    let (stopping, stopped) = tokio::sync::watch::channel(false);
    let mut running = Vec::new();
    for (listener, _) in sockets {
        let mut leaving = stopped.clone();
        let held = surface.clone();
        running.push(tokio::spawn(async move {
            let _ = axum::serve(listener, held)
                .with_graceful_shutdown(async move {
                    let _ = leaving.changed().await;
                })
                .await;
        }));
    }
    let ending = tokio::select! {
        () = &mut *until => Ending::Stopped,
        () = revoked(Arc::clone(admitting), offered, look) => Ending::Revoked,
    };
    let _ = stopping.send(true);
    for server in running {
        let _ = server.await;
    }
    ending
}

/// Wait until the password this binding rests on is gone.
///
/// Never, where it rests on none: a surface answering this machine alone is offered
/// to nobody a password would have kept out, so there is nothing for its going to
/// take away.
async fn revoked(admitting: Arc<Admitting>, offered: Offered, look: Duration) {
    match offered {
        Offered::Machine | Offered::Refused => std::future::pending().await,
        Offered::Network => {
            while admitting.credential().is_some() {
                tokio::time::sleep(look).await;
            }
        }
    }
}

/// The token could not be minted.
fn tokenless() -> Problem {
    Problem::new(
        NO_TOKEN,
        Severity::Error,
        format!("{PRODUCT} could not mint a token for this run"),
        "Every request to this surface has to carry a secret that only this run knows, and \
         this machine would not supply the unpredictable bytes one is made of.",
        Remedy::new(
            "Try again, and if it happens twice the operating system's own random \
                     source is at fault",
        ),
    )
    .in_state(Standing::Guided)
}

#[cfg(test)]
mod tests;

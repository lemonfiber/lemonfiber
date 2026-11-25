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

mod answering;
pub(crate) mod encrypted;
#[cfg(test)]
pub(crate) mod fixtures;
mod holding;
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
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::saying::Saying;
use lemonfiber_api::events::stepping::Stepping;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::frontend as serving;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::{Jobs, LEASE};
use lemonfiber_api::router::{self, Own, Serving};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Surfacing};
use lemonfiber_core::companion::served::{Served, ServedReport};
use lemonfiber_core::frontend::Source;
use lemonfiber_core::platform::HOST_OS;
use lemonfiber_core::ports::asking::Asking;
use lemonfiber_core::PRODUCT;
use lemonfiber_error::{Problem, Remedy, State as Standing};

use crate::exit::complain;
use crate::say::say;
use crate::ui::reach::{address, held, permitted, unauthenticated, Offered, Reach};
use crate::ui::said::{announcement, opening, reverted, Browser};
use holding::{holding, Held};

use lemonfiber_error::codes::serve::NO_TOKEN;

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
    /// Whether it is served encrypted, with the certificate this program keeps.
    pub tls: bool,
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
            tls: false,
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
            tls: raw.transport.tls,
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
    serving::routes(app, Arc::clone(&serving.ctx)).merge(router::routes(serving, streaming))
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
    asking: Arc<dyn Asking>,
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
    let refused = if asked.password {
        password(&ctx, asking).await
    } else {
        None
    };
    match refused {
        Some(code) => code,
        None => serving(ctx, own(), asked, embedded, until, LOOK).await,
    }
}

/// Set the password, and say what to exit with where it could not be set.
///
/// Nothing where it was set, because there is nothing to exit with yet: the surface
/// still has to be served, and the words said here are the ones an operator reads
/// above the announcement.
async fn password(ctx: &Ctx, asking: Arc<dyn Asking>) -> Option<ExitCode> {
    match dispatch(Command::Password, &ctx.clone().with_asking(asking)).await {
        Ok(_) => {
            for line in password::said() {
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

/// Whether the operator's password is kept, read afresh at every asking.
fn password_kept(ctx: &Ctx) -> bool {
    ctx.settings
        .admission
        .as_deref()
        .and_then(lemonfiber_core::admission::Credential::at)
        .is_some()
}

/// How the web surface is served on `port`, settled through the dispatcher, and the
/// certificate it encrypts with where it does.
async fn settled(
    ctx: &Ctx,
    port: u16,
    encrypted: bool,
    network: bool,
) -> Result<(ServedReport, Option<encrypted::Encrypting>), Box<Problem>> {
    let settling = Command::Served(Served {
        port,
        encrypted,
        network,
    });
    let served = served_by(dispatch(settling, ctx).await?);
    let encrypting = encrypted::encrypting(&served)?;
    Ok((served, encrypting))
}

/// How the web surface is served, as settling it answered; served under nothing where
/// the answer is another.
fn served_by(answered: Outcome) -> ServedReport {
    match answered {
        Outcome::Served(report) => report,
        _ => ServedReport::default(),
    }
}

/// The clock and the randomness this run's web surface holds of its own: the machine's,
/// the randomness being the operating system's CSPRNG, because the token and every job id
/// are drawn from it and a guessed one opens what it guards.
fn own() -> Own {
    Own {
        clock: Arc::new(lemonfiber_adapters::System),
        random: Arc::new(lemonfiber_adapters::Os),
    }
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
    own: Own,
    asked: Asked,
    embedded: Option<Source>,
    mut until: Until,
    look: Duration,
) -> ExitCode {
    let Some(token) = Token::mint(own.random.as_ref()) else {
        return complain(&tokenless());
    };
    // The one gather every listener hears, made before the context so that the
    // waits a command runs into have somewhere to say what they are waiting for:
    // a browser is told the name of the work and nothing else, and everything it
    // learns after that arrives here.
    let live = Arc::new(Live::opening(Arc::clone(&own.clock)));
    // A walk's steps go down the same stream, whole rather than rendered: the words
    // are the core's, and a second rendering of them here would be a second copy of
    // the walk's own prose beside the one the terminal draws.
    let (steps, carrying) = Stepping::onto(Arc::clone(&live));
    let ctx = ctx
        .with_narrator(Arc::new(Saying::onto(Arc::clone(&live))))
        .with_steps(Arc::new(steps));
    let (ctx, token) = (Arc::new(ctx), Arc::new(token));
    tokio::spawn(carrying.carrying());
    // Started before anything can ask to hear it. It gathers nothing until somebody
    // is listening, and gathers at once for the first to arrive.
    tokio::spawn(Arc::clone(&live).gathering(Arc::new(
        lemonfiber_api::events::extending::Together::in_a_run(&ctx),
    )));
    let jobs = Jobs::default();
    // Work with no ending of its own is held only while somebody is still asking
    // about it, so a guard whose browser went away is let go rather than left
    // polling a drive until this process stops.
    tokio::spawn(jobs.clone().sweeping(LEASE));
    tokio::spawn(untuning(Arc::clone(&ctx)));
    let app = app(embedded, asked.assets.clone());
    if let Err(problem) = encrypted::port_settled(asked.tls, asked.port) {
        return complain(&problem);
    }

    let mut reach = asked.reach;
    let mut browsing = asked.browser;
    loop {
        let offered = permitted(reach, password_kept(&ctx));
        if offered == Offered::Refused {
            return complain(&Box::new(unauthenticated()));
        }
        let sockets = match held(offered, asked.port).await {
            Ok(sockets) => sockets,
            Err(problem) => return complain(&problem),
        };
        let at: Vec<SocketAddr> = sockets.iter().map(|(_, bound)| *bound).collect();
        let port = at.first().map_or(0, SocketAddr::port);
        let network = offered == Offered::Network;
        let (served, encrypting) = match settled(&ctx, port, asked.tls, network).await {
            Ok(settled) => settled,
            Err(problem) => return complain(&problem),
        };
        let bound = encrypted::bound(&served, port, network);
        let browser = match (browsing, at.first()) {
            (true, Some(first)) => {
                opening(
                    &lemonfiber_adapters::Local,
                    HOST_OS,
                    &address(*first, encrypting.is_some()),
                )
                .await
            }
            _ => Browser::Unasked,
        };
        // Only ever the first time round: an operator whose binding reverted is
        // already looking at the terminal that said so, and a second window is not
        // what they asked for.
        browsing = false;
        let fingerprint = encrypting.as_ref().map(|on| on.fingerprint.as_str());
        for line in announcement(&at, offered, token.as_str(), browser, fingerprint) {
            say!("{line}");
        }
        for line in unrecorded(served.unrecorded.as_deref()) {
            say!("{line}");
        }
        let serving = Serving::for_run(
            Arc::clone(&ctx),
            Arc::clone(&token),
            bound.clone(),
            jobs.clone(),
            Arc::clone(&live),
            own.clone(),
        );
        let streaming = Arc::new(Streaming {
            token: Arc::clone(&token),
            bound,
            live: Arc::clone(&live),
            reading: Arc::clone(&ctx),
        });
        let surface = surface(serving, streaming, app);
        let held = Held {
            sockets,
            surface,
            offered,
            tls: encrypting.as_ref().map(|on| on.acceptor.clone()),
        };
        match holding(held, &live, &ctx, &mut until, look).await {
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

/// What is said where how this run is served could not be written down, so a phone
/// cannot be paired with it; nothing where it was.
fn unrecorded(why: Option<&str>) -> Vec<String> {
    why.map(|why| {
        vec![
            String::new(),
            format!(
                "Where this is served could not be written down, so a phone cannot be \
                 paired with it: {why}"
            ),
        ]
    })
    .unwrap_or_default()
}

/// Close the live streams whose passes lapsed unrenewed, every
/// [`lemonfiber_core::screening::UNTUNED_EVERY`] for as long as this surface serves.
async fn untuning(ctx: Arc<Ctx>) {
    loop {
        tokio::time::sleep(lemonfiber_core::screening::UNTUNED_EVERY).await;
        drop(dispatch(Command::Surface(Surfacing::Untune), &ctx).await);
    }
}

/// The token could not be minted.
fn tokenless() -> Problem {
    Problem::new(
        NO_TOKEN,
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

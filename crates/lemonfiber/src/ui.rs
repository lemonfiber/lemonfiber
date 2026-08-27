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
use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State as Standing};
use lemonfiber_core::frontend::Source;
use lemonfiber_core::platform::HOST_OS;
use lemonfiber_core::PRODUCT;
use tokio::net::TcpListener;

use crate::exit::complain;
use crate::say::say;
use crate::ui::reach::{address, held, permitted, unauthenticated, Offered, Reach};
use crate::ui::said::{announcement, opening, reverted, Browser};

/// Raised when this machine will not supply the randomness a token is made of.
const NO_TOKEN: Code = Code::new("SERVE-2");

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
        ctx.random.as_ref(),
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
const LOOK: Duration = Duration::from_secs(5);

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
    let Some(token) = Token::mint(ctx.random.as_ref()) else {
        return complain(&tokenless());
    };
    // The one gather every listener hears, made before the context so that the
    // waits a command runs into have somewhere to say what they are waiting for:
    // a browser is told the name of the work and nothing else, and everything it
    // learns after that arrives here.
    let live = Arc::new(Live::opening(ctx.clock.as_ref()));
    // A walk's steps go down the same stream, whole rather than rendered: the words
    // are the core's, and a second rendering of them here would be a second copy of
    // the walk's own prose beside the one the terminal draws.
    let (steps, carrying) = Stepping::onto(Arc::clone(&live));
    let ctx = ctx
        .narrating(Arc::new(Saying::onto(Arc::clone(&live))))
        .narrating_steps(Arc::new(steps));
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
            (true, Some(first)) => opening(ctx.runner.as_ref(), HOST_OS, &address(*first)).await,
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
mod tests {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{HeaderValue, StatusCode};
    use axum::Router;
    use lemonfiber_api::admission::Admitting;
    use lemonfiber_api::events::live::Live;
    use lemonfiber_api::events::Streaming;
    use lemonfiber_api::guard::Binding;
    use lemonfiber_api::guard::Token;
    use lemonfiber_api::jobs::Jobs;
    use lemonfiber_api::router::Serving;
    use lemonfiber_core::app::Ctx;
    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::{Environment, HostOs};
    use lemonfiber_core::ports::process::Runner;
    use lemonfiber_fixtures::ports::{Chance, Idle};

    use super::fixtures::{bound, exited, missing};
    use super::reach::{held, Offered, Reach};
    use super::said::opening;
    use super::LOOK;
    use super::{address, app, run, serving, surface, tokenless, Asked, Browser, NOT_A_PORT};
    use clap::Parser as _;
    use lemonfiber_core::admission::credential;
    use lemonfiber_fixtures::support::a_password;
    use std::path::PathBuf;

    use crate::prompt::fixtures::Script;

    #[test]
    fn a_named_directory_is_the_app_instead_of_the_embedded_one() {
        let named = app(None, Some("/srv/app".into()));
        assert!(
            matches!(named, Some(lemonfiber_core::frontend::Source::External(path))
                if path == std::path::Path::new("/srv/app"))
        );
    }

    #[test]
    fn a_build_with_no_app_and_no_directory_has_none() {
        assert!(app(None, None).is_none());
    }

    #[test]
    fn a_token_this_machine_will_not_supply_is_reported_rather_than_invented() {
        let problem = tokenless();
        assert!(!problem.remedies.is_empty(), "somewhere to go");
        assert!(problem.summary.contains("token"), "{}", problem.summary);
    }

    /// What a command line comes to, or nothing where it is not a request to serve.
    ///
    /// Through the parser rather than by building the flags here: what these pin is
    /// the command line's own defaults, and defaults written down a second time
    /// agree today and drift on the day one of them changes.
    fn asked_for(said: &[&str]) -> Option<Asked> {
        let parsed = lemonfiber::cli::Cli::try_parse_from(said).ok()?;
        match parsed.command {
            Some(lemonfiber::cli::Request::Ui(raw)) => Some(Asked::from(raw)),
            _ => None,
        }
    }

    /// The screen and the command line start from the same place, or the two presses
    /// that key has always taken would quietly mean something else.
    #[test]
    fn asking_for_nothing_is_what_the_screen_starts_from() {
        assert_eq!(asked_for(&["lemonfiber", "ui"]), Some(Asked::unsaid()));
    }

    /// Each flag reaches the choice it names, since a flag that reaches none is a
    /// flag that silently does nothing.
    #[test]
    fn each_flag_reaches_the_choice_it_names() {
        assert_eq!(
            asked_for(&[
                "lemonfiber",
                "ui",
                "--port",
                "7171",
                "--no-browser",
                "--assets",
                "/srv/app",
                "--set-password",
                "--lan",
            ]),
            Some(Asked {
                port: Some(7171),
                browser: false,
                assets: Some(PathBuf::from("/srv/app")),
                password: true,
                reach: Reach::Network,
            })
        );
    }

    /// A port the command line will not read never reaches this surface at all,
    /// which is the shell's half of the check the screen makes for itself.
    #[test]
    fn a_port_the_command_line_will_not_read_never_reaches_this_surface() {
        assert_eq!(asked_for(&["lemonfiber", "ui", "--port", "seventy"]), None);
        assert_eq!(asked_for(&["lemonfiber", "version"]), None);
    }

    /// A word is a port or it is refused, rather than rounded to something.
    #[test]
    fn a_word_typed_where_a_port_goes_is_one_or_it_is_refused() {
        let asked = Asked::unsaid();

        for (said, port) in [("7171", 7171), (" 7171 ", 7171), ("0", 0)] {
            assert_eq!(
                asked.on_port(said).map(|asked| asked.port),
                Ok(Some(port)),
                "{said:?}"
            );
        }
        for said in ["seventy", "-1", "65536", "71.71", "7171x"] {
            assert_eq!(
                asked.on_port(said).map(|asked| asked.port),
                Err(NOT_A_PORT),
                "{said:?}"
            );
        }
    }

    /// Naming no port is a request rather than an omission: it asks for whichever
    /// one is free, which is what naming no flag asks for.
    #[test]
    fn naming_no_port_at_a_screen_asks_for_whichever_one_is_free() {
        for said in ["", "   "] {
            assert_eq!(
                Asked::unsaid().on_port(said).map(|asked| asked.port),
                Ok(None),
                "{said:?}"
            );
        }
    }

    /// Naming no directory is the interface this program was built with, which is
    /// what naming no flag asks for.
    #[test]
    fn naming_no_directory_is_the_interface_built_into_this_program() {
        for said in ["", "  "] {
            assert!(
                Asked::unsaid().serving_from(said).assets.is_none(),
                "{said:?}"
            );
        }
    }

    /// Filling one choice leaves the other four where they were, or a screen setting
    /// a port would be taking a browser away with it.
    #[test]
    fn filling_one_choice_leaves_the_other_four_alone() {
        let asked = Asked::unsaid()
            .serving_from("/srv/app")
            .turned()
            .asking()
            .reaching();

        assert_eq!(
            asked.on_port("7171"),
            Ok(Asked {
                port: Some(7171),
                browser: false,
                assets: Some(PathBuf::from("/srv/app")),
                password: true,
                reach: Reach::Network,
            })
        );
        assert_eq!(
            asked.turned().asking().reaching(),
            Asked::unsaid().serving_from("/srv/app")
        );
    }

    #[test]
    fn asking_for_nothing_in_particular_asks_for_nothing_in_particular() {
        let asked = Asked::default();
        assert_eq!(asked.port, None);
        assert!(
            !asked.browser,
            "a browser is opened only where it is wanted"
        );
        assert!(asked.assets.is_none());
    }

    // ── Starting the whole of it, and stopping it again ───────────────────────

    /// A context over the stack this binary ships, with the randomness a test
    /// chose and the runner it wants every program answered by.
    fn running(runner: Arc<dyn Runner>, bytes: Option<Vec<u8>>, settings: Settings) -> Ctx {
        Ctx::new(
            runner,
            Arc::new(lemonfiber_core::adapters::Daemon::local()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            lemonfiber_core::stack::Source::Embedded(&lemonfiber::cli::STACK),
            settings,
            Environment::MacOs,
        )
        .with_random(Arc::new(Chance::exactly(bytes)))
    }

    /// The same, over a runner that spawns nothing.
    fn ctx(bytes: Option<Vec<u8>>) -> Ctx {
        running(Arc::new(Idle), bytes, Settings::default())
    }

    /// The same, keeping a password wherever a test says.
    fn keeping(admission: Option<PathBuf>) -> Ctx {
        running(
            Arc::new(Idle),
            Some(enough()),
            Settings {
                admission,
                ..Settings::default()
            },
        )
    }

    /// A directory of this test's own, emptied first so a rerun starts fresh.
    fn a_directory(named: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-ui-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Bytes enough to mint a token from.
    fn enough() -> Vec<u8> {
        vec![0x0a; 32]
    }

    /// The token those bytes are written as, which a test sends back.
    fn written() -> String {
        "0a".repeat(32)
    }

    /// Start the surface, stop it at once, and say what it exited with.
    async fn started(ctx: Ctx, asked: Asked) -> String {
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(serving(
            ctx,
            asked,
            None,
            Box::pin(async move {
                let _ = stopped.await;
            }),
            LOOK,
        ));
        // Let the loop reach the socket before it is asked to leave it, so this
        // proves a surface that started rather than one that never did.
        tokio::task::yield_now().await;
        let _ = stop.send(());
        running.await.map(crate::exit::shown).unwrap_or_default()
    }

    #[tokio::test]
    async fn a_surface_that_started_stops_cleanly_when_it_is_asked_to() {
        assert_eq!(
            started(ctx(Some(enough())), Asked::default()).await,
            crate::exit::shown(ExitCode::SUCCESS)
        );
    }

    #[tokio::test]
    async fn no_way_of_failing_to_open_a_browser_fails_the_command() {
        // The two ways it goes wrong: the program is not there at all, and the program
        // is there and would not do it. Each is checked to be the failing shape and
        // then driven through the whole of a run — a mapping that quietly called one of
        // them success would otherwise leave this passing on a run that never met a
        // browser which would not open.
        let asked = Asked {
            browser: true,
            ..Asked::default()
        };
        for runner in [missing(), exited(1)] {
            assert_eq!(
                opening(&runner, HostOs::Linux, &address(bound())).await,
                Browser::Unopened
            );
            assert_eq!(
                started(
                    running(Arc::new(runner), Some(enough()), Settings::default()),
                    asked.clone()
                )
                .await,
                crate::exit::shown(ExitCode::SUCCESS)
            );
        }
    }

    #[tokio::test]
    async fn a_port_already_held_stops_the_run_rather_than_serving_on_another() {
        // Held for the length of this test, and asked for again by name. If the
        // first take had failed the port would be absent and the run would
        // succeed, which this would then report — a wrong answer either way is
        // an assertion that fails, never a branch nothing runs.
        let taken = held(Offered::Machine, None).await.ok();
        let asked = Asked {
            port: taken
                .as_ref()
                .and_then(|taken| taken.first())
                .map(|(_, bound)| bound.port()),
            ..Asked::default()
        };
        let code = serving(
            ctx(Some(enough())),
            asked,
            None,
            Box::pin(std::future::ready(())),
            LOOK,
        )
        .await;
        assert_ne!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS)
        );
        drop(taken);
    }

    #[tokio::test]
    async fn a_machine_that_will_not_supply_randomness_serves_nothing() {
        // A surface whose token could not be minted would be one every request
        // reached, so there is nothing here to fall back to.
        let code = serving(
            ctx(None),
            Asked::default(),
            None,
            Box::pin(std::future::ready(())),
            LOOK,
        )
        .await;
        assert_ne!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS)
        );
    }

    // ── What a request to the running surface meets ───────────────────────────

    /// The surface a run builds, or nothing where the machine gave no token.
    ///
    /// Both answers are asked for below, so neither is a line nothing runs —
    /// this module is under the same coverage gate as the code it tests.
    fn as_served(random: &Chance) -> Option<Router> {
        let token = Arc::new(Token::mint(random)?);
        let live = Arc::new(Live::opening(
            lemonfiber_fixtures::ports::Stopped::at(0).as_ref(),
        ));
        let admitting = Arc::new(Admitting::default());
        let serving = Serving {
            ctx: Arc::new(ctx(Some(enough()))),
            token: Arc::clone(&token),
            bound: Binding::here(bound().port()),
            jobs: Jobs::default(),
            admitting: Arc::clone(&admitting),
            live: Arc::clone(&live),
        };
        let streaming = Arc::new(Streaming {
            token,
            bound: Binding::here(bound().port()),
            admitting,
            live,
        });
        Some(surface(serving, streaming, None))
    }

    /// A request to the running surface, carrying what the test chose.
    ///
    /// Built rather than assembled through a builder: a builder hands back a
    /// result whose error arm nothing here can reach.
    fn asking(action: Option<&str>, token: Option<&str>) -> Request {
        let mut request = Request::new(Body::from("{}"));
        let asked =
            action.map_or_else(|| "/".to_owned(), |action| format!("/api/actions/{action}"));
        if action.is_some() {
            *request.method_mut() = axum::http::Method::POST;
        }
        *request.uri_mut() = asked.parse().unwrap_or_default();
        let headers = request.headers_mut();
        headers.insert("host", HeaderValue::from_static("127.0.0.1:8471"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        if let Some(token) = token {
            headers.insert(
                "x-lemonfiber-token",
                HeaderValue::from_str(token).unwrap_or(HeaderValue::from_static("")),
            );
        }
        request
    }

    /// What a request to that surface is answered with.
    async fn met(
        surface: Option<Router>,
        action: Option<&str>,
        token: Option<&str>,
    ) -> Option<u16> {
        answering(surface, asking(action, token)).await
    }

    /// The same, for a request a caller built itself.
    ///
    /// The one place a surface that was never built is answered for, so a test
    /// asking about a path rather than an action does not carry a second arm for
    /// the case only one test reaches.
    async fn answering(surface: Option<Router>, request: Request) -> Option<u16> {
        match surface {
            Some(surface) => tower::ServiceExt::oneshot(surface, request)
                .await
                .ok()
                .map(|response| response.status().as_u16()),
            None => None,
        }
    }

    /// The surface as a run with a working machine builds it.
    fn working() -> Option<Router> {
        as_served(&Chance::exactly(Some(enough())))
    }

    #[tokio::test]
    async fn an_endpoint_reached_without_the_token_is_refused() {
        assert_eq!(
            met(working(), Some("up"), None).await,
            Some(StatusCode::FORBIDDEN.as_u16())
        );
    }

    #[tokio::test]
    async fn an_endpoint_reached_with_the_token_is_admitted_and_then_answered() {
        // Admitted, and then refused on its own terms: there is no such action,
        // which is a different answer from not being let in at all.
        assert_eq!(
            met(working(), Some("reticulate"), Some(&written())).await,
            Some(StatusCode::NOT_FOUND.as_u16())
        );
    }

    #[tokio::test]
    async fn the_page_itself_is_reached_without_a_token() {
        // A browser opening a page sends no header of ours, and it is the page
        // that goes on to ask for one. This build carries no app, so the page
        // says so rather than being refused for want of a token.
        assert_eq!(
            met(working(), None, None).await,
            Some(StatusCode::NOT_FOUND.as_u16())
        );
    }

    #[tokio::test]
    async fn there_is_no_surface_at_all_without_a_token_to_guard_it() {
        // The other answer, and the reason the run refuses rather than serving:
        // a surface whose token could not be minted is one every request reaches.
        let unguarded = as_served(&Chance::exactly(None));
        assert_eq!(met(unguarded, Some("up"), None).await, None);
    }

    /// A path with no route under it is answered by the app, not by the guard.
    ///
    /// Asked of the whole surface rather than of `router::routes`, which is the
    /// difference that matters: the endpoints are merged *under* the app's
    /// fallback, axum keeps one fallback per tree, and so the guarded one is not
    /// the one that answers. Every test beside this one asks for a path that has a
    /// route, where the guard does wrap — which is how the router's own account of
    /// itself stayed wrong through the change that made it wrong.
    ///
    /// So an unauthenticated caller can tell a path that exists from one that does
    /// not. That is written down where it is true now; this holds the surface to
    /// it, and will fail if the composition changes in either direction.
    #[tokio::test]
    async fn a_path_no_route_declares_is_an_absence_rather_than_a_refusal() {
        let mut request = Request::new(Body::empty());
        *request.uri_mut() = "/api/nothing-declares-this".parse().unwrap_or_default();
        request
            .headers_mut()
            .insert("host", HeaderValue::from_static("127.0.0.1:8471"));

        assert_eq!(
            answering(working(), request).await,
            Some(StatusCode::NOT_FOUND.as_u16()),
            "an unmatched path under `/api/` is an absence"
        );
        // And the same run refuses a path that does have a route, so the two are
        // told apart by a caller carrying no token at all.
        assert_eq!(
            met(working(), Some("up"), None).await,
            Some(StatusCode::FORBIDDEN.as_u16()),
            "while a path that has one is refused"
        );
    }

    // ── The password this surface asks for ────────────────────────────────────

    /// A run asked for a password sets one, says so, and goes on to serve.
    #[tokio::test]
    async fn a_run_asked_for_a_password_sets_one_and_then_serves() {
        let dir = a_directory("kept");
        let path = dir.join("admission.json");
        let chosen = a_password();
        let answers = Script::of(&[&chosen, &chosen]);

        let code = run(
            keeping(Some(path.clone())),
            Asked {
                password: true,
                ..Asked::default()
            },
            &answers,
            None,
            Box::pin(std::future::ready(())),
        )
        .await;

        assert_eq!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS)
        );
        assert!(credential::at(&path).is_some_and(|held| held.verifies(&chosen)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every way of not getting a password serves nothing, because a run asked for
    /// one and given none has not been given what it asked for — and serving anyway
    /// would put the surface up under exactly the arrangement the operator was
    /// changing.
    #[tokio::test]
    async fn a_password_that_could_not_be_set_stops_the_run_rather_than_serving() {
        let dir = a_directory("refused");
        let path = dir.join("admission.json");
        let chosen = a_password();
        let short: String = chosen.chars().take(3).collect();
        let asked = Asked {
            password: true,
            ..Asked::default()
        };

        // Nowhere to keep one; the two answers differed; the password is too short;
        // and the file cannot be written because a directory is in its place.
        assert!(std::fs::create_dir_all(dir.join("taken.json")).is_ok());
        let ways: Vec<(Option<PathBuf>, Vec<String>)> = vec![
            (None, vec![chosen.clone(), chosen.clone()]),
            (
                Some(path.clone()),
                vec![chosen.clone(), chosen.to_uppercase()],
            ),
            (Some(path.clone()), vec![short.clone(), short]),
            (
                Some(dir.join("taken.json")),
                vec![chosen.clone(), chosen.clone()],
            ),
        ];
        for (kept, said) in ways {
            let lines: Vec<&str> = said.iter().map(String::as_str).collect();
            let answers = Script::of(&lines);
            let code = run(
                keeping(kept.clone()),
                asked.clone(),
                &answers,
                None,
                Box::pin(std::future::ready(())),
            )
            .await;
            assert_ne!(
                crate::exit::shown(code),
                crate::exit::shown(ExitCode::SUCCESS),
                "{kept:?}"
            );
        }
        assert_eq!(credential::at(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── How far it is offered, and what has to be true first ──────────────────

    /// A password kept where a test can take it away again.
    fn a_password_at(path: &std::path::Path) {
        let held =
            lemonfiber_core::admission::Credential::set(&a_password(), &Chance::cycling()).ok();
        assert!(held
            .as_ref()
            .is_some_and(|held| credential::keep(path, held).is_ok()));
    }

    /// One request over a real connection, and the status it was answered with.
    ///
    /// Written by hand rather than through a client, because what is being proven is
    /// which requests this surface answers and a client would be a second opinion
    /// about what was sent.
    async fn over_tcp(port: u16, host: &str, token: &str) -> Option<u16> {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .ok()?;
        let request = format!(
            "GET /api/explain?word=indexer HTTP/1.1\r\nHost: {host}\r\n\
             X-Lemonfiber-Token: {token}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut said = Vec::new();
        stream.read_to_end(&mut said).await.ok()?;
        String::from_utf8_lossy(&said)
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    }

    /// Asking for the network without a password is refused, not warned about and not
    /// quietly served on this machine instead.
    #[tokio::test]
    async fn the_network_without_a_password_is_refused_rather_than_served_narrower() {
        let dir = a_directory("unpassworded");
        let code = serving(
            keeping(Some(dir.join("admission.json"))),
            Asked {
                reach: Reach::Network,
                ..Asked::default()
            },
            None,
            Box::pin(std::future::ready(())),
            LOOK,
        )
        .await;
        assert_ne!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// And the password taken away while it is on the network gives the network up.
    ///
    /// Proven over a real connection rather than by reading the code back: what
    /// changes is which requests are answered, and the request that tells the two
    /// apart is one naming an address this machine is not — accepted while it is
    /// offered to a network, refused the moment it is not.
    #[tokio::test]
    async fn a_password_taken_away_gives_up_the_network_and_keeps_this_machine() {
        let dir = a_directory("reverted");
        let path = dir.join("admission.json");
        a_password_at(&path);
        let free = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok();
        let port = free
            .as_ref()
            .and_then(|held| held.local_addr().ok())
            .map_or(0, |bound| bound.port());
        assert_ne!(port, 0, "a free port can be taken on this machine");
        drop(free);

        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(serving(
            keeping(Some(path.clone())),
            Asked {
                port: Some(port),
                reach: Reach::Network,
                ..Asked::default()
            },
            None,
            Box::pin(async move {
                let _ = stopped.await;
            }),
            Duration::from_millis(10),
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Offered to a network, a request naming an address this machine answers on
        // is answered — which is the whole of what being offered to a network means.
        let elsewhere = format!("203.0.113.7:{port}");
        let admitted = over_tcp(port, &elsewhere, &written()).await;
        let _ = std::fs::remove_file(&path);
        tokio::time::sleep(Duration::from_millis(300)).await;
        let refused = over_tcp(port, &elsewhere, &written()).await;
        let here = over_tcp(port, &format!("127.0.0.1:{port}"), &written()).await;

        let _ = stop.send(());
        let ended = running.await.map(crate::exit::shown).unwrap_or_default();

        assert_eq!(admitted, Some(200), "a network binding answers an address");
        assert_eq!(refused, Some(403), "and stops the moment the password goes");
        assert_eq!(here, Some(200), "while this machine still reaches it");
        assert_eq!(ended, crate::exit::shown(ExitCode::SUCCESS));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

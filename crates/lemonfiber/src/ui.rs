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

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;

use axum::Router;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::saying::Saying;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::frontend as serving;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::{self, Serving};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State as Standing};
use lemonfiber_core::frontend::Source;
use lemonfiber_core::platform::{HostOs, HOST_OS};
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::PRODUCT;
use tokio::net::TcpListener;

use crate::exit::complain;
use crate::say::say;

/// Raised when the address the surface was asked to serve on cannot be taken.
const ADDRESS_TAKEN: Code = Code::new("SERVE-1");

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
}

/// The address to ask the operating system for.
///
/// Loopback, and only loopback. This surface can start, stop and reconfigure the
/// whole stack and reaches every credential the system holds, so it is the one
/// thing in the product that is never offered to the network by default.
///
/// There is no default port. A port this product chose would be the same port on
/// every machine running it, and a port nobody chose is one something else may
/// already hold — so zero is asked for, which means any free one, and whatever
/// was given is printed in full.
pub(crate) const fn wanted(port: Option<u16>) -> SocketAddr {
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        match port {
            Some(port) => port,
            None => 0,
        },
    )
}

/// Take the socket, and say which one was given.
///
/// The address comes back from the socket rather than from what was asked for,
/// because asking for any free port means not knowing which until it is held.
///
/// # Errors
///
/// Returns the [`Problem`] to report when the address cannot be taken. Boxed
/// because a problem carries what happened, what it means and what to do about
/// it, and a result that carries all of that inline on the way that succeeds is
/// paying for the failure on every call.
pub(crate) async fn taken(wanted: SocketAddr) -> Result<(TcpListener, SocketAddr), Box<Problem>> {
    // Bound and named in one step, so there is one way for this to fail rather
    // than two, only one of which anything could provoke — and so the way it
    // cannot fail leaves behind no arm a test would have to reach.
    let bound = TcpListener::bind(wanted).await;
    let held = bound.and_then(|listener| listener.local_addr().map(|bound| (listener, bound)));
    match held {
        Ok(held) => Ok(held),
        Err(err) => Err(Box::new(unavailable(wanted, &err.to_string()))),
    }
}

/// The address as it is printed, and as it is typed into a browser.
pub(crate) fn address(bound: SocketAddr) -> String {
    format!("http://{bound}")
}

/// What asking this desktop to open a browser came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Browser {
    /// One opened.
    Opened,
    /// None opened, which is not this command's failure.
    Unopened,
    /// None was asked for.
    Unasked,
}

/// The program this desktop opens an address with.
///
/// Which of the three it is comes from the one module allowed to know what this
/// machine is, rather than being asked here.
pub(crate) fn opener(host: HostOs, url: &str) -> Vec<String> {
    let argv: &[&str] = match host {
        HostOs::MacOs => &["open"],
        // An empty first argument, which `start` reads as the window title it is
        // not being given. Without it the address becomes the title and nothing
        // opens.
        HostOs::Windows => &["cmd", "/c", "start", ""],
        HostOs::Linux | HostOs::Other => &["xdg-open"],
    };
    argv.iter()
        .map(|word| (*word).to_owned())
        .chain(std::iter::once(url.to_owned()))
        .collect()
}

/// Ask this desktop to open the address, and say what came of it.
///
/// Every way of not opening one is the same answer. A desktop with no browser, a
/// machine with no desktop and a program that exited badly all leave the operator
/// with an address to open themselves, and none of them is a reason for the
/// command to have failed.
pub(crate) async fn opening(runner: &dyn Runner, host: HostOs, url: &str) -> Browser {
    let ran = runner.run(&opener(host, url)).await;
    if ran.is_ok_and(|output| output.succeeded()) {
        Browser::Opened
    } else {
        Browser::Unopened
    }
}

/// What starting the surface says, in order.
///
/// The transport is stated as a sentence rather than left to the scheme in the
/// address. `http` in front of a name is a fact an operator has no reason to be
/// able to read, and what it costs them is the thing worth saying out loud.
pub(crate) fn announcement(bound: SocketAddr, token: &str, browser: Browser) -> Vec<String> {
    let mut lines = vec![
        format!("{PRODUCT} is serving at {}", address(bound)),
        String::new(),
        "This connection is not encrypted. Anything else running on this machine can read \
         what passes over it."
            .to_owned(),
        "Nothing on your network can reach it — it listens on this machine and nowhere else."
            .to_owned(),
        String::new(),
        "The token for this run, which the page will ask you for:".to_owned(),
        format!("  {token}"),
        "It is kept in memory, written down nowhere, and gone when this stops.".to_owned(),
        String::new(),
    ];
    lines.push(
        match browser {
            Browser::Opened => "A browser has been opened at that address.",
            Browser::Unopened => {
                "A browser could not be opened here. Open the address above yourself."
            }
            Browser::Unasked => "Open the address above in a browser.",
        }
        .to_owned(),
    );
    lines.push(format!("Stop {PRODUCT} with Ctrl-C when you are finished."));
    lines
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
    embedded: Option<Source>,
    until: Until,
) -> ExitCode {
    let (listener, bound) = match taken(wanted(asked.port)).await {
        Ok(held) => held,
        Err(problem) => return complain(&problem),
    };
    let Some(token) = Token::mint(ctx.random.as_ref()) else {
        return complain(&tokenless());
    };
    let browser = if asked.browser {
        opening(ctx.runner.as_ref(), HOST_OS, &address(bound)).await
    } else {
        Browser::Unasked
    };
    for line in announcement(bound, token.as_str(), browser) {
        say!("{line}");
    }

    // The one gather every listener hears, made before the context so that the
    // waits a command runs into have somewhere to say what they are waiting for:
    // a browser is told the name of the work and nothing else, and everything it
    // learns after that arrives here.
    let live = Arc::new(Live::opening(ctx.clock.as_ref()));
    let ctx = ctx.narrating(Arc::new(Saying::onto(Arc::clone(&live))));
    let (ctx, token) = (Arc::new(ctx), Arc::new(token));
    // Started before anything can ask to hear it, so a client that connects at
    // once is not waiting on a first pass that has not been asked for.
    tokio::spawn(Arc::clone(&live).gathering(Arc::new(
        lemonfiber_api::events::dashboard::Dashboard::against(Arc::clone(&ctx)),
    )));
    let serving = Serving {
        ctx: Arc::clone(&ctx),
        token: Arc::clone(&token),
        bound,
        jobs: Jobs::default(),
    };
    let streaming = Arc::new(Streaming { token, bound, live });
    let surface = surface(serving, streaming, app(embedded, asked.assets));
    // Whatever ends the loop, the surface has stopped, and that is the whole of
    // what there is to report. A fault from accepting on a socket this process
    // already holds means the process is going down around it, and a second
    // message about one event helps nobody.
    let _ = axum::serve(listener, surface)
        .with_graceful_shutdown(until)
        .await;
    ExitCode::SUCCESS
}

/// The address could not be taken.
fn unavailable(wanted: SocketAddr, reason: &str) -> Problem {
    let asked = if wanted.port() == 0 {
        "no free port could be taken on this machine".to_owned()
    } else {
        format!("{wanted} could not be taken")
    };
    Problem::new(
        ADDRESS_TAKEN,
        Severity::Error,
        format!("{PRODUCT} could not start serving: {asked}"),
        "Usually something else on this machine is already listening there. Whatever the \
         reason, there is nowhere for a browser to connect, and the words below are the \
         operating system's own.",
        Remedy::new("Ask for a different port").with_detail(format!("{PRODUCT} ui --port 7171")),
    )
    .or_try(Remedy::new(
        "Or name no port and be given whichever one is free",
    ))
    .in_state(Standing::Guided)
    .with_detail(reason.to_owned())
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
    use std::net::SocketAddr;
    use std::process::ExitCode;
    use std::sync::Arc;

    use async_trait::async_trait;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{HeaderValue, StatusCode};
    use axum::Router;
    use lemonfiber_api::events::live::Live;
    use lemonfiber_api::events::Streaming;
    use lemonfiber_api::guard::Token;
    use lemonfiber_api::jobs::Jobs;
    use lemonfiber_api::router::Serving;
    use lemonfiber_core::app::Ctx;
    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::{Environment, HostOs};
    use lemonfiber_core::ports::process::{Failure, Output, Runner};
    use lemonfiber_fixtures::ports::{Chance, Idle};

    use super::{
        address, announcement, app, opener, opening, run, surface, taken, tokenless, unavailable,
        wanted, Asked, Browser,
    };

    /// A run of a program that went the way the test chose.
    struct Ran(Result<Output, Failure>);

    #[async_trait]
    impl Runner for Ran {
        async fn run(&self, _: &[String]) -> Result<Output, Failure> {
            match &self.0 {
                Ok(output) => Ok(output.clone()),
                Err(_) => Err(Failure::NotFound {
                    program: "xdg-open".to_owned(),
                }),
            }
        }
    }

    /// A program that ran and exited with this status.
    fn exited(status: i32) -> Ran {
        Ran(Ok(Output {
            status: Some(status),
            stdout: String::new(),
            stderr: String::new(),
        }))
    }

    /// A program that is not installed.
    fn missing() -> Ran {
        Ran(Err(Failure::NotFound {
            program: "xdg-open".to_owned(),
        }))
    }

    /// An address of numbers, which cannot fail to be one.
    fn bound() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 8471))
    }

    /// Everything a starting surface says, as one block of text.
    fn said(browser: Browser) -> String {
        announcement(bound(), "000fa5ff", browser).join("\n")
    }

    /// The word the claim about the transport turns on. A rewording that drops it
    /// has changed the claim rather than the wording.
    const ENCRYPTION: &str = "encrypt";

    /// How a sentence says there is none of something.
    ///
    /// The closed set English denies with, rather than a list of ways to phrase this
    /// particular sentence: a reword is free to say it however it likes, so long as
    /// it still says *not*.
    const DENIAL: &[&str] = &[
        "not",
        "no",
        "none",
        "nothing",
        "never",
        "without",
        "unencrypted",
    ];

    /// What an unencrypted connection lets somebody do.
    const READING: &[&str] = &["read", "see"];

    /// Who it lets do it, which is somebody who is not the operator.
    const SOMEBODY_ELSE: &[&str] = &["else", "other"];

    /// What a starting surface says about the connection, as sentences, with the
    /// address taken out of them.
    ///
    /// The address goes first because `http` in front of it is not this product
    /// saying anything. A guard that read the scheme would pass a run that had
    /// deleted every word about the transport and left the address to speak for
    /// itself, which is the failure this is here for.
    fn about_the_connection(browser: Browser) -> Vec<String> {
        said(browser)
            .replace(&address(bound()), " ")
            .to_lowercase()
            .split(['.', ';', '\n', '\u{2014}'])
            .map(|sentence| sentence.trim().to_owned())
            .filter(|sentence| !sentence.is_empty())
            .collect()
    }

    /// Whether a sentence says there is none of what it is about.
    fn denies(sentence: &str) -> bool {
        sentence
            .split_whitespace()
            .map(|word| word.trim_matches(|mark: char| !mark.is_alphanumeric()))
            .any(|word| DENIAL.contains(&word))
    }

    #[test]
    fn naming_no_port_asks_for_whichever_one_is_free() {
        assert_eq!(wanted(None).port(), 0);
    }

    #[test]
    fn naming_a_port_asks_for_that_one() {
        assert_eq!(wanted(Some(7171)).port(), 7171);
    }

    #[test]
    fn whichever_port_it_is_the_address_is_this_machine() {
        for port in [None, Some(7171)] {
            assert!(wanted(port).ip().is_loopback(), "{port:?}");
        }
    }

    #[tokio::test]
    async fn a_free_port_is_taken_and_named_in_full() {
        // Asserted as one value rather than through a branch on the way in: this
        // module's tests are under the same coverage gate as the code, and an arm
        // for a bind that never fails is a line nothing could ever run.
        let held = taken(wanted(None)).await.ok();
        assert_eq!(
            held.map(|(_, bound)| (bound.ip().is_loopback(), bound.port() != 0)),
            Some((true, true)),
            "a free port on this machine, named in full"
        );
    }

    #[tokio::test]
    async fn an_address_that_cannot_be_taken_is_reported_rather_than_swapped() {
        // Reserved for documentation and never assigned to an interface, so
        // asking for it fails the same way on every machine.
        let elsewhere = SocketAddr::from(([192, 0, 2, 1], 8471));
        let refusal = taken(elsewhere).await.err().map(|problem| problem.summary);
        assert_eq!(
            refusal
                .as_deref()
                .map(|said| said.contains("could not be taken")),
            Some(true),
            "got: {refusal:?}"
        );
    }

    #[test]
    fn a_machine_with_no_free_port_at_all_says_that_instead() {
        // The other half of the same fault: asking for any port and being given
        // none says something different from being refused a named one.
        let any = unavailable(wanted(None), "denied").summary;
        let named = unavailable(wanted(Some(7171)), "denied").summary;
        assert!(any.contains("no free port"), "{any}");
        assert!(named.contains("127.0.0.1:7171"), "{named}");
    }

    #[test]
    fn a_refusal_to_take_an_address_offers_both_ways_out() {
        // Ask for another port, or stop asking for one in particular.
        let problem = unavailable(wanted(Some(7171)), "address in use");
        assert_eq!(problem.remedies.len(), 2);
        assert_eq!(problem.detail.as_deref(), Some("address in use"));
    }

    #[test]
    fn the_address_is_printed_whole() {
        assert_eq!(address(bound()), "http://127.0.0.1:8471");
    }

    #[test]
    fn each_desktop_is_opened_the_way_that_desktop_opens_things() {
        assert_eq!(
            opener(HostOs::MacOs, "http://127.0.0.1:8471"),
            vec!["open".to_owned(), "http://127.0.0.1:8471".to_owned()]
        );
        assert_eq!(
            opener(HostOs::Linux, "http://127.0.0.1:8471")
                .first()
                .map(String::as_str),
            Some("xdg-open")
        );
        assert_eq!(
            opener(HostOs::Other, "http://127.0.0.1:8471")
                .first()
                .map(String::as_str),
            Some("xdg-open")
        );
        let windows = opener(HostOs::Windows, "http://127.0.0.1:8471");
        assert_eq!(
            windows.len(),
            5,
            "the title it is not being given: {windows:?}"
        );
        assert_eq!(
            windows.last().map(String::as_str),
            Some("http://127.0.0.1:8471")
        );
    }

    #[tokio::test]
    async fn a_browser_that_opens_is_reported_as_opened() {
        assert_eq!(
            opening(&exited(0), HostOs::MacOs, "http://127.0.0.1:8471").await,
            Browser::Opened
        );
    }

    #[test]
    fn a_browser_that_will_not_open_leaves_the_address_to_open_by_hand() {
        let said = said(Browser::Unopened);
        assert!(said.contains("could not be opened"), "{said}");
        assert!(said.contains("Open the address above yourself"), "{said}");
        assert!(said.contains("http://127.0.0.1:8471"), "{said}");
    }

    #[tokio::test]
    async fn whatever_the_browser_did_the_address_is_the_first_thing_said() {
        // The outcomes come from runners rather than being named, so these are the
        // three a run reaches: one that opened, one that would not, and one that was
        // never asked for. Line 0, because an operator whose browser did not open has
        // to find the address, and one printed below an apology is one they scroll for.
        let url = address(bound());
        let reached = [
            Browser::Unasked,
            opening(&exited(0), HostOs::Linux, &url).await,
            opening(&exited(1), HostOs::Linux, &url).await,
            opening(&missing(), HostOs::Linux, &url).await,
        ];
        for outcome in [Browser::Opened, Browser::Unopened, Browser::Unasked] {
            assert!(
                reached.contains(&outcome),
                "{outcome:?} is not among {reached:?}, so this proves less than it reads as"
            );
        }
        for browser in reached {
            assert_eq!(
                announcement(bound(), "000fa5ff", browser)
                    .first()
                    .map(|line| line.contains(&url)),
                Some(true),
                "{browser:?}"
            );
        }
    }

    #[test]
    fn the_transport_is_stated_in_words_rather_than_left_to_the_scheme() {
        // Every outcome, because what is said about the browser is the only part of
        // this that changes and the transport is not one of the things it changes.
        for browser in [Browser::Opened, Browser::Unopened, Browser::Unasked] {
            let about = about_the_connection(browser);
            let mentioned: Vec<&String> = about
                .iter()
                .filter(|sentence| sentence.contains(ENCRYPTION))
                .collect();
            assert!(
                !mentioned.is_empty(),
                "{browser:?} leaves the transport to the scheme: {about:?}"
            );
            assert!(
                mentioned.iter().all(|sentence| denies(sentence)),
                "{browser:?} says the connection is protected: {mentioned:?}"
            );
        }
    }

    #[test]
    fn what_being_unencrypted_costs_is_said_as_well_as_that_it_is() {
        // A fact about a protocol is not a warning. What makes it one is who it lets
        // in, and an operator told only the fact has been told nothing they can act on.
        let about = about_the_connection(Browser::Unasked);
        assert!(
            about.iter().any(|sentence| {
                READING.iter().any(|verb| sentence.contains(verb))
                    && SOMEBODY_ELSE.iter().any(|who| sentence.contains(who))
            }),
            "nothing here says what being unencrypted lets anybody do: {about:?}"
        );
    }

    #[tokio::test]
    async fn the_words_are_about_the_connection_a_run_actually_takes() {
        // The address is one really taken, through the same call a run makes, so the
        // claim is held against the connection rather than against a number written
        // down beside it. A surface that one day serves over TLS prints a different
        // scheme here, and the sentence above it has to change with it.
        let held = taken(wanted(None)).await.ok();
        let checked = held.map(|(_listener, bound)| {
            let said = announcement(bound, "000fa5ff", Browser::Unasked).join("\n");
            (
                address(bound).starts_with("http://"),
                bound.ip().is_loopback(),
                said.contains(&address(bound)),
            )
        });
        assert_eq!(
            checked,
            Some((true, true, true)),
            "unencrypted, reachable from nowhere else, and printed in full — the three \
             things these words claim"
        );
    }

    #[test]
    fn it_says_it_is_reachable_from_nowhere_else() {
        assert!(said(Browser::Unasked).contains("Nothing on your network can reach it"));
    }

    #[test]
    fn the_token_is_printed_and_said_to_be_the_only_copy() {
        let said = said(Browser::Opened);
        assert!(said.contains("000fa5ff"), "the token itself: {said}");
        assert!(said.contains("written down nowhere"), "{said}");
    }

    #[test]
    fn it_says_how_to_stop() {
        // It holds the terminal until it is stopped, so how to stop it is part
        // of what starting it has to say.
        assert!(said(Browser::Opened).contains("Ctrl-C"));
    }

    #[test]
    fn a_browser_that_opened_says_so_rather_than_asking_twice() {
        let said = said(Browser::Opened);
        assert!(said.contains("has been opened"), "{said}");
        assert!(!said.contains("could not be opened"), "{said}");
    }

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
    fn running(runner: Arc<dyn Runner>, bytes: Option<Vec<u8>>) -> Ctx {
        Ctx::new(
            runner,
            Arc::new(lemonfiber_core::adapters::Daemon::local()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            lemonfiber_core::stack::Source::Embedded(&lemonfiber::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
        .with_random(Arc::new(Chance::exactly(bytes)))
    }

    /// The same, over a runner that spawns nothing.
    fn ctx(bytes: Option<Vec<u8>>) -> Ctx {
        running(Arc::new(Idle), bytes)
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
        let serving = tokio::spawn(run(
            ctx,
            asked,
            None,
            Box::pin(async move {
                let _ = stopped.await;
            }),
        ));
        // Let the loop reach the socket before it is asked to leave it, so this
        // proves a surface that started rather than one that never did.
        tokio::task::yield_now().await;
        let _ = stop.send(());
        serving.await.map(crate::exit::shown).unwrap_or_default()
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
                started(running(Arc::new(runner), Some(enough())), asked.clone()).await,
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
        let held = taken(wanted(None)).await.ok();
        let asked = Asked {
            port: held.as_ref().map(|(_, bound)| bound.port()),
            ..Asked::default()
        };
        let code = run(
            ctx(Some(enough())),
            asked,
            None,
            Box::pin(std::future::ready(())),
        )
        .await;
        assert_ne!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS)
        );
        drop(held);
    }

    #[tokio::test]
    async fn a_machine_that_will_not_supply_randomness_serves_nothing() {
        // A surface whose token could not be minted would be one every request
        // reached, so there is nothing here to fall back to.
        let code = run(
            ctx(None),
            Asked::default(),
            None,
            Box::pin(std::future::ready(())),
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
        let serving = Serving {
            ctx: Arc::new(ctx(Some(enough()))),
            token: Arc::clone(&token),
            bound: bound(),
            jobs: Jobs::default(),
        };
        let streaming = Arc::new(Streaming {
            token,
            bound: bound(),
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
        match surface {
            Some(surface) => tower::ServiceExt::oneshot(surface, asking(action, token))
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
}

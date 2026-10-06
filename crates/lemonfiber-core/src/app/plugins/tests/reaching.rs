//! Where a git source is fetched from: over https, from a host out on the internet,
//! held to the addresses that were checked.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_fixtures::ports::Resolving;

use super::fetching::{from_git, served, Serving};
use super::*;
use crate::ports::process::{Failure, Output, Runner};

/// The commit the source's default branch points at.
const HEAD: &str = "8fa05ba718f70624f2c122f8c0371d47e6c90d0e";

/// A git source, with every command it was run with kept whole, settings included.
struct Seen {
    /// The source that answers.
    serving: Arc<Serving>,
    /// Every command, as it was run.
    ran: Mutex<Vec<Vec<String>>>,
}

impl Seen {
    /// Seeing every command `serving` answers.
    fn of(serving: Arc<Serving>) -> Arc<Self> {
        Arc::new(Self {
            serving,
            ran: Mutex::new(Vec::new()),
        })
    }

    /// Every command run so far, whole.
    fn ran(&self) -> Vec<Vec<String>> {
        self.ran.lock().map(|ran| ran.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Runner for Seen {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if let Ok(mut ran) = self.ran.lock() {
            ran.push(argv.to_vec());
        }
        self.serving.run(argv).await
    }

    async fn run_with(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        if let Ok(mut ran) = self.ran.lock() {
            ran.push(argv.to_vec());
        }
        self.serving.run_with(argv, env).await
    }
}

/// A context over a git source that serves `HEAD`, resolving every name as `resolving`
/// says, with every command seen whole.
fn reaching(name: &str, resolving: &Arc<Resolving>) -> (Ctx, Arc<Seen>, Arc<Serving>) {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let seen = Seen::of(serving.clone());
    let mut ctx = served(name, &serving).with_resolver(resolving.clone());
    ctx.seams.runner = seen.clone();
    (ctx, seen, serving)
}

/// A source written with any scheme but https is refused naming it, before a setting is
/// read, a name is resolved or git is run.
#[tokio::test]
async fn a_source_not_fetched_over_https_is_refused_naming_its_scheme() {
    let resolving = Resolving::anywhere();
    let (mut ctx, seen, _) = reaching("reach-scheme", &resolving);
    ctx.settings.reaching =
        crate::config::Reaching::without(crate::config::REACH_PLUGIN_SOURCE_KEY);

    for (written, scheme) in [
        ("http://example.org/plugin-komga", "http://"),
        ("ssh://git@example.org/plugin-komga", "ssh://"),
        ("git://example.org/plugin-komga", "git://"),
        ("git@example.org:plugin-komga", "git@"),
    ] {
        let refused = from_git(&ctx, written).await.err();
        assert_eq!(
            refused.as_ref().map(|one| one.code.to_string()).as_deref(),
            Some("PLUGIN-31"),
            "{written}"
        );
        assert!(
            refused.is_some_and(|one| one.meaning.contains(scheme)),
            "{written} names {scheme}"
        );
    }
    assert!(seen.ran().is_empty(), "{:?}", seen.ran());
    assert!(resolving.asked().is_empty());
}

/// A host standing for this machine or a network of its own is refused apart from a
/// source that did not answer, and git is never run.
#[tokio::test]
async fn a_source_standing_for_an_address_here_is_refused_and_git_is_not_run() {
    let resolving = Resolving::standing_for(&[std::net::IpAddr::from([169, 254, 169, 254])]);
    let (ctx, seen, _) = reaching("reach-here", &resolving);

    let refused = from_git(&ctx, "https://metadata.example/plugin-komga").await;

    assert_eq!(refusal(refused), "PLUGIN-32");
    assert!(seen.ran().is_empty(), "{:?}", seen.ran());
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// Every command an install runs against the source is held to the addresses the host
/// was checked for, and the name is resolved once for the reading and once for the yes.
#[tokio::test]
async fn every_command_an_install_runs_is_held_to_the_addresses_checked() {
    let resolving = Resolving::anywhere();
    let (ctx, seen, serving) = reaching("reach-pinned", &resolving);

    let installed = from_git(&ctx, "https://example.org/plugin-komga").await;

    assert_eq!(counted(installed), Some(1));
    let pin = format!(
        "http.curloptResolve=example.org:443:{}",
        lemonfiber_fixtures::ports::ANYWHERE
    );
    let ran: Vec<Vec<String>> = seen
        .ran()
        .into_iter()
        .filter(|argv| argv.first().map(String::as_str) == Some("git"))
        .collect();
    assert!(
        !ran.is_empty()
            && ran.iter().all(|argv| argv.contains(&pin)
                && argv.contains(&"http.followRedirects=false".to_owned())),
        "{ran:?}"
    );
    assert!(serving
        .asked()
        .iter()
        .any(|one| one.first().map(String::as_str) == Some("ls-remote")));
    assert_eq!(
        resolving.asked(),
        [
            ("example.org".to_owned(), 443),
            ("example.org".to_owned(), 443)
        ]
    );
}

/// Listing what is installed asks each source whether it answers and resolves nothing
/// itself, so a listing reaches no further than it did.
#[tokio::test]
async fn listing_what_is_installed_resolves_nothing() {
    let resolving = Resolving::anywhere();
    let (ctx, _, _) = reaching("reach-listed", &resolving);
    assert_eq!(
        counted(from_git(&ctx, "https://example.org/plugin-komga").await),
        Some(1)
    );
    let before = resolving.asked().len();

    let listed = plugins(&ctx, &Asked::Installed).await;

    assert_eq!(counted(listed), Some(1));
    assert_eq!(resolving.asked().len(), before);
}

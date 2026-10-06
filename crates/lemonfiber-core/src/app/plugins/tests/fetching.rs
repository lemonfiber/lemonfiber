//! Installing from a git source: resolved to one commit, fetched as data, recorded.

use std::sync::Mutex;

use async_trait::async_trait;

use super::*;
use crate::plugin::Source;
use crate::ports::process::{Failure, Output, Runner};

/// The commit the source's default branch points at.
const HEAD: &str = "8fa05ba718f70624f2c122f8c0371d47e6c90d0e";

/// The commit a tag's own object has, and the one it points at.
const TAG_OBJECT: &str = "1111111111111111111111111111111111111111";
const TAGGED: &str = "2222222222222222222222222222222222222222";

/// A git source that serves `MANIFEST`, and everything else answered the way the
/// default runner answers it.
pub(super) struct Serving {
    /// What `git ls-remote` answers, or how it fails.
    listed: Result<String, String>,
    /// How the fetch fails, where it does.
    fetch: Option<String>,
    /// Every git command it was asked, without the leading `git -c …`.
    asked: Mutex<Vec<Vec<String>>>,
    /// The environment each was run with.
    environments: Mutex<Vec<Vec<(String, String)>>>,
}

impl Serving {
    pub(super) fn listing(listed: &str) -> Self {
        Self {
            listed: Ok(listed.to_owned()),
            fetch: None,
            asked: Mutex::new(Vec::new()),
            environments: Mutex::new(Vec::new()),
        }
    }

    /// The environment each command was run with.
    fn environments(&self) -> Vec<Vec<(String, String)>> {
        self.environments
            .lock()
            .map(|environments| environments.clone())
            .unwrap_or_default()
    }

    pub(super) fn asked(&self) -> Vec<Vec<String>> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
    /// Every directory a commit was checked out into, in the order they were made.
    pub(super) fn checkouts(&self) -> Vec<std::path::PathBuf> {
        self.asked()
            .iter()
            .filter_map(|one| match one.as_slice() {
                [init, quiet, at] if init == "init" && quiet == "--quiet" => Some(at.into()),
                _ => None,
            })
            .collect()
    }

    /// Whether a commit was checked out, and every directory it was checked out into is
    /// gone.
    pub(super) fn left_nothing(&self) -> bool {
        let made = self.checkouts();
        !made.is_empty() && made.iter().all(|at| !at.exists())
    }
}

#[async_trait]
impl Runner for Serving {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if argv.first().map(String::as_str) != Some("git") {
            return Ok(spoke(""));
        }
        let rest = asked_of(argv);
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(rest.clone());
        }
        let words: Vec<&str> = rest.iter().map(String::as_str).collect();
        Ok(match words.as_slice() {
            ["ls-remote", ..] => match &self.listed {
                Ok(listed) => spoke(listed),
                Err(why) => engine_refused(why),
            },
            ["init", "--quiet", at] => {
                let _ = std::fs::create_dir_all(at);
                spoke("")
            }
            ["-C", _, "fetch", ..] => match &self.fetch {
                Some(why) => engine_refused(why),
                None => spoke(""),
            },
            ["-C", at, "checkout", ..] => {
                let _ = std::fs::write(Path::new(at).join("plugin.toml"), MANIFEST);
                spoke("")
            }
            _ => spoke(""),
        })
    }

    async fn run_with(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        if let Ok(mut environments) = self.environments.lock() {
            environments.push(env.to_vec());
        }
        self.run(argv).await
    }
}

/// What git was asked, without `git` and the `-c` pairs every command carries.
pub(super) fn asked_of(argv: &[String]) -> Vec<String> {
    let mut rest = argv.get(1..).unwrap_or_default();
    while let [flag, _, after @ ..] = rest {
        if flag != "-c" {
            break;
        }
        rest = after;
    }
    rest.to_vec()
}

/// A context whose runner is `serving`.
pub(super) fn served(name: &str, serving: &Arc<Serving>) -> Ctx {
    let mut ctx = ctx(name);
    ctx.seams.runner = serving.clone();
    ctx
}

/// Install from `written`, as the operator would write it.
pub(super) async fn from_git(
    ctx: &Ctx,
    written: &str,
) -> Result<Installs, Box<crate::error::Problem>> {
    super::answered(
        ctx,
        Asked::Install {
            source: Source::named(written),
            consent: crate::app::plugins::Consent::default(),
        },
    )
    .await
}

/// A source named with no revision is installed at the commit it serves by default,
/// and the record keeps the source and that commit.
#[tokio::test]
async fn a_git_source_is_installed_at_the_commit_it_serves_and_recorded() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let ctx = served("git-head", &serving);

    let installed = report(from_git(&ctx, "https://example.org/plugin-komga").await)
        .and_then(|one| one.installed.first().cloned());

    assert_eq!(
        installed.map(|one| (one.from, one.revision)),
        Some((
            "https://example.org/plugin-komga".to_owned(),
            HEAD.to_owned()
        ))
    );
    let asked = serving.asked();
    assert!(
        asked
            .iter()
            .any(|one| one.first().map(String::as_str) == Some("ls-remote")
                && one.last().map(String::as_str) == Some("HEAD")),
        "{asked:?}"
    );
    assert!(
        asked.iter().any(|one| one.contains(&"fetch".to_owned())
            && one.contains(&"--depth".to_owned())
            && one.last().map(String::as_str) == Some(HEAD)),
        "the one commit was not what was fetched: {asked:?}"
    );
    let environments = serving.environments();
    assert!(
        environments.len() == asked.len()
            && environments
                .iter()
                .all(|one| *one == super::super::git::environment()),
        "a git command ran under this machine's own configuration: {environments:?}"
    );
    assert!(serving.left_nothing(), "the checkout was left behind");
}

/// A tag is installed at the commit it points at, not at the tag's own object.
#[tokio::test]
async fn a_tag_is_installed_at_the_commit_it_points_at() {
    let serving = Arc::new(Serving::listing(&format!(
        "{TAG_OBJECT}\trefs/tags/v1.2.0\n{TAGGED}\trefs/tags/v1.2.0^{{}}\n"
    )));
    let ctx = served("git-tag", &serving);

    let installed = report(from_git(&ctx, "https://example.org/plugin-komga@v1.2.0").await)
        .and_then(|one| one.installed.first().cloned());

    assert_eq!(installed.map(|one| one.revision), Some(TAGGED.to_owned()));
}

/// A whole commit needs no asking: nothing is listed, and that commit is fetched.
#[tokio::test]
async fn a_whole_commit_is_fetched_without_asking() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = served("git-commit", &serving);

    let installed = report(
        from_git(
            &ctx,
            &format!("https://example.org/plugin-komga@{}", HEAD.to_uppercase()),
        )
        .await,
    )
    .and_then(|one| one.installed.first().cloned());

    assert_eq!(installed.map(|one| one.revision), Some(HEAD.to_owned()));
    assert!(!serving
        .asked()
        .iter()
        .any(|one| one.first().map(String::as_str) == Some("ls-remote")));
}

/// A revision the source does not hold is refused before anything is fetched.
#[tokio::test]
async fn a_revision_the_source_does_not_hold_is_refused() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = served("git-no-revision", &serving);

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga@nowhere").await),
        "PLUGIN-17"
    );
    assert!(!serving
        .asked()
        .iter()
        .any(|one| one.contains(&"fetch".to_owned())));
}

/// A source that will not answer is refused, and says what git said.
#[tokio::test]
async fn a_source_that_will_not_answer_is_refused_with_what_git_said() {
    let serving = Arc::new(Serving {
        listed: Err("fatal: repository not found".to_owned()),
        fetch: None,
        asked: Mutex::new(Vec::new()),
        environments: Mutex::new(Vec::new()),
    });
    let ctx = served("git-unreachable", &serving);

    let refused = from_git(&ctx, "https://example.org/nothing").await.err();

    assert_eq!(
        refused.as_ref().map(|one| one.code.to_string()),
        Some("PLUGIN-16".to_owned())
    );
    assert!(refused
        .as_ref()
        .is_some_and(|one| format!("{one:?}").contains("repository not found")));
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A commit that will not be handed over is refused, and nothing is left behind.
#[tokio::test]
async fn a_commit_that_will_not_be_fetched_is_refused_and_leaves_nothing() {
    let serving = Arc::new(Serving {
        listed: Ok(format!("{HEAD}\tHEAD\n")),
        fetch: Some("fatal: couldn't find remote ref".to_owned()),
        asked: Mutex::new(Vec::new()),
        environments: Mutex::new(Vec::new()),
    });
    let ctx = served("git-unfetched", &serving);

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga").await),
        "PLUGIN-16"
    );
    assert!(serving.left_nothing());
}

/// A git that cannot be run at all is a source that could not be fetched.
#[tokio::test]
async fn a_git_that_cannot_run_is_a_source_that_could_not_be_fetched() {
    let mut ctx = ctx("git-missing");
    ctx.seams.runner = Arc::new(lemonfiber_fixtures::support::Scripted(Err(
        Failure::NotFound {
            program: "git".to_owned(),
        },
    )));

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga").await),
        "PLUGIN-16"
    );
}

/// Fetching from a git source switched off refuses before anything is asked.
#[tokio::test]
async fn a_git_source_switched_off_is_refused_before_anything_is_asked() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = served("git-off", &serving);
    ctx.settings.reaching =
        crate::config::Reaching::without(crate::config::REACH_PLUGIN_SOURCE_KEY);

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga").await),
        "PLUGIN-15"
    );
    assert!(serving.asked().is_empty());
}

/// A rehearsal of a git install fetches to say what it would do, and leaves nothing.
#[tokio::test]
async fn a_rehearsed_git_install_records_nothing_and_leaves_nothing() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = served("git-rehearsed", &serving);
    ctx.dry_run = true;

    let install = report(from_git(&ctx, "https://example.org/plugin-komga").await)
        .and_then(|one| one.install);

    assert_eq!(
        install
            .as_ref()
            .map(|one| (one.recorded, one.would.revision.clone())),
        Some((false, HEAD.to_owned()))
    );
    assert!(!record_of(&ctx).exists());
    assert!(serving.left_nothing());
}

/// A listing with lines that are not a commit and a name is read past them.
#[test]
fn a_listing_is_read_past_what_is_not_a_commit() {
    assert_eq!(
        super::super::fetching::listed_commit(
            &format!("warning: nothing\n{HEAD}\trefs/heads/main\n"),
            "main"
        ),
        Some(HEAD.to_owned())
    );
    assert_eq!(super::super::fetching::listed_commit("", "main"), None);
}

/// What `plugin installed` said about each source.
async fn standings(ctx: &Ctx) -> Vec<crate::plugin::Fetchable> {
    report(reading(ctx).await)
        .map(|one| {
            one.sources
                .into_iter()
                .map(|source| source.standing)
                .collect()
        })
        .unwrap_or_default()
}

/// A git source that answers can be updated from; one that stopped answering cannot,
/// and the listing says what asking it said.
#[tokio::test]
async fn a_git_source_is_asked_whether_it_still_answers_when_plugins_are_listed() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = served("git-listed", &serving);
    assert_eq!(
        counted(from_git(&ctx, "https://example.org/plugin-komga").await),
        Some(1)
    );

    assert_eq!(
        standings(&ctx).await,
        vec![crate::plugin::Fetchable::Reachable]
    );

    let gone = Arc::new(Serving {
        listed: Err("fatal: repository not found".to_owned()),
        fetch: None,
        asked: Mutex::new(Vec::new()),
        environments: Mutex::new(Vec::new()),
    });
    ctx.seams.runner = gone.clone();
    assert_eq!(
        standings(&ctx).await,
        vec![crate::plugin::Fetchable::Unreachable {
            why: "fatal: repository not found".to_owned()
        }]
    );
    assert!(gone
        .asked()
        .iter()
        .any(|one| one.first().map(String::as_str) == Some("ls-remote")));
}

/// With fetching from a git source switched off, nothing is asked and the listing
/// says so rather than calling the source unreachable.
#[tokio::test]
async fn a_git_source_switched_off_is_not_asked_when_plugins_are_listed() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = served("git-listed-off", &serving);
    assert_eq!(
        counted(from_git(&ctx, "https://example.org/plugin-komga").await),
        Some(1)
    );
    ctx.settings.reaching =
        crate::config::Reaching::without(crate::config::REACH_PLUGIN_SOURCE_KEY);
    let quiet = Arc::new(Serving::listing(""));
    ctx.seams.runner = quiet.clone();

    assert!(matches!(
        standings(&ctx).await.as_slice(),
        [crate::plugin::Fetchable::Unasked { .. }]
    ));
    assert!(quiet.asked().is_empty());
}

/// A directory still there can be updated from; one that has gone cannot.
#[tokio::test]
async fn a_directory_that_has_gone_is_a_source_that_cannot_be_fetched() {
    let ctx = ctx("dir-listed");
    let at = source("dir-listed", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(1));
    assert_eq!(
        standings(&ctx).await,
        vec![crate::plugin::Fetchable::Reachable]
    );

    let _ = std::fs::remove_dir_all(&at);

    assert!(matches!(
        standings(&ctx).await.as_slice(),
        [crate::plugin::Fetchable::Unreachable { .. }]
    ));
}

/// A record naming no source has nowhere to ask, and says so.
#[tokio::test]
async fn a_record_naming_no_source_is_not_asked() {
    let ctx = ctx("no-source-listed");
    let at = source("no-source-listed", MANIFEST);
    let mut installed = report(installing(&ctx, &at).await)
        .map(|one| one.installed)
        .unwrap_or_default();
    for one in &mut installed {
        one.from = String::new();
    }

    let said = super::super::fetching::standings(&ctx, &installed).await;

    assert!(matches!(
        said.first().map(|one| &one.standing),
        Some(crate::plugin::Fetchable::Unasked { .. })
    ));
}

/// Two rehearsals of one commit at once, in one run and over one machine, each check it
/// out into a directory of its own and each removes only its own.
#[tokio::test]
async fn two_installs_of_one_commit_at_once_never_share_a_checkout() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = served("git-together", &serving);
    ctx.dry_run = true;

    let (one, other) = tokio::join!(
        from_git(&ctx, "https://example.org/plugin-komga"),
        from_git(&ctx, "https://example.org/plugin-komga"),
    );

    assert!(report(one).and_then(|one| one.install).is_some());
    assert!(report(other).and_then(|one| one.install).is_some());
    // Each install reads its offer and then answers it, so each checks out twice.
    let made = serving.checkouts();
    let mut apart = made.clone();
    apart.sort();
    apart.dedup();
    assert_eq!(made.len(), 4, "{made:?}");
    assert_eq!(
        apart.len(),
        made.len(),
        "two installs shared a checkout: {made:?}"
    );
    assert!(serving.left_nothing());
}

/// A checkout is made under lemonfiber's own data directory rather than the temporary
/// directory every user of the machine shares.
#[test]
fn a_checkout_is_made_under_lemonfibers_own_data_directory() {
    let ctx = ctx("git-where");
    let data = crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().to_path_buf());

    let at = super::super::fetching::checkout(&ctx, HEAD);

    assert!(
        at.as_ref()
            .zip(data.as_ref())
            .is_some_and(|(at, data)| at.starts_with(data.join("checkouts"))),
        "{at:?} under {data:?}"
    );
}

/// Where the checkout cannot be made, nothing is fetched and the refusal says so.
#[tokio::test]
async fn a_checkout_that_cannot_be_made_is_refused_before_anything_is_fetched() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let ctx = served("git-unmade", &serving);
    let data = crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().to_path_buf());
    // A file where the directory checkouts are made in would go.
    let _ = data.map(|data| {
        let _ = std::fs::create_dir_all(&data);
        std::fs::write(data.join("checkouts"), "in the way")
    });

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga").await),
        "PLUGIN-16"
    );
    assert!(!serving
        .asked()
        .iter()
        .any(|one| one.contains(&"fetch".to_owned())));
}

/// A machine not set up has no data directory to check anything out in.
#[tokio::test]
async fn a_machine_not_set_up_has_nowhere_to_check_out() {
    let serving = Arc::new(Serving::listing(&format!("{HEAD}\tHEAD\n")));
    let mut ctx = a_context().build();
    ctx.seams.runner = serving.clone();

    assert_eq!(
        refusal(from_git(&ctx, "https://example.org/plugin-komga").await),
        "CONFIG-3"
    );
}

/// A git that takes a while to answer, counting how many are asked at once.
#[derive(Default)]
struct Slow {
    /// How many are being asked now.
    now: std::sync::atomic::AtomicUsize,
    /// The most that were ever asked at once.
    most: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl Runner for Slow {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        use std::sync::atomic::Ordering::SeqCst;
        let now = self.now.fetch_add(1, SeqCst) + 1;
        self.most.fetch_max(now, SeqCst);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        self.now.fetch_sub(1, SeqCst);
        let url = asked_of(argv).get(2).cloned().unwrap_or_default();
        Ok(if url.ends_with("gone") {
            engine_refused("fatal: repository not found")
        } else {
            spoke("")
        })
    }
}

/// Listing what is installed asks the sources a few at a time rather than one after
/// another, and answers for each in the record's order.
#[tokio::test(start_paused = true)]
async fn sources_are_asked_a_few_at_a_time_and_answered_in_order() {
    let ctx = ctx("listed-at-once");
    let at = source("listed-at-once", MANIFEST);
    let one = report(installing(&ctx, &at).await).and_then(|done| done.installed.first().cloned());
    let installed: Vec<crate::plugin::Installed> = (0..6)
        .filter_map(|n| {
            one.clone().map(|one| crate::plugin::Installed {
                plugin: format!("plugin-{n}"),
                from: if n == 4 {
                    "https://example.org/gone".to_owned()
                } else {
                    format!("https://example.org/plugin-{n}")
                },
                ..one
            })
        })
        .collect();
    let slow = Arc::new(Slow::default());
    let mut ctx = ctx;
    ctx.seams.runner = slow.clone();

    let started = tokio::time::Instant::now();
    let said = super::super::fetching::standings(&ctx, &installed).await;

    assert_eq!(
        said.iter()
            .map(|one| one.plugin.as_str())
            .collect::<Vec<_>>(),
        ["plugin-0", "plugin-1", "plugin-2", "plugin-3", "plugin-4", "plugin-5"]
    );
    assert!(matches!(
        said.get(4).map(|one| &one.standing),
        Some(crate::plugin::Fetchable::Unreachable { .. })
    ));
    assert_eq!(
        slow.most.load(std::sync::atomic::Ordering::SeqCst),
        super::super::git::AT_ONCE
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(6));
}

//! A start of a form holding a part this build carries that this machine has never run:
//! its install offered, offline, and nothing of the form written or started without the
//! yes to it.

use async_trait::async_trait;

use super::embedding::ran;
use super::fetching::Serving;
use super::*;
use crate::app::{Command, Outcome};
use crate::model::LifecycleReport;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::process::{Failure, Output, Runner};
use crate::test_support::Reporting;

/// The part offered.
const PART: &str = "jellyfin";

/// Its adapter.
const ADAPTER: &str = "jellyfin-adapter";

/// The form holding it.
fn library() -> Vec<String> {
    vec!["library".to_owned()]
}

/// The git source an install reads, beside every other command run, each recorded.
struct Watching {
    /// What answers git.
    serving: Arc<Serving>,
    /// Every command run, git or not.
    seen: std::sync::Mutex<Vec<Vec<String>>>,
}

impl Watching {
    /// Whether anything was brought up.
    fn started(&self) -> bool {
        self.seen.lock().is_ok_and(|seen| {
            seen.iter()
                .any(|argv| argv.iter().any(|word| word == "up" || word == "start"))
        })
    }
}

#[async_trait]
impl Runner for Watching {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(argv.to_vec());
        }
        self.serving.run(argv).await
    }
}

/// A runner answering git as an install's source does, recording everything run.
fn watching() -> Arc<Watching> {
    Arc::new(Watching {
        serving: Arc::new(Serving::listing("")),
        seen: std::sync::Mutex::new(Vec::new()),
    })
}

/// A machine that has run every part the bundle installs but [`PART`], whose engine holds
/// no container for it, with its server and adapter answering every proof its install
/// asks.
fn fresh(name: &str, serving: &Arc<Watching>) -> Ctx {
    let mut ctx = super::embedding::unmoved(name, &serving.serving);
    ctx.seams.runner = serving.clone();
    ctx.seams.engine = Arc::new(
        Reporting::holding(&[ADAPTER], Lifecycle::Running, Health::Healthy).publishing(&[(
            ADAPTER,
            "127.0.0.1",
            8080,
        )]),
    );
    let _ = std::fs::remove_dir_all(crate::plugin::configuration(&stack_of(&ctx), PART));
    ctx
}

/// What an up of the library form, answering `offer`, came to.
async fn up(
    ctx: &Ctx,
    offer: Option<String>,
) -> Result<LifecycleReport, Box<crate::error::Problem>> {
    let up = Command::Up {
        forms: library(),
        offer,
    };
    match crate::app::dispatch(up, ctx).await? {
        Outcome::Lifecycle(report) => Ok(report),
        other => unreachable!("an up answers with its lifecycle report, not {other:?}"),
    }
}

/// Whether a start went past the install it offered: it ran, whatever its services then
/// came to, rather than answering with the install still on offer.
fn no_offer(report: &Result<LifecycleReport, Box<crate::error::Problem>>) -> bool {
    match report {
        Ok(one) => one.installing.is_empty(),
        Err(problem) => problem.code == crate::error::codes::life::NEVER_SETTLED,
    }
}

/// An up answering nothing offers the install, names it, and writes and starts nothing.
#[tokio::test]
async fn an_up_of_a_part_never_run_here_offers_its_install_and_starts_nothing() {
    let serving = watching();
    let ctx = fresh("offline-offered", &serving);

    let report = up(&ctx, None).await;

    let report = report.ok();
    assert_eq!(
        report.as_ref().map(|one| one
            .installing
            .iter()
            .map(|install| (install.plugin.as_str(), install.offer.is_some()))
            .collect::<Vec<_>>()),
        Some(vec![(PART, true)])
    );
    assert!(report
        .as_ref()
        .is_some_and(|one| one.offer.is_some() && one.command.is_empty() && !one.rehearsed));
    assert!(!serving.started());
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A rehearsal names the install on offer beside what the start would run, and installs
/// nothing.
#[tokio::test]
async fn a_rehearsed_up_names_the_install_and_installs_nothing() {
    let serving = watching();
    let mut ctx = fresh("offline-rehearsed", &serving);
    ctx.dry_run = true;

    let report = up(&ctx, None).await.ok();

    assert!(report.as_ref().is_some_and(|one| one.rehearsed
        && !one.command.is_empty()
        && one.offer.is_some()
        && one.installing.iter().any(|install| install.plugin == PART)));
    assert!(!serving.started());
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// An up answering the offer standing now installs the part from this build and then
/// starts the form, with nothing left on offer.
#[tokio::test]
async fn an_up_answering_the_offer_installs_the_part_and_starts() {
    let serving = watching();
    let ctx = fresh("offline-answered", &serving);
    let offer = up(&ctx, None).await.ok().and_then(|report| report.offer);
    assert!(offer.is_some());

    let report = up(&ctx, offer).await;

    assert!(no_offer(&report), "{report:?}");
    assert!(serving.started());
    assert_eq!(counted(reading(&ctx).await), Some(1));
}

/// An up answering an offer that is not the one standing now is refused, and installs,
/// writes and starts nothing.
#[tokio::test]
async fn an_up_answering_a_moved_offer_is_refused() {
    let serving = watching();
    let ctx = fresh("offline-moved", &serving);

    let refused = up(&ctx, Some("not-the-offer".to_owned())).await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::life::OFFLINE_INSTALL_MOVED)
    );
    assert!(!serving.started());
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A part whose stack service has run here is a move in place rather than an install,
/// so an up offers no install and starts as it always has.
#[tokio::test]
async fn a_part_that_has_run_here_is_offered_a_move_and_no_install() {
    let serving = watching();
    let ctx = fresh("offline-ran", &serving);
    ran(&stack_of(&ctx), PART);
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;

    let rehearsed = up(&rehearsing, None).await;
    let real = up(&ctx, None).await;

    assert!(
        rehearsed.as_ref().is_ok_and(|one| one.installing.is_empty()
            && one.offer.is_none()
            && one.moves.iter().any(|offered| offered.plugin == PART)),
        "{rehearsed:?}"
    );
    assert!(no_offer(&real) && serving.started(), "{real:?}");
}

/// A streamed start answering no install it offers is refused before anything runs.
#[tokio::test]
async fn a_streamed_start_answering_nothing_is_refused() {
    let serving = watching();
    let ctx = fresh("offline-streamed", &serving);

    let opened = crate::app::start_progress(&ctx, &library(), &[], None).await;

    assert_eq!(
        opened.err().map(|problem| problem.code),
        Some(crate::error::codes::life::OFFLINE_INSTALL_UNANSWERED)
    );
    assert!(!serving.started());
}

/// Whether the whole form was brought up: a start naming no service, as a start of
/// forms is, rather than one naming a plugin's own.
fn brought_the_form_up(serving: &Watching) -> bool {
    serving.seen.lock().is_ok_and(|seen| {
        seen.iter().any(|argv| {
            argv.iter().any(|word| word == "up") && !argv.iter().any(|word| word == "--")
        })
    })
}

/// An install agreed to whose proofs do not hold is put back, and stops the start: the
/// part is not recorded and nothing of the form is started.
#[tokio::test]
async fn an_install_that_does_not_hold_stops_the_start() {
    let serving = watching();
    let ctx =
        fresh("offline-unheld", &serving).with_http(lemonfiber_fixtures::http::Fake::silent());
    let offer = up(&ctx, None).await.ok().and_then(|report| report.offer);
    assert!(offer.is_some());

    let refused = up(&ctx, offer).await.err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(crate::error::codes::life::OFFLINE_INSTALL_UNHELD)
    );
    assert!(
        refused
            .as_ref()
            .is_some_and(|problem| problem.summary.contains(PART)
                && problem
                    .remedies
                    .iter()
                    .any(|remedy| remedy.action.contains("lemonfiber plugin install jellyfin"))),
        "{refused:?}"
    );
    assert!(!brought_the_form_up(&serving));
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A streamed start answering the offer standing now installs the part from this build
/// and then streams the start of the form.
#[tokio::test]
async fn a_streamed_start_answering_the_offer_installs_and_starts() {
    let serving = watching();
    let ctx = fresh("offline-streamed-answered", &serving);
    let offer = up(&ctx, None).await.ok().and_then(|report| report.offer);
    assert!(offer.is_some());

    let opened = crate::app::start_progress(&ctx, &library(), &[], offer.as_deref()).await;

    assert!(opened.is_ok(), "{:?}", opened.err());
    assert_eq!(counted(reading(&ctx).await), Some(1));
    assert!(brought_the_form_up(&serving));
}

/// A streamed start of named services offers no install and answers none: it installs
/// nothing and starts what it names.
#[tokio::test]
async fn a_streamed_start_of_named_services_installs_nothing() {
    let serving = watching();
    let ctx = fresh("offline-streamed-named", &serving);

    let opened =
        crate::app::start_progress(&ctx, &library(), &["navidrome".to_owned()], None).await;

    assert!(opened.is_ok(), "{:?}", opened.err());
    assert_eq!(counted(reading(&ctx).await), Some(0));
    assert!(serving
        .seen
        .lock()
        .is_ok_and(|seen| seen.iter().any(|argv| {
            argv.iter().any(|word| word == "up") && argv.iter().any(|word| word == "navidrome")
        })));
}

/// Nothing is offered where the record of what is installed cannot be read, and an
/// install whose reading cannot be made is offered with why.
#[tokio::test]
async fn an_unread_record_offers_nothing_and_an_unread_install_says_why() {
    let serving = watching();
    let ctx = fresh("offline-unread", &serving);
    let data = crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().to_path_buf());
    let _ = data.map(|data| {
        let _ = std::fs::create_dir_all(&data);
        std::fs::write(data.join("checkouts"), "in the way")
    });

    let unread = crate::app::offline(&ctx, &library()).await;

    assert!(
        unread.is_ok_and(|offline| offline.is_some_and(|offline| offline
            .installs
            .iter()
            .all(|install| install.offer.is_none() && install.why.is_some())))
    );

    let record = crate::app::plugins::kept_at(&ctx).unwrap_or_default();
    assert!(std::fs::write(record, "{ not json").is_ok());
    let offline = crate::app::offline(&ctx, &library()).await;
    assert!(offline.is_ok_and(|offline| offline.is_none()));
}

/// What a switch to the library form, answering `offer`, came to.
async fn switch(
    ctx: &Ctx,
    offer: Option<String>,
) -> Result<LifecycleReport, Box<crate::error::Problem>> {
    let switch = Command::Switch {
        forms: library(),
        offer,
    };
    match crate::app::dispatch(switch, ctx).await? {
        Outcome::Lifecycle(report) => Ok(report),
        other => unreachable!("a switch answers with its lifecycle report, not {other:?}"),
    }
}

/// A switch is a start too: answering nothing, it offers the install and stops, writes
/// and starts nothing; rehearsed, it names the install; answering the offer, it installs
/// the part and switches.
#[tokio::test]
async fn a_switch_offers_the_install_as_an_up_does() {
    let serving = watching();
    let ctx = fresh("offline-switch", &serving);

    let unanswered = switch(&ctx, None).await.ok();
    assert!(unanswered
        .as_ref()
        .is_some_and(|one| one.command.is_empty()
            && one.installing.iter().any(|install| install.plugin == PART)));
    assert!(!serving.started());

    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    let rehearsed = switch(&rehearsing, None).await.ok();
    assert!(rehearsed.as_ref().is_some_and(
        |one| one.rehearsed && one.installing.iter().any(|install| install.plugin == PART)
    ));
    assert_eq!(counted(reading(&ctx).await), Some(0));

    let switched = switch(&ctx, unanswered.and_then(|one| one.offer)).await;
    assert!(no_offer(&switched), "{switched:?}");
    assert_eq!(counted(reading(&ctx).await), Some(1));
}

/// A start at boot starts everything the forms hold but the part waiting for its install.
#[tokio::test]
async fn a_boot_starts_all_but_the_part_waiting() {
    let serving = watching();
    let ctx = fresh("offline-boot", &serving);

    let waiting = crate::app::engine::without_waiting(&ctx, &library()).await;

    let Ok(Some((started, offline))) = waiting else {
        unreachable!("a part waits for its install: {waiting:?}");
    };
    assert!(offline.installs.iter().all(|one| one.plugin == PART));
    assert!(!started.iter().any(|service| service == PART));
    assert!(
        started.iter().any(|service| service == "navidrome"),
        "{started:?}"
    );
}

/// What runs inside a waiting part's network, and what runs inside that, is left out with
/// it, since Compose starts the part a service runs through; nothing else is.
#[test]
fn what_runs_through_a_waiting_part_is_left_out_with_it() {
    use crate::topology::Placed;
    let placed = std::collections::BTreeMap::from([
        (
            "qbittorrent".to_owned(),
            Placed::Inside("gluetun".to_owned()),
        ),
        (
            "seeding".to_owned(),
            Placed::Inside("qbittorrent".to_owned()),
        ),
        ("sonarr".to_owned(), Placed::Nowhere),
    ]);

    let mut left = crate::app::engine::left_out(vec!["gluetun".to_owned()], &placed);
    left.sort();

    assert_eq!(left, ["gluetun", "qbittorrent", "seeding"]);
    assert_eq!(
        crate::app::engine::left_out(vec!["jellyfin".to_owned()], &placed),
        ["jellyfin"]
    );
}

/// Nothing is left out at boot where every part the forms hold is installed or has run.
#[tokio::test]
async fn a_boot_leaves_nothing_out_where_nothing_waits() {
    let serving = watching();
    let ctx = fresh("offline-boot-none", &serving);
    ran(&stack_of(&ctx), PART);

    let waiting = crate::app::engine::without_waiting(&ctx, &library()).await;

    assert!(waiting.is_ok_and(|waiting| waiting.is_none()));
}

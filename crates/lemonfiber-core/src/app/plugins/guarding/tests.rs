//! lemonfiber's own guards moved onto their plugins, with no yes of their own, by the acts
//! that write the project and leave the stack running, and never moved back.

use std::sync::Arc;

use lemonfiber_contract::capabilities::{invite::decline, request::gate};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;

use crate::app::plugins::Asked;
use crate::app::Ctx;
use crate::journal::{Change, Kind};
use crate::plugin::Installs;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::adapted::Adapted;
use crate::test_support::{a_context, a_password, env_at, guards, Reporting};
use crate::update::run::Moved;

/// The request gate, the stack's own service and the plugin moving into its place.
const GATE: &str = guards::GATE.plugin;

/// The decline service, the same.
const DECLINE: &str = guards::DECLINE.plugin;

/// What `guard`'s plugin says of itself: the contract it speaks, and a release at the
/// digest its image is pinned by where `released`.
fn about(guard: Adapted, spoken: &str, released: bool) -> Answer {
    let listed: Vec<_> = guard
        .digest()
        .filter(|_| released)
        .map(|digest| serde_json::json!({ "version": "0.17.0", "digest": digest }))
        .into_iter()
        .collect();
    Answer::reply(
        200,
        serde_json::json!({ "speaks": [spoken], "upstream": guard.plugin, "releases": listed })
            .to_string(),
    )
}

/// Where an adapter speaking its contracts on `port` is asked what it speaks.
fn asked_about(port: u16) -> &'static str {
    Box::leak(format!("{port}{}", lemonfiber_contract::adapter::about_path()).into_boxed_str())
}

/// Each guard's plugin answering as its proofs ask, its account of itself listing the
/// release its image is pinned by where `released`, and every contract call refused
/// without its key.
fn answering(released: bool) -> Arc<Fake> {
    let (gate, decline) = (guards::GATE.contract(), guards::DECLINE.contract());
    Fake::by_path(vec![
        (
            asked_about(gate),
            about(
                guards::GATE,
                &lemonfiber_contract::spoken(gate::CAPABILITY, gate::MAJOR),
                released,
            ),
        ),
        (
            asked_about(decline),
            about(
                guards::DECLINE,
                &lemonfiber_contract::spoken(decline::CAPABILITY, decline::MAJOR),
                released,
            ),
        ),
        ("/health", Answer::reply(200, r#"{"key":null}"#)),
        ("/lemonfiber/", Answer::reply(401, "")),
    ])
}

/// A machine running both guards as the stack's own services, which this build moves
/// onto their plugins, each answering as its proofs ask.
fn unmoved(name: &str) -> Ctx {
    let env_file = env_at(name, &a_password());
    let stack = env_file.with_file_name("data").join("stack");
    a_context()
        .engine(Arc::new(running(&[GATE, DECLINE])))
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            stack_dir: Some(stack),
            ..crate::config::Settings::default()
        })
        .build()
        .with_http(answering(true))
        .with_program(Proving::exiting(0, "request-gate 0.17.0\n"))
        .with_images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()))
        .with_patience(std::time::Duration::ZERO)
}

/// The engine's account of each of `services` running healthy, each guard among them
/// published on the port its plugin speaks its contract on.
fn running(services: &[&str]) -> Reporting {
    let (gate, decline) = (guards::GATE.contract(), guards::DECLINE.contract());
    Reporting::holding(services, Lifecycle::Running, Health::Healthy).forwarding(&[
        (GATE, "127.0.0.1", gate, gate),
        (DECLINE, "127.0.0.1", decline, decline),
    ])
}

/// The stack `ctx` runs.
fn manifest(ctx: &Ctx) -> lemonfiber_manifest::Manifest {
    ctx.stack
        .checked_manifest(ctx.today())
        .unwrap_or_else(|_| unreachable!("the repository's stack reads"))
}

/// Each guard, moved onto its plugin.
fn both_moved() -> Vec<Moved> {
    [DECLINE, GATE]
        .into_iter()
        .map(|guard| Moved {
            plugin: guard.to_owned(),
            service: guard.to_owned(),
            taken: true,
            why: None,
        })
        .collect()
}

/// Every change `ctx`'s journal holds, oldest first.
fn journalled(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

/// Each move in place the journal holds, as its run, its service and its release.
fn moves_journalled(ctx: &Ctx) -> Vec<(String, String, String)> {
    journalled(ctx)
        .into_iter()
        .filter_map(|change| match change.kind {
            Kind::MovedInPlace { service, release } => Some((change.operation, service, release)),
            _ => None,
        })
        .collect()
}

/// The plugins `ctx` records as installed.
fn installed(ctx: &Ctx) -> Vec<String> {
    crate::app::plugins::read(ctx)
        .map(|held| {
            held.installed()
                .iter()
                .map(|one| one.plugin.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Both guards move onto their plugins once, each journalled as its own install's run
/// with the move beside it, and a second pass finds nothing left to move.
#[tokio::test]
async fn the_guards_are_moved_once_each_as_a_run_of_its_own() {
    let ctx = unmoved("guards-moved");

    let first = super::taken(&ctx, &manifest(&ctx)).await;
    let second = super::taken(&ctx, &manifest(&ctx)).await;

    assert_eq!(first, both_moved());
    assert!(second.is_empty(), "{second:?}");
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
    assert_eq!(
        moves_journalled(&ctx),
        [DECLINE, GATE]
            .into_iter()
            .map(|guard| (
                crate::plugin::owner(guard),
                guard.to_owned(),
                env!("CARGO_PKG_VERSION").to_owned()
            ))
            .collect::<Vec<_>>()
    );
}

/// Each guard read as its move would go, and neither taken.
fn both_read() -> Vec<Moved> {
    both_moved()
        .into_iter()
        .map(|one| Moved {
            taken: false,
            ..one
        })
        .collect()
}

/// A rehearsal reads each move as it would go, takes none, and journals nothing.
#[tokio::test]
async fn a_rehearsal_reads_each_move_and_takes_none() {
    let ctx = unmoved("guards-rehearsed").rehearsing();

    let read = super::taken(&ctx, &manifest(&ctx)).await;

    assert_eq!(read, both_read());
    assert!(installed(&ctx).is_empty());
    assert!(moves_journalled(&ctx).is_empty());
}

/// A guard whose capability the operator gave another filler stays where it is, and the
/// other guard moves.
#[tokio::test]
async fn a_guard_whose_capability_went_to_another_filler_is_left_where_it_is() {
    let ctx = unmoved("guards-given-away");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let chose = crate::config::store::set(
        &env,
        crate::wiring::FILLS_KEY,
        &format!("{}=another-gate", gate::CAPABILITY),
    );
    assert!(chose.is_ok(), "{chose:?}");

    let offered: Vec<String> = super::on_offer(&ctx, &manifest(&ctx))
        .into_iter()
        .map(|one| one.plugin)
        .collect();
    let moved = super::taken(&ctx, &manifest(&ctx)).await;

    assert_eq!(offered, vec![DECLINE.to_owned()]);
    assert_eq!(
        moved,
        both_moved()
            .into_iter()
            .filter(|one| one.plugin == DECLINE)
            .collect::<Vec<_>>()
    );
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned()]);
}

/// A move the install refuses, here for an overlay already declaring the guard's
/// service, is not taken and stays on offer saying why; the other guard moves all the
/// same.
#[tokio::test]
async fn a_move_the_install_refuses_stays_on_offer_saying_why() {
    let mut ctx = unmoved("guards-overlaid");
    let overlay = ctx
        .settings
        .env_file
        .clone()
        .unwrap_or_default()
        .with_file_name("overlay.yml");
    let _ = std::fs::write(&overlay, format!("services:\n  {GATE}:\n    image: x\n"));
    ctx.settings.overlays = vec![overlay];

    let moved = super::taken(&ctx, &manifest(&ctx)).await;
    let still: Vec<String> = super::on_offer(&ctx, &manifest(&ctx))
        .into_iter()
        .map(|one| one.plugin)
        .collect();

    let gate = moved.iter().find(|one| one.plugin == GATE);
    assert!(
        gate.is_some_and(|one| !one.taken && one.why.is_some()),
        "{moved:?}"
    );
    assert!(
        moved.iter().any(|one| one.plugin == DECLINE && one.taken),
        "{moved:?}"
    );
    assert_eq!(still, vec![GATE.to_owned()]);
}

/// A move whose install is taken back, here for a plugin listing no release at the
/// digest its image is pinned by, is not taken and says which proof did not hold.
#[tokio::test]
async fn a_move_taken_back_names_the_proof_that_did_not_hold() {
    let ctx = unmoved("guards-unproven").with_http(answering(false));

    let moved = super::taken(&ctx, &manifest(&ctx)).await;

    assert_eq!(
        moved
            .iter()
            .map(|one| (one.plugin.as_str(), one.taken, one.why.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (
                DECLINE,
                false,
                Some("its install was taken back over decline-speaks")
            ),
            (
                GATE,
                false,
                Some("its install was taken back over request-gate-speaks")
            ),
        ]
    );
    assert!(installed(&ctx).is_empty());
}

/// A stack service no part moves in place of.
const UNMOVED: &str = "flaresolverr";

/// An update narrowed to a service no part moves in place of, confirmed or only shown.
fn updating(confirm: bool) -> crate::update::run::Asked {
    crate::update::run::Asked {
        service: Some(UNMOVED.to_owned()),
        confirm,
        wait: crate::app::Waiting::Never,
        offer: None,
    }
}

/// A confirmed update, even one narrowed to another service, moves both guards with no
/// yes of their own and before anything else it takes, and offers neither as one of its
/// own moves; its reading shows them and moves nothing.
#[tokio::test]
async fn an_update_moves_the_guards_and_offers_neither_as_its_own() {
    let ctx = unmoved("guards-updated");

    let read = crate::update::run::update(&ctx, updating(false)).await;
    assert_eq!(
        read.as_ref().ok().map(|report| report.guards.clone()),
        Some(both_read()),
        "{read:?}"
    );
    assert!(installed(&ctx).is_empty());

    let confirmed = crate::update::run::update(&ctx, updating(true)).await;
    let report = confirmed.as_ref().ok();

    assert_eq!(
        report.map(|report| report.guards.clone()),
        Some(both_moved()),
        "{confirmed:?}"
    );
    assert!(
        report.is_some_and(|report| report
            .moves
            .iter()
            .all(|one| one.plugin != GATE && one.plugin != DECLINE)
            && report
                .moved
                .iter()
                .all(|one| one.plugin != GATE && one.plugin != DECLINE)),
        "{confirmed:?}"
    );
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
}

/// What a plugin act on `plugin` came to, as a report says it: recorded where `landed`.
fn act(plugin: &str, landed: bool) -> Installs {
    let would = crate::test_support::an_installed(plugin, Vec::new());
    Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(crate::plugin::Install {
            would,
            recorded: landed,
            changes: Vec::new(),
            proofs: Vec::new(),
            against: None,
            verified: None,
            overrides: Vec::new(),
            reversed: None,
            asks: Vec::new(),
            contests: Vec::new(),
            recipes_ran: Vec::new(),
            taking: Vec::new(),
            granting: Vec::new(),
            sharing: Vec::new(),
            waiting: Vec::new(),
            secured: None,
        })),
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
    }
}

/// An install of `plugin` answering `agreement`, where there is one.
fn installing(plugin: &str, agreement: Option<&str>) -> Asked {
    Asked::Install {
        source: crate::plugin::Source::named(plugin),
        consent: crate::app::plugins::Consent {
            agreement: agreement.map(str::to_owned),
            ..crate::app::plugins::Consent::default()
        },
    }
}

/// Installing another plugin moves the guards once it lands; its reading shows them; an
/// install that did not land, an install of a guard itself and an act that answers no
/// offer move nothing.
#[tokio::test]
async fn installing_another_plugin_moves_the_guards_once_it_lands() {
    let ctx = unmoved("guards-beside-an-install");

    let read = super::carried(&ctx, &installing("komga", None), &act("komga", false)).await;
    let unlanded = super::carried(
        &ctx,
        &installing("komga", Some("yes")),
        &act("komga", false),
    )
    .await;
    let itself = super::carried(&ctx, &installing(GATE, Some("yes")), &act(GATE, true)).await;
    let listed = super::carried(&ctx, &Asked::Installed, &act("komga", true)).await;
    assert_eq!(read, both_read());
    assert!(unlanded.is_empty() && itself.is_empty() && listed.is_empty());
    assert!(installed(&ctx).is_empty());

    let landed = super::carried(&ctx, &installing("komga", Some("yes")), &act("komga", true)).await;

    assert_eq!(landed, both_moved());
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
}

/// The stamp of the run that moved `guard`.
fn moved_at(ctx: &Ctx, guard: &str) -> String {
    journalled(ctx)
        .into_iter()
        .find(
            |change| matches!(&change.kind, Kind::MovedInPlace { service, .. } if service == guard),
        )
        .map(|change| change.at)
        .unwrap_or_default()
}

/// The code a refusal carries and what it says, or nothing where there was none.
fn refused<T: std::fmt::Debug>(
    outcome: &Result<T, Box<crate::error::Problem>>,
) -> Option<(crate::error::Code, String)> {
    outcome.as_ref().err().map(|problem| {
        (
            problem.code,
            format!("{} {}", problem.summary, problem.meaning),
        )
    })
}

/// An act over a stack that cannot be read carries no move.
#[tokio::test]
async fn an_act_over_a_stack_that_cannot_be_read_carries_no_move() {
    let mut ctx = unmoved("guards-unread");
    ctx.stack = crate::test_support::nowhere();

    let carried =
        super::carried(&ctx, &installing("komga", Some("yes")), &act("komga", true)).await;

    assert!(carried.is_empty(), "{carried:?}");
    assert!(installed(&ctx).is_empty());
}

/// A move with nowhere to journal it, or a journal that will not be written, is refused;
/// an install that moves nothing journals nothing.
#[test]
fn a_move_that_cannot_be_journalled_is_refused() {
    let ctx = unmoved("guards-unjournalled");
    let stack = manifest(&ctx);
    let gate = guards::GATE.installed();
    let journal = crate::app::targets::layout(&ctx).map(|paths| paths.journal());
    let _ = journal
        .as_ref()
        .map(|journal| std::fs::create_dir_all(journal.join("a directory")));
    let mut homeless = ctx.clone();
    homeless.settings.env_file = None;

    for at in [&homeless, &ctx] {
        assert!(gate
            .as_ref()
            .is_some_and(|gate| super::journalled(at, &stack, gate).is_err()));
    }
    let other = crate::test_support::an_installed("komga", Vec::new());
    assert!(super::journalled(&homeless, &stack, &other).is_ok());
}

/// Undoing a move by its stamp, rehearsed or not, is refused with the code of its own
/// and names the release that moved the guard; the guard stays installed.
#[tokio::test]
async fn undoing_a_move_is_refused_naming_the_release() {
    let ctx = unmoved("guards-undone");
    let _ = super::taken(&ctx, &manifest(&ctx)).await;
    let at = moved_at(&ctx, GATE);

    for asking in [ctx.clone().rehearsing(), ctx.clone()] {
        let undone = crate::app::putting_back::undo(&asking, Some(&at)).await;

        let said = refused(&undone);
        assert_eq!(
            said.as_ref().map(|(code, _)| *code),
            Some(crate::error::codes::undo::MOVED_FOR_GOOD),
            "{undone:?}"
        );
        assert!(
            said.is_some_and(|(_, said)| said.contains(env!("CARGO_PKG_VERSION"))),
            "{undone:?}"
        );
    }
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
}

/// Removing a moved guard, read or answered, is refused with the code of its own and
/// names the release that moved it; the guard stays installed.
#[tokio::test]
async fn removing_a_moved_guard_is_refused_naming_the_release() {
    let ctx = unmoved("guards-removed");
    let _ = super::taken(&ctx, &manifest(&ctx)).await;

    for agreement in [None, Some("yes")] {
        let removed = crate::app::plugins::plugins(
            &ctx,
            &Asked::Remove {
                plugin: GATE.to_owned(),
                consent: crate::app::plugins::Consent {
                    agreement: agreement.map(str::to_owned),
                    ..crate::app::plugins::Consent::default()
                },
            },
        )
        .await;

        let said = refused(&removed);
        assert_eq!(
            said.as_ref().map(|(code, _)| *code),
            Some(crate::error::codes::undo::MOVED_FOR_GOOD),
            "{removed:?}"
        );
        assert!(
            said.is_some_and(|(_, said)| said.contains(env!("CARGO_PKG_VERSION"))),
            "{removed:?}"
        );
    }
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
}

/// A reset carries the guard moves: its reading shows them, and the reset answering it
/// moves both.
#[tokio::test]
async fn a_reset_carries_the_guard_moves() {
    let ctx = unmoved("guards-reset");

    let read = crate::app::reset::reset(&ctx, None).await;
    let offer = read
        .as_ref()
        .map(|report| report.offer.clone())
        .unwrap_or_default();
    assert_eq!(
        read.as_ref().ok().map(|report| report.guards.clone()),
        Some(both_read()),
        "{read:?}"
    );
    assert!(installed(&ctx).is_empty());

    let confirmed = crate::app::reset::reset(&ctx, Some(offer)).await;

    assert_eq!(
        confirmed.as_ref().ok().map(|report| report.guards.clone()),
        Some(both_moved()),
        "{confirmed:?}"
    );
    assert_eq!(installed(&ctx), vec![DECLINE.to_owned(), GATE.to_owned()]);
}

/// A setting set carries the guard moves: its reading shows them and moves nothing.
#[tokio::test]
async fn a_setting_set_carries_the_guard_moves() {
    let ctx = unmoved("guards-set");

    let read = crate::settings::run::settings(
        &ctx,
        &crate::app::Settling::Set {
            values: [("flaresolverr:log-level".to_owned(), "debug".to_owned())].into(),
            agreement: None,
        },
    )
    .await;

    assert!(
        matches!(&read, Ok(crate::app::Outcome::Settings(report)) if report.guards == both_read()),
        "{read:?}"
    );
    assert!(installed(&ctx).is_empty());
}

/// A choice of filler carries the guard moves: its reading shows them and moves nothing.
#[tokio::test]
async fn a_filler_chosen_carries_the_guard_moves() {
    let ctx = unmoved("guards-filled");

    let read = crate::wiring::run::wiring(
        &ctx,
        &crate::app::Linking::Fill(crate::app::Filling {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
            reason: None,
            agreement: None,
        }),
    )
    .await;

    assert!(
        matches!(&read, Ok(crate::app::Outcome::Substitution(report)) if report.guards == both_read()),
        "{read:?}"
    );
    assert!(installed(&ctx).is_empty());
}

/// The guard moves an up of the library form came to, or nothing where it was refused.
async fn up(ctx: &Ctx) -> Result<Vec<Moved>, Box<crate::error::Problem>> {
    let outcome = crate::app::dispatch(
        crate::app::Command::Up {
            forms: vec!["library".to_owned()],
        },
        ctx,
    )
    .await?;
    Ok(match outcome {
        crate::app::Outcome::Lifecycle(report) => report.guards,
        _ => Vec::new(),
    })
}

/// A rehearsed up shows both moves and takes none; an up takes them, each journalled as
/// its own run; and the next up finds nothing left to move.
#[tokio::test]
async fn an_up_moves_the_guards_once_and_its_rehearsal_takes_none() {
    let mut ctx = unmoved("guards-up");
    let services = manifest(&ctx).services;
    let declared: Vec<&str> = services.iter().map(|service| service.id.as_str()).collect();
    ctx.seams.engine = Arc::new(running(&declared));

    let rehearsed = up(&ctx.clone().rehearsing()).await;
    assert_eq!(rehearsed.as_ref().ok(), Some(&both_read()), "{rehearsed:?}");
    assert!(installed(&ctx).is_empty());

    let first = up(&ctx).await;
    let second = up(&ctx).await;

    assert_eq!(first.as_ref().ok(), Some(&both_moved()), "{first:?}");
    assert!(second.as_ref().is_ok_and(Vec::is_empty), "{second:?}");
    assert_eq!(moves_journalled(&ctx).len(), 2);
}

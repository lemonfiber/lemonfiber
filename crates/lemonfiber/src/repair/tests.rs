use lemonfiber_core::instant::Instant;
use lemonfiber_core::repair::run::{Mended, Report};
use lemonfiber_core::repair::{Outcome, Repair};

use lemonfiber_core::stack::Source;

use crate::exit::{repairing, shown, success};

use std::sync::Arc;

use lemonfiber_core::ports::asking::Asking;
use lemonfiber_fixtures::typing::Typing;

use super::{repaired, run, Consent, Ctx, Mending};
use lemonfiber::cli::Fixing;
use lemonfiber_core::config::paths::Paths;

/// Where a test's records live, in a scratch directory of its own — named, because a
/// test that undoes a journal must not be reading one another test wrote.
fn paths(name: &str) -> (Paths, lemonfiber_fixtures::scratch::Scratch) {
    let root = lemonfiber_fixtures::scratch::Scratch::unmade(&format!("repair-cli-{name}"));
    (
        Paths::rooted(&root.join("config"), &root.join("data")),
        root,
    )
}

/// A context over the stack this binary ships, with nothing configured — so nothing is
/// wrong that lemonfiber could put right, which is the state a healthy machine is in.
fn ctx() -> Ctx {
    lemonfiber_testing::a_live_context()
        .over(Source::Embedded(&lemonfiber::carried::STACK))
        .build()
}

/// The same context, keeping its records in the layout `paths` names.
fn ctx_in(paths: &Paths) -> Ctx {
    let mut ctx = ctx();
    ctx.settings.env_file = Some(paths.env_file());
    ctx.settings.stack_dir = Some(paths.stack());
    ctx
}

fn repair(reversible: bool) -> Repair {
    Repair {
        check: "vpn.port-forward-client".to_owned(),
        does: "Move the download client onto the forwarded port".to_owned(),
        effects: vec!["Transfers in flight pause briefly".to_owned()],
        reversible,
    }
}

fn report(outcomes: Vec<Outcome>) -> Report {
    Report {
        rehearsed: false,
        offered: Vec::new(),
        agreement: String::new(),
        beyond: Vec::new(),
        mended: outcomes
            .into_iter()
            .map(|outcome| Mended {
                repair: repair(true),
                outcome,
            })
            .collect(),
        acted: true,
    }
}

/// Nothing repaired is nothing to put back — said plainly rather than by reversing
/// the whole journal, which holds the wiring lemonfiber seeded and what the first run
/// wrote as well.
#[tokio::test]
async fn an_undo_with_nothing_repaired_puts_nothing_back() {
    let (at, _root) = paths("nothing-undone");
    let code = run(
        &ctx_in(&at),
        Mending {
            fixing: Fixing {
                fix: false,
                yes: false,
                disruptive: false,
            },
            undo: true,
        },
        Typing::answering("n"),
        false,
    )
    .await;

    assert_eq!(shown(code), success());
}

/// The reversal itself: what the last repair set goes back to what it was, read from
/// the journal that repair wrote rather than from anything the run still holds.
#[tokio::test]
async fn an_undo_puts_back_what_the_last_repair_set() {
    use lemonfiber_core::journal::{Change, Kind};

    let (paths, _root) = paths("undone");
    let journal = paths.journal();
    if let Some(dir) = journal.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let recorded = Change {
        at: Instant::from_seconds(1000),
        operation: lemonfiber_core::repair::OPERATION.to_owned(),
        target: "qbittorrent".to_owned(),
        kind: Kind::Set {
            key: "QBITTORRENT_PORT".to_owned(),
            previous: Some("8080".to_owned()),
            current: "51413".to_owned(),
        },
    };
    std::fs::write(
        &journal,
        serde_json::to_string(&recorded).unwrap_or_default() + "\n",
    )
    .ok();
    // And the setting as that repair left it. A reversal puts back what it wrote
    // over, so a machine that never held the repair's value has nothing of the
    // repair on it to reverse.
    lemonfiber_core::config::store::set(&paths.env_file(), "QBITTORRENT_PORT", "51413").ok();

    let code = run(
        &ctx_in(&paths),
        Mending {
            fixing: Fixing {
                fix: false,
                yes: false,
                disruptive: false,
            },
            undo: true,
        },
        Typing::answering("n"),
        false,
    )
    .await;

    assert_eq!(shown(code), success());
    let env = std::fs::read_to_string(paths.env_file()).unwrap_or_default();
    assert!(env.contains("QBITTORRENT_PORT=8080"), "{env}");
}

/// A repair that registered something can only be put back by the service that holds
/// it, so an undo run when that service is gone says which one it needed rather than
/// reporting a reversal that did not happen.
#[tokio::test]
async fn an_undo_that_needs_a_service_that_is_gone_says_so() {
    use lemonfiber_core::journal::{Change, Kind};

    let (paths, _root) = paths("beyond-reach");
    let journal = paths.journal();
    if let Some(dir) = journal.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let recorded = Change {
        at: Instant::from_seconds(1000),
        operation: lemonfiber_core::repair::OPERATION.to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Created {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
        },
    };
    std::fs::write(
        &journal,
        serde_json::to_string(&recorded).unwrap_or_default() + "\n",
    )
    .ok();

    let code = run(
        &ctx_in(&paths),
        Mending {
            fixing: Fixing {
                fix: false,
                yes: false,
                disruptive: false,
            },
            undo: true,
        },
        Typing::answering("n"),
        false,
    )
    .await;

    assert_ne!(shown(code), success());
}

/// A stack that will not read is the one thing every check needs before any of them
/// can run, so the operator hears about it rather than being told there is nothing to
/// put right — which would be true of a machine lemonfiber cannot see at all.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_refused_rather_than_called_healthy() {
    let nowhere = lemonfiber_testing::a_live_context()
        .over(Source::External(std::path::Path::new("/no/such/stack")))
        .build();

    let code = run(
        &nowhere,
        Mending {
            fixing: Fixing {
                fix: true,
                yes: true,
                disruptive: false,
            },
            undo: false,
        },
        Typing::answering("n"),
        false,
    )
    .await;

    assert_ne!(shown(code), success());
}

/// The command end to end, over a stack with nothing wrong that lemonfiber can mend.
///
/// Every stance reaches the same answer here — there is nothing to offer, so there is
/// nothing to ask about and nothing to carry out — which is what a run on a healthy
/// machine should say however it was asked.
#[tokio::test]
async fn a_run_with_nothing_to_mend_succeeds_however_it_was_asked() {
    // Asked about each, told to go ahead, and read by a script: all three reach the
    // same answer on a machine with nothing wrong that lemonfiber could put right.
    assert_eq!(asked_for(false, false).await, success());
    assert_eq!(asked_for(true, false).await, success());
    assert_eq!(asked_for(false, true).await, success());
}

/// One run of the command, as the flags would arrive.
///
/// A named function rather than a closure: a closure is a function of its own as far
/// as coverage is concerned, and one inside a test is one the gate counts and no other
/// test enters.
#[tokio::test]
async fn a_repair_with_nobody_there_is_told_which_flags_rather_than_asked() {
    let nobody = Typing::nobody();
    let (at, _root) = paths("nobody");
    let code = run(
        &ctx_in(&at),
        Mending {
            fixing: Fixing {
                fix: true,
                yes: false,
                disruptive: false,
            },
            undo: false,
        },
        Arc::clone(&nobody) as Arc<dyn Asking>,
        false,
    )
    .await;

    assert_ne!(
        shown(code),
        success(),
        "a run that cannot be asked is told what to pass instead"
    );
    assert!(
        nobody.asked().is_empty(),
        "the question was never put, rather than put and left waiting"
    );
}

/// A rehearsal is the offer: it needs nobody at a terminal and puts no question.
#[tokio::test]
async fn a_rehearsal_asks_nobody_and_needs_no_terminal() {
    let mut rehearsing = ctx();
    rehearsing.dry_run = true;
    let nobody = Typing::nobody();
    let code = run(
        &rehearsing,
        Mending {
            fixing: Fixing {
                fix: true,
                yes: false,
                disruptive: false,
            },
            undo: false,
        },
        Arc::clone(&nobody) as Arc<dyn Asking>,
        false,
    )
    .await;

    assert_eq!(shown(code), success());
    assert!(nobody.asked().is_empty());
}

/// A rehearsal told to go ahead carries nothing out, and its report says it was one.
#[tokio::test]
async fn a_rehearsal_told_to_go_ahead_carries_nothing_out() {
    let mut rehearsing = ctx();
    rehearsing.dry_run = true;

    let outcome = repaired(&rehearsing, Consent::Standing, false, Typing::nobody()).await;

    let Ok(lemonfiber_core::app::Outcome::Repair(report)) = outcome else {
        unreachable!("a rehearsal answers with the repair report: {outcome:?}");
    };
    assert!(report.rehearsed);
    assert!(!report.acted);
}

async fn asked_for(yes: bool, json: bool) -> String {
    let (at, _root) = paths("offers");
    shown(
        run(
            &ctx_in(&at),
            Mending {
                fixing: Fixing {
                    fix: true,
                    yes,
                    disruptive: false,
                },
                undo: false,
            },
            Typing::answering("n"),
            json,
        )
        .await,
    )
}

/// An operator who asked for things to be put right and had one fail needs their
/// script to know, and a run with nothing to mend has mended everything it offered.
#[test]
fn anything_left_unmended_is_a_non_zero_result() {
    assert_eq!(shown(repairing(&report(Vec::new()))), success());
    assert_eq!(shown(repairing(&report(vec![Outcome::Fixed]))), success());
    assert_ne!(
        shown(repairing(&report(vec![Outcome::FixFailed]))),
        success()
    );
    assert_ne!(
        shown(repairing(&report(vec![Outcome::Declined]))),
        success()
    );
    assert_ne!(
        shown(repairing(&report(vec![Outcome::Fixed, Outcome::FixFailed]))),
        success()
    );
}

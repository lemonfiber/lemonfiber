use std::collections::BTreeMap;
use std::path::Path;

use super::{guarded, progress_at, resume, Applying};
use crate::alert::Appetite;
use crate::config::paths::{Paths, SECRETS};
use crate::config::{store, Protocols};
use crate::error::codes::setup::GUARD_UNANSWERED;
use crate::platform::Environment;
use crate::stack::Source;
use crate::wizard::{Answer, Guarded, Library, Phase, Progress, Step, Vpn, Wizard};
use lemonfiber_fixtures::ports::STAMPED;

/// What an apply writes with: a real directory, a stack already on disk, and a real
/// machine's randomness, which is what the key the journal's credentials are sealed
/// under is made from.
fn applying(paths: &Paths) -> Applying<'_> {
    Applying {
        paths,
        source: Source::External(Path::new("/lemonfiber-not-a-real-stack")),
        stamp: STAMPED,
        random: &A_MACHINE,
        confined: &lemonfiber_adapters::Disk,
        guard: None,
    }
}

/// The randomness a real machine supplies.
static A_MACHINE: lemonfiber_fixtures::ports::Chance =
    lemonfiber_fixtures::ports::Chance::cycling();

/// A scratch directory unique to this process and case, cleared first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

fn layout(dir: &Path) -> Paths {
    Paths::rooted(&dir.join("config"), &dir.join("data"))
}

/// What the guard is answered with to dial its tunnel, assembled so no credential sits
/// here.
fn dialling() -> String {
    ["wireguard", "not", "real"].join("-")
}

/// A wizard with every question answered for a machine running both protocols.
fn answered(dir: &Path) -> Wizard {
    let mut wizard = Wizard::new(Environment::LinuxNative);
    for answer in [
        Answer::Protocols(Protocols::both()),
        Answer::Vpn(Vpn::Carrying(Guarded::default())),
        Answer::DataLocation(dir.join("data-root")),
        Answer::Credentials(None),
        Answer::Provider(None),
        Answer::ServiceUser(Some((1000, 1000))),
        Answer::Library(Library::Docker),
        Answer::Household(true),
        Answer::Notifications(Appetite::default_appetite()),
        Answer::Autostart(false),
    ] {
        wizard.answer(answer).unwrap_or(());
    }
    wizard
}

#[test]
fn progress_reads_back_what_a_run_saved() {
    let dir = scratch("progress");
    let path = dir.join("setup-progress.json");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let saved = Progress {
        phase: Phase::Applying,
        ..Progress::default()
    };
    let text = serde_json::to_string(&saved).unwrap_or_default();
    assert!(std::fs::write(&path, text).is_ok());

    assert_eq!(progress_at(&path), Some(saved));
}

#[test]
fn no_progress_file_reads_as_nothing_to_resume() {
    assert_eq!(
        progress_at(Path::new("/lemonfiber/no/such/progress.json")),
        None
    );
}

#[test]
fn a_torn_progress_file_reads_as_nothing_rather_than_failing() {
    let dir = scratch("torn-progress");
    let path = dir.join("setup-progress.json");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    assert!(std::fs::write(&path, "{ half a wr").is_ok());

    assert_eq!(progress_at(&path), None);
}

#[test]
fn resume_carries_an_interrupted_apply_forward_from_its_answers() {
    let dir = scratch("resume");
    let paths = layout(&dir);
    let mut wizard = answered(&dir);
    assert!(wizard.transition(Phase::Reviewing));
    assert!(wizard.transition(Phase::Applying));

    assert!(resume(&mut wizard, &applying(&paths)).is_ok());

    assert_eq!(wizard.phase(), Phase::Applied);
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
}

#[test]
fn a_vpn_answer_the_guard_no_longer_takes_is_taken_back_and_asked_again() {
    let guard = super::guarding::tests::guard();
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let _ = wizard.answer(Answer::Protocols(Protocols {
        usenet: false,
        torrent: true,
    }));
    let _ = wizard.answer(Answer::Vpn(Vpn::Carrying(Guarded::default())));

    let refused = guarded(&mut wizard, Some(&guard));

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(GUARD_UNANSWERED)
    );
    assert_eq!(wizard.answers().vpn, None);
    assert_eq!(wizard.at(), Step::Vpn);
}

#[test]
fn an_apply_with_no_guard_to_hold_a_vpn_answer_to_writes_none_of_it() {
    let dir = scratch("guard-unheld");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::MacOs);
    for answer in [
        Answer::Protocols(Protocols {
            usenet: false,
            torrent: true,
        }),
        Answer::Vpn(Vpn::Carrying(Guarded {
            settings: BTreeMap::new(),
            secrets: BTreeMap::from([("indexer:key".to_owned(), dialling())]),
        })),
        Answer::DataLocation(dir.join("media")),
        Answer::Credentials(None),
        Answer::Library(Library::None),
        Answer::Household(false),
        Answer::Notifications(Appetite::default_appetite()),
        Answer::Autostart(false),
    ] {
        let _ = wizard.answer(answer);
    }
    assert!(wizard.ready_for_review(), "{:?}", wizard.unanswered());

    let applied = resume(&mut wizard, &applying(&paths));

    assert_eq!(
        applied.err().map(|problem| problem.code),
        Some(GUARD_UNANSWERED)
    );
    assert!(!paths.env_file().with_file_name(SECRETS).exists());
    assert!(!paths.env_file().exists());
}

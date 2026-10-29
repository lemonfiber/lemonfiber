//! A terminal run's VPN step, asked question by question of the egress guard.

use std::collections::BTreeMap;

use super::*;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::error::codes::setup::GUARD_UNANSWERED;
use crate::wizard::Guarded;

/// What the guard is answered with to dial its tunnel, assembled so no credential sits
/// here.
fn dialling() -> String {
    ["wireguard", "not", "real"].join("-")
}

/// A prompt answering the guard's questions from `answers`, everything else workable.
fn answering(dir: &Path, answers: &[(&str, &str)]) -> Scripted {
    Scripted {
        protocols: Protocols {
            usenet: false,
            torrent: true,
        },
        guarding: answers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        ..Scripted::workable(dir.join("data-root"))
    }
}

/// What an apply over `paths` writes with, the guard the fixture declares among it.
fn guarded<'a>(paths: &'a Paths, guard: &'a super::super::Guard) -> Applying<'a> {
    Applying {
        guard: Some(guard),
        ..applying(paths, "t")
    }
}

/// The secret store an apply over `paths` keeps.
fn secrets(paths: &Paths) -> std::path::PathBuf {
    paths.env_file().with_file_name(SECRETS)
}

#[tokio::test]
async fn the_guard_is_asked_question_by_question_and_what_it_is_given_is_kept_where_it_belongs() {
    let dir = scratch("guard-walked");
    let paths = layout(&dir);
    let guard = super::super::guarding::tests::guard();
    let dialled = dialling();
    let prompt = answering(
        &dir,
        &[
            ("guard:vpn-type", "wireguard"),
            ("guard:provider", "ivpn"),
            ("guard:wireguard-key", &dialled),
        ],
    );
    let mut wizard = Wizard::new(Environment::LinuxNative);

    let outcome = run(
        &mut wizard,
        (&prompt, Some(&guard)),
        &ProbeFs::links(),
        &proving(),
        &guarded(&paths, &guard),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)), "{outcome:?}");
    assert_eq!(
        *prompt.asked_of_the_guard.borrow(),
        vec!["guard:vpn-type", "guard:provider", "guard:wireguard-key"]
    );
    let held = crate::settings::Secrets::at(&secrets(&paths));
    assert!(held.is_ok_and(|kept| kept.value("guard:wireguard-key") == Some(dialled.as_str())));
    let settings = crate::settings::Settings::at(&paths.env_file().with_file_name(SETTINGS));
    assert!(settings.is_ok_and(|kept| kept.value("guard:provider") == Some("ivpn")));
    assert!(!paths.setup_progress().exists());
}

#[tokio::test]
async fn a_run_leaving_the_guard_short_is_refused_with_nothing_of_it_kept() {
    let dir = scratch("guard-short");
    let paths = layout(&dir);
    let guard = super::super::guarding::tests::guard();
    let prompt = answering(&dir, &[("guard:vpn-type", "openvpn")]);
    let mut wizard = Wizard::new(Environment::LinuxNative);

    let outcome = run(
        &mut wizard,
        (&prompt, Some(&guard)),
        &ProbeFs::links(),
        &proving(),
        &guarded(&paths, &guard),
    )
    .await;

    assert_eq!(
        outcome.err().map(|problem| problem.code),
        Some(GUARD_UNANSWERED)
    );
    assert_eq!(wizard.answers().vpn, None);
    assert!(!secrets(&paths).exists());
}

#[tokio::test]
async fn a_resumed_run_whose_vpn_answer_the_guard_no_longer_takes_asks_it_again() {
    let dir = scratch("guard-resumed");
    let paths = layout(&dir);
    let guard = super::super::guarding::tests::guard();
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let _ = wizard.answer(Answer::Protocols(Protocols {
        usenet: false,
        torrent: true,
    }));
    let _ = wizard.answer(Answer::Vpn(Vpn::Carrying(Guarded::default())));
    let prompt = answering(&dir, &[("guard:wireguard-key", &dialling())]);

    let outcome = run(
        &mut wizard,
        (&prompt, Some(&guard)),
        &ProbeFs::links(),
        &proving(),
        &guarded(&paths, &guard),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)), "{outcome:?}");
    assert_eq!(
        *prompt.asked_of_the_guard.borrow(),
        vec!["guard:vpn-type", "guard:provider", "guard:wireguard-key"]
    );
}

#[tokio::test]
async fn an_apply_with_no_guard_to_hold_a_vpn_answer_to_writes_none_of_it() {
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

    let applied = super::super::resume(&mut wizard, &applying(&paths, "t"));

    assert_eq!(
        applied.err().map(|problem| problem.code),
        Some(GUARD_UNANSWERED)
    );
    assert!(!secrets(&paths).exists());
    assert!(!paths.env_file().exists());
}

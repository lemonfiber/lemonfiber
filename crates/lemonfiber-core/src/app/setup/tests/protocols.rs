//! The VPN question, the prerequisites, and confirming a run.

use super::*;

/// A torrent run that says it has a VPN is not warned about anything.
#[tokio::test]
async fn a_tunnelled_torrent_run_is_not_warned() {
    let dir = scratch("vpn-carried");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert_eq!(
        prompt.warned_unprotected.get(),
        0,
        "nothing is exposed, so there is nothing to warn about"
    );
    assert_eq!(wizard.answers().vpn, Some(Vpn::Carrying));
}

/// The requirement itself: torrents without a VPN are warned about, the
/// operator has to say so a second time, and the run goes on.
#[tokio::test]
async fn torrents_without_a_vpn_are_warned_and_confirmed_and_never_refused() {
    let dir = scratch("vpn-absent");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));
    prompt.vpn.borrow_mut().push_back(false);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(
        matches!(outcome, Ok(Outcome::Applied)),
        "a warning, never a refusal: {outcome:?}"
    );
    assert_eq!(
        prompt.warned_unprotected.get(),
        1,
        "the exposure was put to them"
    );
    assert_eq!(
        wizard.answers().vpn,
        Some(Vpn::Absent),
        "recorded as accepted, so a later diagnosis reads a decision not an oversight"
    );
}

/// Declining the warning returns to the question rather than ending setup —
/// the way out of the loop is always available, and it is not a refusal.
#[tokio::test]
async fn declining_the_exposure_asks_again_rather_than_stopping() {
    let dir = scratch("vpn-reconsidered");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));
    // No VPN, then "actually, no, do not go on" — and on the second pass they
    // say a VPN carries it after all.
    prompt.vpn.borrow_mut().extend([false, true]);
    prompt.unprotected.borrow_mut().push_back(false);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert_eq!(prompt.warned_unprotected.get(), 1);
    assert_eq!(wizard.answers().vpn, Some(Vpn::Carrying));
}

/// A Usenet-only run never meets the question: nothing about it is exposed to
/// a swarm, so asking would be a question with no consequence behind it.
#[tokio::test]
async fn a_usenet_only_run_is_never_asked_about_a_vpn() {
    let dir = scratch("vpn-usenet");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let mut prompt = Scripted::workable(dir.join("data-root"));
    prompt.protocols = Protocols {
        usenet: true,
        torrent: false,
    };
    // Scripted to answer "no VPN" — which must never be reached at all.
    prompt.vpn.borrow_mut().push_back(false);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert_eq!(prompt.warned_unprotected.get(), 0);
    assert_eq!(wizard.answers().vpn, None, "the step did not apply");
}

#[tokio::test]
async fn the_prerequisites_are_shown_derived_from_the_chosen_protocols() {
    let dir = scratch("prereqs");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));

    assert!(matches!(
        run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &proving(),
            &applying(&paths, "t")
        )
        .await,
        Ok(Outcome::Applied)
    ));

    // The checklist was shown once, derived from the protocols answered — not a
    // fixed list, and not before the protocols were chosen.
    let shown = prompt.shown_prerequisites.borrow();
    assert_eq!(shown.as_slice(), [Protocols::both()]);
}

#[tokio::test]
async fn a_confirmed_run_gathers_the_answers_and_applies_them() {
    let dir = scratch("applied");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
    assert_eq!(
        file.get("PUID"),
        Some("1000"),
        "the container user was asked"
    );
    // Gathering saved progress — including the answers — and a finished apply
    // removes that copy: the secrets it held live only in the .env now.
    assert!(
        !paths.setup_progress().exists(),
        "the resumable progress file is gone once setup is applied"
    );
    // A location whose own filesystem links is taken as chosen, and the good
    // result is shown directly — not inferred from a parent.
    assert_eq!(
        prompt.hardlinked.borrow().as_slice(),
        [(dir.join("data-root"), false)]
    );
}

#[tokio::test]
async fn a_run_the_operator_does_not_confirm_applies_nothing() {
    let dir = scratch("abandoned");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        confirm: false,
        ..Scripted::workable(dir.join("data-root"))
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Abandoned)));
    assert!(!paths.env_file().exists(), "nothing was written");
}

#[tokio::test]
async fn where_the_container_user_does_not_apply_it_is_not_asked() {
    let dir = scratch("macos");
    let paths = layout(&dir);
    // On macOS ownership is mapped away, so the container-user question does not
    // apply and is passed over — a run still reaches applied without it.
    let mut wizard = Wizard::new(Environment::MacOs);
    let prompt = Scripted::workable(dir.join("data-root"));

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("PUID"), None, "no container user was written");
}

#[tokio::test]
async fn gathering_saves_progress_so_a_quit_run_can_resume() {
    let dir = scratch("gather-save");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // Declining at review stops before apply, so the file on disk is what
    // gathering saved: every answer, still gathering.
    let prompt = Scripted {
        confirm: false,
        ..Scripted::workable(dir.join("data-root"))
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;
    assert!(matches!(outcome, Ok(Outcome::Abandoned)));

    // A wizard resumed from what was saved needs no more questions — every
    // answer survived the quit.
    let resumed = progress_at(&paths.setup_progress())
        .map(|progress| Wizard::resume(Environment::LinuxNative, progress));
    assert_eq!(
        resumed.map(|wizard| wizard.ready_for_review()),
        Some(true),
        "the saved progress resumes to a complete set of answers",
    );
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
    // A wizard left at applying with a complete set of answers — the state a
    // failed apply persists — is carried forward to applied.
    let mut wizard = Wizard::new(Environment::LinuxNative);
    for answer in [
        Answer::Protocols(Protocols::both()),
        Answer::Vpn(Vpn::Carrying),
        Answer::DataLocation(dir.join("data-root")),
        Answer::Credentials(None),
        Answer::Provider(None),
        Answer::ServiceUser(Some((1000, 1000))),
        Answer::Library(Library::JellyfinDocker),
        Answer::Household(true),
        Answer::Notifications(Appetite::default_appetite()),
        Answer::Autostart(false),
    ] {
        wizard.answer(answer).unwrap_or(());
    }
    assert!(wizard.transition(Phase::Reviewing));
    assert!(wizard.transition(Phase::Applying));

    assert!(super::super::resume(&mut wizard, &applying(&paths, "t")).is_ok());

    assert_eq!(wizard.phase(), Phase::Applied);
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
}

#[tokio::test]
async fn a_wizard_past_gathering_is_refused_rather_than_re_applied() {
    let dir = scratch("underway");
    let paths = layout(&dir);
    // A wizard already reviewed is past gathering. Running setup on it must
    // refuse — driving it again would apply over the journal a recovery reads —
    // rather than treat it as a fresh run.
    let mut wizard = Wizard::new(Environment::LinuxNative);
    for answer in [
        Answer::Protocols(Protocols::both()),
        Answer::Vpn(Vpn::Carrying),
        Answer::DataLocation(dir.join("data-root")),
        Answer::Credentials(None),
        Answer::Provider(None),
        Answer::ServiceUser(Some((1000, 1000))),
        Answer::Library(Library::JellyfinDocker),
        Answer::Household(true),
        Answer::Notifications(Appetite::default_appetite()),
        Answer::Autostart(false),
    ] {
        wizard.answer(answer).unwrap_or(());
    }
    assert!(
        wizard.transition(Phase::Reviewing),
        "the wizard reaches review"
    );
    let prompt = Scripted::workable(dir.join("data-root"));

    let refused = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(refused, Err(problem) if problem.code == super::super::ALREADY_UNDERWAY));
}

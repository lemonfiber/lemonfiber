//! Applying the answers, and the ways out of an apply that stopped.

use super::*;

/// What an interrupted apply leaves behind: half-written settings, the marker
/// saying the writing had begun, and the journal of what it managed to write.
fn interrupted(paths: &Paths) {
    assert!(store::write(&paths.env_file(), "DATA_ROOT=/srv\n").is_ok());
    let stopped = Progress {
        phase: Phase::Applying,
        ..Progress::default()
    };
    assert!(store::write(
        &paths.setup_progress(),
        &serde_json::to_string(&stopped).unwrap_or_default()
    )
    .is_ok());
    let written = Change {
        at: "1".to_owned(),
        operation: "setup".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: "DATA_ROOT".to_owned(),
            previous: None,
            current: "/srv".to_owned(),
        },
    };
    assert!(store::write(
        &paths.journal(),
        &serde_json::to_string(&written).unwrap_or_default()
    )
    .is_ok());
}

#[tokio::test]
async fn applying_before_every_question_is_answered_is_refused() {
    let (_scratch, paths) = scratch("early");

    assert_eq!(
        refused(&ctx(&paths), SetupAction::Apply).await,
        Some(NOT_REVIEWED)
    );
    assert!(!paths.env_file().exists(), "and nothing was written");
}

#[tokio::test]
async fn a_complete_set_of_answers_is_written_and_setup_stops_being_offered() {
    let (_scratch, paths) = scratch("applied");
    let context = ctx(&paths);
    let root = paths.data_dir().join("media");
    answer_everything(&context, &root).await;

    let report = walked(&context, SetupAction::Apply).await;

    assert_eq!(
        report.as_ref().map(|report| report.phase),
        Some(Phase::Applied)
    );
    assert_eq!(
        report.map(|report| report.offered),
        Some(false),
        "this machine is set up now"
    );
    assert!(paths.env_file().exists(), "the settings landed");
    assert!(root.is_dir(), "and so did the library's home");
    assert!(
        !paths.setup_progress().exists(),
        "the resumable copy of the answers is not left lying about"
    );
}

#[tokio::test]
async fn a_machine_already_set_up_is_told_so_rather_than_asked_again() {
    let (_scratch, paths) = scratch("configured");
    let context = ctx(&paths);
    assert!(store::write(&paths.env_file(), "DATA_ROOT=/srv\n").is_ok());

    assert_eq!(
        refused(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await,
        Some(ALREADY_SET_UP)
    );
    // Asking is still answered: whether setup is on offer is exactly what a
    // surface asks this to decide whether to show the wizard at all.
    assert_eq!(
        walked(&context, SetupAction::Where)
            .await
            .map(|report| report.offered),
        Some(false)
    );
}

#[tokio::test]
async fn an_apply_that_stopped_part_way_is_picked_up_rather_than_read_as_finished() {
    let (_scratch, paths) = scratch("interrupted");
    interrupted(&paths);

    let report = walked(&ctx(&paths), SetupAction::Where).await;

    assert_eq!(
        report.as_ref().map(|report| report.offered),
        Some(true),
        "a half-written apply is not a finished install"
    );
    assert_eq!(
        report.as_ref().map(|report| report.phase),
        Some(Phase::Applying),
        "and it says which of the two it is"
    );
    assert_eq!(
        report.map(|report| report.written),
        Some(vec!["the setting DATA_ROOT".to_owned()]),
        "and names what it had already written, so a choice is made about it"
    );
}

#[tokio::test]
async fn starting_over_undoes_what_was_written_and_forgets_the_answers() {
    let (_scratch, paths) = scratch("start-over");
    interrupted(&paths);

    let report = walked(&ctx(&paths), SetupAction::Recover(Choice::StartOver)).await;

    assert_eq!(
        report.as_ref().map(|report| report.at),
        Some(Step::Welcome),
        "the walk is back at its beginning"
    );
    assert_eq!(
        report.map(|report| report.written.len()),
        Some(0),
        "and there is nothing left to choose about"
    );
    assert!(!paths.setup_progress().exists(), "the answers are gone");
    assert!(
        !paths.journal().exists(),
        "and so is the record of the apply"
    );
    assert_eq!(
        store::read(&paths.env_file())
            .ok()
            .and_then(|settings| settings.get("DATA_ROOT").map(ToOwned::to_owned)),
        None,
        "and the setting it had written is off the file"
    );
}

#[tokio::test]
async fn rolling_back_undoes_what_was_written_and_applies_the_answers_again() {
    let (_scratch, paths) = scratch("roll-back");
    let context = ctx(&paths);
    let root = paths.data_dir().join("media");
    answer_everything(&context, &root).await;
    // An apply that stopped after writing one setting, over the answers just
    // gathered — which is the only state a roll back is offered from.
    let saved = crate::app::setup::progress_at(&paths.setup_progress());
    let mut progress = saved.unwrap_or_default();
    progress.phase = Phase::Applying;
    assert!(store::write(
        &paths.setup_progress(),
        &serde_json::to_string(&progress).unwrap_or_default()
    )
    .is_ok());

    let report = walked(&context, SetupAction::Recover(Choice::RollBack)).await;

    assert_eq!(
        report.map(|report| report.phase),
        Some(Phase::Applied),
        "the answers were applied again"
    );
    assert!(paths.env_file().exists(), "and the settings landed");
}

#[tokio::test]
async fn resuming_carries_the_apply_forward_from_the_answers_it_kept() {
    let (_scratch, paths) = scratch("resume-recovery");
    let context = ctx(&paths);
    let root = paths.data_dir().join("media");
    answer_everything(&context, &root).await;
    let saved = crate::app::setup::progress_at(&paths.setup_progress());
    let mut progress = saved.unwrap_or_default();
    progress.phase = Phase::Applying;
    assert!(store::write(
        &paths.setup_progress(),
        &serde_json::to_string(&progress).unwrap_or_default()
    )
    .is_ok());

    let report = walked(&context, SetupAction::Recover(Choice::Resume)).await;

    assert_eq!(report.map(|report| report.phase), Some(Phase::Applied));
    assert!(paths.env_file().exists());
}

/// A rehearsed way out of a half-written apply leaves it half-written, and still
/// names what the choice is being made about.
///
/// The refusal above it is the half a rehearsal keeps, because whether an apply
/// stopped part-way is a fact about the machine rather than a consequence of
/// acting. What it leaves out is the reversal and the apply behind it — so the
/// setting the interrupted run wrote is still on the file, the record of it is
/// still in the journal, and the answers are still where they were. An operator who
/// asked what starting over would do has not started over, and the list they were
/// shown is the same list the real choice will be made from.
#[tokio::test]
async fn a_rehearsed_way_out_of_a_half_written_apply_leaves_it_half_written() {
    let (_scratch, paths) = scratch("rehearsed-recover");
    interrupted(&paths);

    let report = walked(
        &ctx(&paths).rehearsing(),
        SetupAction::Recover(Choice::StartOver),
    )
    .await;

    assert_eq!(
        report.as_ref().map(|report| report.phase),
        Some(Phase::Applying),
        "a rehearsal carried the recovery out"
    );
    assert_eq!(
        report.map(|report| report.written),
        Some(vec!["the setting DATA_ROOT".to_owned()]),
        "and it no longer says what the choice is being made about"
    );
    assert!(
        paths.journal().exists(),
        "the record of the half-written apply was discarded by a question"
    );
    assert_eq!(
        store::read(&paths.env_file())
            .ok()
            .and_then(|settings| settings.get("DATA_ROOT").map(ToOwned::to_owned)),
        Some("/srv".to_owned()),
        "the setting the interrupted apply had written was taken off the file"
    );
}

#[tokio::test]
async fn a_way_out_of_an_apply_that_never_stopped_is_refused() {
    let (_scratch, paths) = scratch("nothing-to-recover");
    let context = ctx(&paths);
    assert!(setup(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    assert_eq!(
        refused(&context, SetupAction::Recover(Choice::StartOver)).await,
        Some(NOTHING_TO_RECOVER)
    );
    assert!(
        paths.setup_progress().exists(),
        "and the answers it would have forgotten are still there"
    );
}

#[tokio::test]
async fn nowhere_to_keep_configuration_is_said_rather_than_guessed_at() {
    let nowhere = a_context().settings(Settings::default()).build();

    assert!(
        setup(&nowhere, SetupAction::Where).await.is_err(),
        "a run with no configured home has nowhere to gather answers into"
    );
}

#[tokio::test]
async fn a_rehearsed_apply_reports_the_plan_and_writes_none_of_it() {
    // The report is the review: every setting an apply would write, in the words
    // it would write them in, and no file of it on disk.
    let (_scratch, paths) = scratch("rehearsed-apply");
    let context = ctx(&paths);
    let root = paths.data_dir().join("media");
    answer_everything(&context, &root).await;

    let report = walked(&ctx(&paths).rehearsing(), SetupAction::Apply).await;

    assert_eq!(
        report.as_ref().map(|report| report.phase),
        Some(Phase::Reviewing),
        "a rehearsal reaches review and stops at it"
    );
    assert_eq!(
        planned(report.as_ref(), "DATA_ROOT"),
        Some(root.display().to_string()),
        "a rehearsal naming nothing it would write has printed no invocation"
    );
    assert!(
        !paths.env_file().exists(),
        "the settings an apply writes were written"
    );
    assert!(
        !paths.autostart().exists() && !paths.notifications().exists(),
        "the answers an apply keeps beside the settings were kept"
    );
    assert!(
        paths.setup_progress().exists(),
        "a finished apply clears the progress, and this one had not applied"
    );
}

#[tokio::test]
async fn a_rehearsed_apply_is_refused_before_review_the_way_a_real_one_is() {
    let (_scratch, paths) = scratch("rehearsed-early");
    let rehearsing = ctx(&paths).rehearsing();
    assert!(setup(
        &rehearsing,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    assert_eq!(
        refused(&rehearsing, SetupAction::Apply).await,
        Some(NOT_REVIEWED),
        "a rehearsal judging this for itself would be a second judgement to keep true"
    );
}

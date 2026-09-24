//! Walking the questions: which apply, in what order, and resuming.

use super::*;

#[test]
fn a_recorded_mode_round_trips_back_to_the_library_choice() {
    for library in [Library::JellyfinDocker, Library::JellyfinNative] {
        let mode = library.mode().unwrap_or_default();
        assert_eq!(Library::from_mode(mode), Some(library));
    }
    // No media server writes no mode, and an unrecognised value stands for
    // nothing rather than being guessed at.
    assert_eq!(Library::None.mode(), None);
    assert_eq!(Library::from_mode("elsewhere"), None);
}

#[test]
fn a_fresh_wizard_starts_at_welcome_with_nothing_answered() {
    let wizard = on_native_linux();
    assert_eq!(wizard.at(), Step::Welcome);
    assert_eq!(wizard.answers(), &super::super::Answers::default());
    assert_eq!(wizard.progress(), &Progress::default());
}

#[test]
fn setup_is_offered_only_when_nothing_is_configured() {
    assert!(offer_setup(false));
    assert!(!offer_setup(true));
}

#[test]
fn only_the_question_steps_report_as_questions() {
    for step in [
        Step::Protocols,
        Step::DataLocation,
        Step::ServiceUser,
        Step::Library,
        Step::Household,
        Step::Autostart,
    ] {
        assert!(step.is_question(), "{step:?} asks something");
    }
    for step in [
        Step::Welcome,
        Step::Preflight,
        Step::Prerequisites,
        Step::Review,
    ] {
        assert!(!step.is_question(), "{step:?} only informs");
    }
}

#[test]
fn the_container_user_is_asked_only_where_ownership_is_real() {
    assert!(on_native_linux().applies(Step::ServiceUser));
    assert!(!on_macos().applies(Step::ServiceUser));
    // Every other step applies regardless of platform.
    for step in [Step::Welcome, Step::Protocols, Step::Library, Step::Review] {
        assert!(on_macos().applies(step));
        assert!(on_native_linux().applies(step));
    }
}

#[test]
fn advancing_walks_the_steps_in_order() {
    let mut wizard = on_native_linux();
    // A download protocol is chosen so the credentials step applies and the walk
    // covers every step; without one it is passed over, as its own test proves.
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    let mut visited = vec![wizard.at()];
    while let Some(step) = wizard.advance() {
        visited.push(step);
    }
    assert_eq!(visited, Step::ORDER.to_vec());
    // Advancing from the last step goes nowhere.
    assert_eq!(wizard.at(), Step::Review);
    assert_eq!(wizard.advance(), None);
}

#[test]
fn advancing_skips_a_step_that_does_not_apply() {
    // On macOS the container-user step is skipped: data location goes straight
    // to library.
    let mut wizard = on_macos();
    wizard.progress.at = Step::DataLocation;
    assert_eq!(wizard.advance(), Some(Step::Library));
}

#[test]
fn going_back_walks_the_steps_in_reverse_and_stops_at_welcome() {
    let mut wizard = on_native_linux();
    // A protocol is chosen so the credentials step applies both ways.
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    while wizard.advance().is_some() {}
    assert_eq!(wizard.at(), Step::Review);
    let mut seen = vec![wizard.at()];
    while let Some(step) = wizard.back() {
        seen.push(step);
    }
    let mut expected = Step::ORDER.to_vec();
    expected.reverse();
    assert_eq!(seen, expected);
    assert_eq!(wizard.at(), Step::Welcome);
    assert_eq!(wizard.back(), None);
}

#[test]
fn going_back_skips_a_step_that_does_not_apply() {
    let mut wizard = on_macos();
    wizard.progress.at = Step::Library;
    assert_eq!(wizard.back(), Some(Step::DataLocation));
}

#[test]
fn an_answer_is_recorded_against_its_field() {
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard
        .answer(Answer::DataLocation(PathBuf::from("/srv/media")))
        .unwrap_or(());
    wizard
        .answer(Answer::ServiceUser(Some((1000, 1001))))
        .unwrap_or(());
    wizard
        .answer(Answer::Library(Library::JellyfinDocker))
        .unwrap_or(());
    wizard.answer(Answer::Household(true)).unwrap_or(());
    wizard
        .answer(Answer::Notifications(Appetite::default_appetite()))
        .unwrap_or(());
    wizard.answer(Answer::Autostart(false)).unwrap_or(());

    let answers = wizard.answers();
    assert_eq!(answers.protocols, Some(Protocols::both()));
    assert_eq!(answers.data_location, Some(PathBuf::from("/srv/media")));
    assert_eq!(answers.service_user, Some(Some((1000, 1001))));
    assert_eq!(answers.library, Some(Library::JellyfinDocker));
    assert_eq!(answers.household, Some(true));
    assert_eq!(answers.autostart, Some(false));
}

#[test]
fn a_container_user_is_refused_where_ownership_is_mapped_away() {
    let mut wizard = on_macos();
    assert_eq!(
        wizard.answer(Answer::ServiceUser(Some((1000, 1000)))),
        Err(super::super::Rejected::ServiceUserNotApplicable)
    );
    // But declining one is always fine — there is nothing to map.
    assert_eq!(wizard.answer(Answer::ServiceUser(None)), Ok(()));
    assert_eq!(wizard.answers().service_user, Some(None));
}

#[test]
fn native_jellyfin_is_refused_where_it_buys_nothing() {
    let mut linux = on_native_linux();
    assert_eq!(
        linux.answer(Answer::Library(Library::JellyfinNative)),
        Err(super::super::Rejected::NativeJellyfinUnavailable)
    );
    assert_eq!(linux.answers().library, None);

    // Where it is offered, it is accepted.
    let mut macos = on_macos();
    assert_eq!(
        macos.answer(Answer::Library(Library::JellyfinNative)),
        Ok(())
    );
    assert_eq!(macos.answers().library, Some(Library::JellyfinNative));
}

#[test]
fn the_unanswered_questions_shrink_as_they_are_answered() {
    let mut wizard = on_native_linux();
    assert_eq!(
        wizard.unanswered(),
        vec![
            Step::Protocols,
            Step::DataLocation,
            Step::ServiceUser,
            Step::Library,
            Step::Household,
            Step::Notifications,
            Step::Autostart,
        ]
    );
    assert!(!wizard.ready_for_review());
    answer_all(&mut wizard);
    assert!(wizard.unanswered().is_empty());
    assert!(wizard.ready_for_review());
}

#[test]
fn a_step_that_does_not_apply_is_not_among_the_unanswered() {
    // macOS never asks for the container user, so it is absent from the list
    // and does not hold review up.
    let wizard = on_macos();
    assert!(!wizard.unanswered().contains(&Step::ServiceUser));
    assert!(wizard.is_answered(Step::ServiceUser));
}

#[test]
fn is_answered_tracks_each_step() {
    let mut wizard = on_native_linux();
    // Informing steps are always "answered": nothing to hold up.
    for step in [
        Step::Welcome,
        Step::Preflight,
        Step::Prerequisites,
        Step::Review,
    ] {
        assert!(wizard.is_answered(step));
    }
    // Questions start unanswered.
    for step in [
        Step::Protocols,
        Step::DataLocation,
        Step::ServiceUser,
        Step::Library,
        Step::Household,
        Step::Autostart,
    ] {
        assert!(!wizard.is_answered(step));
    }
    answer_all(&mut wizard);
    for step in Step::ORDER {
        assert!(wizard.is_answered(step), "{step:?} should be answered");
    }
}

#[test]
fn progress_survives_a_round_trip_so_setup_can_resume() {
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard.advance();
    let saved = serde_json::to_string(wizard.progress()).unwrap_or_default();

    let restored: Progress = serde_json::from_str(&saved).unwrap_or_default();
    let resumed = Wizard::resume(Environment::LinuxNative, restored);
    assert_eq!(resumed.progress(), wizard.progress());
    assert_eq!(resumed.at(), wizard.at());
    assert_eq!(resumed.answers(), wizard.answers());
}

#[test]
fn resuming_where_native_jellyfin_is_no_longer_offered_clears_it() {
    // Chosen on macOS, resumed on Linux, where the container transcodes and
    // native mode buys nothing: the choice this machine rejects must not be
    // carried into what would be written.
    let mut macos = on_macos();
    macos
        .answer(Answer::Library(Library::JellyfinNative))
        .unwrap_or(());
    let saved = macos.progress().clone();

    let resumed = Wizard::resume(Environment::LinuxNative, saved);
    assert_eq!(resumed.answers().library, None);
    // The question returns, to be answered afresh where it now applies.
    assert!(resumed.unanswered().contains(&Step::Library));
}

#[test]
fn resuming_where_ownership_is_mapped_away_drops_the_container_user() {
    // A concrete uid/gid chosen on native Linux is meaningless once resumed on
    // macOS, so it is dropped rather than applied.
    let mut linux = on_native_linux();
    linux
        .answer(Answer::ServiceUser(Some((1000, 1000))))
        .unwrap_or(());
    let saved = linux.progress().clone();

    let resumed = Wizard::resume(Environment::MacOs, saved);
    assert_eq!(resumed.answers().service_user, None);
    // macOS never asks it, so it does not come back as unanswered either.
    assert!(!resumed.unanswered().contains(&Step::ServiceUser));
}

#[test]
fn resuming_re_homes_a_cursor_left_on_a_skipped_step() {
    // The cursor was on the container-user step on native Linux; macOS does
    // not present it, so a resumed run moves to the next step it does rather
    // than opening on a question it skips.
    let mut linux = on_native_linux();
    linux.progress.at = Step::ServiceUser;
    let saved = linux.progress().clone();

    let resumed = Wizard::resume(Environment::MacOs, saved);
    assert_eq!(resumed.at(), Step::Library);
    assert!(resumed.applies(resumed.at()));
}

#[test]
fn the_serialised_step_and_answers_read_as_their_kebab_names() {
    let mut wizard = on_macos();
    wizard
        .answer(Answer::Library(Library::JellyfinNative))
        .unwrap_or(());
    wizard.progress.at = Step::DataLocation;
    let json = serde_json::to_string(wizard.progress()).unwrap_or_default();
    assert!(json.contains(r#""at":"data-location""#), "{json}");
    assert!(json.contains(r#""library":"jellyfin-native""#), "{json}");
}

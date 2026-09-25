use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::support::Scripted;

use super::{at_login, AutostartCheck, Standing, ENGINE_NOT_AT_BOOT, SETTINGS};
use crate::doctor::{Category, Check, Verdict};
use crate::error::Problem;
use crate::platform::Environment;
use crate::ports::process::{Failure, Output};
use crate::ports::{FileSystem, Runner};

/// Where a test pretends this operator's home directory is.
fn home() -> PathBuf {
    PathBuf::from("/home/op")
}

/// The location a test writes the setting to when it does not care which.
///
/// Written out rather than taken off the list by position, so the list can be
/// reordered without quietly changing what these tests are about — and asserted
/// to be on it, so it cannot drift off the list either.
const SOMEWHERE: &str = "Library/Group Containers/group.com.docker/settings-store.json";

/// And the one Docker Desktop uses on Linux, for the test that is about platforms.
const ON_LINUX: &str = ".docker/desktop/settings-store.json";

#[test]
fn the_places_these_tests_write_to_are_places_the_check_looks() {
    assert!(SETTINGS.contains(&SOMEWHERE));
    assert!(SETTINGS.contains(&ON_LINUX));
}

/// A runner answering every program with the given output.
fn saying(stdout: &str) -> Arc<dyn Runner> {
    Arc::new(Scripted(Ok(Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    })))
}

/// A filesystem holding Docker Desktop's settings at exactly that candidate.
fn holding(candidate: &str, text: &str) -> Arc<dyn FileSystem> {
    Files::at(vec![(home().join(candidate), text)])
}

/// A filesystem with nothing in it at all.
fn bare() -> Arc<dyn FileSystem> {
    Files::empty()
}

/// The one verdict a check produces.
async fn verdict(check: AutostartCheck) -> Option<Verdict> {
    let found = check.run().await;
    assert_eq!(found.len(), 1, "one finding, about one thing");
    found.into_iter().next().map(|finding| {
        assert_eq!(finding.category, Category::Environment);
        finding.verdict
    })
}

/// A check over Docker Desktop, with the given filesystem underneath it.
fn desktop(filesystem: Arc<dyn FileSystem>) -> AutostartCheck {
    AutostartCheck::new(
        filesystem,
        saying(""),
        Environment::MacOs,
        true,
        Some(home()),
    )
}

/// The words a verdict says, whichever kind it is.
///
/// Total over the five kinds and over the absence of one, because eight
/// assertions below read the answer through it: a reader that fell through on a
/// kind it did not recognise would hand every one of them an empty string, and
/// `assert!(said(..).contains(..))` on an empty string fails in a way that reads
/// as the check having said the wrong thing rather than as the reader having a
/// hole in it.
fn said(verdict: Option<Verdict>) -> String {
    match verdict {
        Some(Verdict::Pass { note }) => note.unwrap_or_default(),
        Some(Verdict::Skipped { reason } | Verdict::Unverified { reason, .. }) => reason,
        Some(Verdict::Warn(problem) | Verdict::Fail(problem)) => problem.summary,
        None => String::new(),
    }
}

/// The fault a verdict carries, where it is a warning about one.
///
/// Beside [`said`] rather than inside the one test that reads a fault out, so the
/// other arm is exercised by the tests about the readings that are *not* faults.
/// That is the distinction this whole check exists for: a setting nobody could
/// read is `enabled-unverified` and carries no fault, and a reader that folded it
/// into one would report a machine nothing is known about as a machine that is
/// wrong — the same falsehood as a pass, in the other direction.
fn warned(verdict: Option<Verdict>) -> Option<Problem> {
    match verdict {
        Some(Verdict::Warn(problem)) => Some(problem),
        _ => None,
    }
}

#[tokio::test]
async fn a_machine_nobody_asked_to_start_on_boot_is_not_held_to_anything() {
    // Skipped rather than unverified: it is not that nobody could find out, it is
    // that the question does not apply.
    let check = AutostartCheck::new(bare(), saying(""), Environment::MacOs, false, Some(home()));
    let verdict = verdict(check).await;
    assert!(matches!(verdict, Some(Verdict::Skipped { .. })));
    assert!(said(verdict).contains(Standing::Disabled.named()));
}

#[tokio::test]
async fn docker_desktop_set_to_open_at_login_is_confirmed_rather_than_assumed() {
    // Every location the setting has ever lived in, because it moved between
    // Docker Desktop versions and sits somewhere different on each platform.
    for candidate in SETTINGS {
        let verdict = verdict(desktop(holding(candidate, r#"{"autoStart": true}"#))).await;
        assert!(
            matches!(verdict, Some(Verdict::Pass { .. })),
            "the setting at {candidate} was not read"
        );
        assert!(said(verdict).contains(Standing::Enabled.named()));
    }
}

#[tokio::test]
async fn the_settings_store_spells_the_key_differently_and_is_read_too() {
    // The key is `autoStart` in the old settings file and `AutoStart` in the
    // store that replaced it. A reader that knew one spelling would report a
    // machine set up correctly as unverified.
    let check = desktop(holding(SOMEWHERE, r#"{"AutoStart": true, "Other": 1}"#));
    assert!(matches!(verdict(check).await, Some(Verdict::Pass { .. })));
}

#[tokio::test]
async fn docker_desktop_that_does_not_open_at_login_is_named_as_the_cause() {
    // The single most common reason a stack does not come back, and the one the
    // operator cannot see: nothing errors, nothing is logged, it is just gone.
    let problem = warned(verdict(desktop(holding(SOMEWHERE, r#"{"autoStart": false}"#))).await);
    assert_eq!(
        problem.as_ref().map(|problem| problem.code),
        Some(ENGINE_NOT_AT_BOOT)
    );
    assert!(
        problem
            .as_ref()
            .is_some_and(|problem| problem.meaning.contains("no notification")),
        "it says why nobody would notice"
    );
    assert!(
        problem.is_some_and(|problem| problem
            .remedies
            .first()
            .is_some_and(|remedy| remedy.action.contains("Docker Desktop"))),
        "and instructs, since this is not a setting lemonfiber can write"
    );
}

#[tokio::test]
async fn a_setting_nobody_could_read_is_unverified_and_says_the_word_for_it() {
    // The state the whole feature turns on: configured on our side, unproven on
    // the other. It must never render as a pass.
    for text in [
        r#"{"somethingElse": true}"#,
        "not json at all",
        r#"{"autoStart": "yes"}"#,
    ] {
        let verdict = verdict(desktop(holding(SOMEWHERE, text))).await;
        assert!(
            matches!(verdict, Some(Verdict::Unverified { .. })),
            "{text} was not treated as unreadable"
        );
        assert!(said(verdict).contains(Standing::EnabledUnverified.named()));
    }
}

#[tokio::test]
async fn a_machine_with_no_settings_file_at_all_is_unverified_rather_than_blamed() {
    assert!(matches!(
        verdict(desktop(bare())).await,
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_machine_that_will_not_say_where_home_is_cannot_confirm_anything() {
    let check = AutostartCheck::new(bare(), saying(""), Environment::MacOs, true, None);
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Unverified { .. })
    ));
}

/// A check over native Linux, where the service manager is what is asked.
fn native(stdout: &str) -> AutostartCheck {
    AutostartCheck::new(
        bare(),
        saying(stdout),
        Environment::LinuxNative,
        true,
        Some(home()),
    )
}

#[tokio::test]
async fn a_daemon_the_distribution_enabled_at_boot_is_the_whole_of_it_on_linux() {
    assert!(matches!(
        verdict(native("enabled\n")).await,
        Some(Verdict::Pass { .. })
    ));
}

#[tokio::test]
async fn a_daemon_enabled_only_until_the_next_restart_is_not_enabled_at_boot() {
    // The arrangement somebody believes is permanent and is not — the same belief
    // this check exists to take away, in a different costume.
    let verdict = verdict(native("enabled-runtime\n")).await;
    assert!(matches!(verdict, Some(Verdict::Warn(_))));
    assert!(said(verdict).contains("until the next restart"));
}

#[tokio::test]
async fn a_daemon_that_is_not_enabled_at_boot_is_reported() {
    for answer in ["disabled", "masked", "masked-runtime"] {
        assert!(
            matches!(verdict(native(answer)).await, Some(Verdict::Warn(_))),
            "{answer} should be reported"
        );
    }
}

#[tokio::test]
async fn a_service_manager_that_answered_something_else_leaves_it_unverified() {
    // Never a pass. An answer nobody here recognises says nothing about whether
    // the daemon starts, and guessing would be the comfortable falsehood.
    let verdict = verdict(native("static\n")).await;
    assert!(matches!(verdict, Some(Verdict::Unverified { .. })));
    assert!(said(verdict).contains("static"));
}

#[tokio::test]
async fn a_service_manager_that_said_nothing_is_quoted_as_having_said_nothing() {
    let verdict = verdict(native("   ")).await;
    assert!(matches!(verdict, Some(Verdict::Unverified { .. })));
    assert!(said(verdict).contains("nothing at all"));
}

#[tokio::test]
async fn a_service_manager_that_could_not_be_run_leaves_it_unverified() {
    let refused: Arc<dyn Runner> = Arc::new(Scripted(Err(Failure::NotFound {
        program: "systemctl".to_owned(),
    })));
    let check = AutostartCheck::new(
        bare(),
        refused,
        Environment::LinuxNative,
        true,
        Some(home()),
    );
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_platform_lemonfiber_does_not_support_says_so_rather_than_guessing() {
    let check = AutostartCheck::new(
        bare(),
        saying("enabled"),
        Environment::Unsupported,
        true,
        Some(home()),
    );
    let verdict = verdict(check).await;
    assert!(matches!(verdict, Some(Verdict::Unverified { .. })));
    assert!(said(verdict).contains("does not know how this platform"));
}

#[tokio::test]
async fn docker_desktop_on_linux_and_on_windows_is_asked_the_same_question() {
    // Three environments, one prerequisite: the daemon is Docker Desktop and its
    // open-at-login setting is the load-bearing half. A check that asked the
    // service manager on a Linux machine running Desktop would be asking about a
    // service that is not what starts the engine there.
    for environment in [
        Environment::LinuxDesktop,
        Environment::Windows,
        Environment::MacOs,
    ] {
        let check = AutostartCheck::new(
            holding(ON_LINUX, r#"{"autoStart": false}"#),
            saying("enabled"),
            environment,
            true,
            Some(home()),
        );
        assert!(
            matches!(verdict(check).await, Some(Verdict::Warn(_))),
            "{environment:?} asked the wrong thing"
        );
    }
}

/// A reading short of a warning names no fault to go and act on.
///
/// `enabled-unverified` is the state this whole check exists to give, and it is
/// worth something only while it stays apart from the other two. A reader that
/// found a fault in a pass would be the comfortable falsehood; one that found a
/// fault in a reading nobody could confirm is the same falsehood pointed the
/// other way, and it is the one an operator would act on — going to change a
/// setting that may already be right, on the strength of a machine that never
/// said so.
#[tokio::test]
async fn a_reading_short_of_a_warning_names_no_fault_to_act_on() {
    assert!(
        warned(verdict(native("disabled")).await).is_some(),
        "a daemon that is not enabled at boot is a fault, and carries the one it is"
    );
    assert!(
        warned(verdict(native("enabled\n")).await).is_none(),
        "a daemon the distribution already enabled is not"
    );
    assert!(
        warned(verdict(native("static\n")).await).is_none(),
        "and an answer nobody here recognises has established nothing either way"
    );
}

/// A check that reported nothing at all has nothing to say.
///
/// Every check in this file reports exactly one finding, so this is the answer to
/// a question that does not arise — and it is answered rather than left to fall
/// through, because the reader it belongs to is what every assertion about the
/// words of a verdict goes through. A hole in it would turn a check that said
/// nothing into a check that said the wrong thing, and the failure would name the
/// check rather than the reader that lost its words.
#[test]
fn a_check_that_reported_nothing_at_all_has_nothing_to_say() {
    assert!(said(None).is_empty());
}

#[test]
fn the_three_states_are_spelled_the_way_the_feature_names_them() {
    assert_eq!(Standing::Disabled.named(), "disabled");
    assert_eq!(Standing::Enabled.named(), "enabled");
    assert_eq!(Standing::EnabledUnverified.named(), "enabled-unverified");
}

#[test]
fn only_a_boolean_answer_counts_as_an_answer() {
    assert_eq!(at_login(r#"{"autoStart": true}"#), Some(true));
    assert_eq!(at_login(r#"{"AUTOSTART": false}"#), Some(false));
    assert_eq!(at_login(r#"{"autoStart": 1}"#), None);
    assert_eq!(at_login("[]"), None);
    assert_eq!(at_login(""), None);
}

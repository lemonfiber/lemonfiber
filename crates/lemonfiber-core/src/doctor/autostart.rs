//! Whether the stack would actually come back, rather than whether it was asked to.
//!
//! "Start on boot" is three different arrangements wearing one name. On Linux with
//! Docker Engine the distribution has already enabled the daemon at boot, so the
//! containers' own restart policies are the whole of it. On macOS and Windows the
//! daemon is Docker Desktop, which **does not open at login by default** — so the
//! machine restarts after an overnight update and nothing comes back, with no error
//! and nothing in any log the operator would think to look at.
//!
//! That setting belongs to Docker Desktop. lemonfiber can read it and cannot write
//! it, and the whole point of this check is that the difference is stated rather than
//! papered over: where the prerequisite can be confirmed it is confirmed, where it is
//! confirmed absent it is named as the single most common cause of this failure, and
//! where it can be neither it is reported as `enabled-unverified` — configured on our
//! side, unproven on the other.
//!
//! **The unverified verdict is the load-bearing one.** An operator who believes they
//! have autostart and does not is worse off than one who knows they have none: the
//! first finds out weeks later, from a household asking why nothing has downloaded
//! since Tuesday. A "could not check" that rendered as a pass would produce exactly
//! that belief, which is why it is a verdict of its own here rather than a severity.
//!
//! Reading the setting is a read of a file Docker Desktop wrote, in the operator's
//! own home directory, and every candidate location is tried rather than one chosen
//! from the build target — the setting has moved between Docker Desktop versions and
//! sits in a different place on each platform, and a check that knew only one of them
//! would report `enabled-unverified` on a machine where the answer was sitting on
//! disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::platform::Environment;
use crate::ports::{FileSystem, Runner};

pub(crate) use crate::error::codes::env::ENGINE_NOT_AT_BOOT;

/// The check this reports under, named once so a finding and an answer to it cannot
/// drift apart on a rename.
///
/// Public because one caller has to be able to *leave it out*. Setup's preflight asks
/// this whole family and stops before a single question where the answer is broken or
/// undetermined — which is right about an engine that cannot be reached and wrong
/// about a Docker Desktop setting that could not be read, since the second says
/// nothing about whether this machine can run the stack today. A name it can match on
/// is the honest way to say that; the alternative is this check quietly never
/// answering `enabled-unverified`, which is the one answer it exists to give.
pub const CHECK: &str = "environment.autostart";

/// What the operator reads on the line above the verdict.
const TITLE: &str = "The stack comes back after a restart";

/// What the operator asked for, weighed against what the platform can confirm.
///
/// Three of the five states the feature names. `degraded` and `failed-boot` are
/// about a boot that has already happened and are not this check's to give: nothing
/// here watches a start, so claiming either would be claiming to have seen something
/// this never looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Nothing starts automatically, because nobody asked for it to.
    Disabled,
    /// Asked for, and the platform prerequisite is confirmed present.
    Enabled,
    /// Asked for, and a platform prerequisite is not confirmed.
    EnabledUnverified,
}

impl Standing {
    /// The word the feature names this state by.
    ///
    /// Hyphenated because these are the words an operator reads and the requirement
    /// writes, and a second spelling would be a second vocabulary for one set of
    /// facts.
    #[must_use]
    pub const fn named(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::EnabledUnverified => "enabled-unverified",
        }
    }
}

/// What was found out about the prerequisite this platform's autostart rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prerequisite {
    /// It is in place, and that was established rather than assumed.
    InPlace {
        /// What was read, in the words the operator can go and check themselves.
        evidence: String,
    },
    /// It is confirmed absent, which is the common cause of this whole failure.
    Absent {
        /// What is not set, and where it is set.
        what: String,
        /// How to set it, since this is not something lemonfiber can write.
        how: String,
    },
    /// It could not be established either way.
    Unconfirmed {
        /// Why not, in terms the operator can act on.
        why: String,
        /// What to do about the uncertainty.
        how: String,
    },
}

/// Whether the stack would actually come back after a restart of this machine.
pub struct AutostartCheck {
    filesystem: Arc<dyn FileSystem>,
    runner: Arc<dyn Runner>,
    environment: Environment,
    wanted: bool,
    home: Option<PathBuf>,
}

impl AutostartCheck {
    /// A check over the operator's answer and this machine's own arrangements.
    ///
    /// `wanted` is the answer as it was given rather than anything derived from it:
    /// what this check adds is the other half, and a check that decided for itself
    /// whether autostart had been asked for would be deciding the thing it reports.
    #[must_use]
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        runner: Arc<dyn Runner>,
        environment: Environment,
        wanted: bool,
        home: Option<PathBuf>,
    ) -> Self {
        Self {
            filesystem,
            runner,
            environment,
            wanted,
            home,
        }
    }
}

#[async_trait]
impl Check for AutostartCheck {
    fn category(&self) -> Category {
        Category::Environment
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// What this machine would do at its next restart, and what is known about it.
async fn ran(check: &AutostartCheck) -> Vec<Finding> {
    if !check.wanted {
        return vec![Finding::in_category(
            Category::Environment,
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: format!(
                    "{}: starting on boot was not asked for, so nothing here is expected to \
                     bring the stack back",
                    Standing::Disabled.named()
                ),
            },
        )];
    }
    vec![Finding::in_category(
        Category::Environment,
        CHECK,
        TITLE,
        verdict(prerequisite(check).await),
    )]
}

/// The verdict for what was found out about the prerequisite.
///
/// Three findings for three genuinely different situations, and the middle one is
/// the reason the check exists at all: a prerequisite nobody could read is not a
/// prerequisite that is in place, and reporting it as one would manufacture exactly
/// the belief this feature is written to prevent.
fn verdict(found: Prerequisite) -> Verdict {
    match found {
        Prerequisite::InPlace { evidence } => Verdict::Pass {
            note: Some(format!("{}: {evidence}", Standing::Enabled.named())),
        },
        Prerequisite::Absent { what, how } => Verdict::Warn(
            Problem::new(
                ENGINE_NOT_AT_BOOT,
                Severity::Warning,
                what,
                "The containers carry a restart policy, and a restart policy only brings a \
                 container back once the engine behind it is running. Nothing starts the engine, \
                 so after the next restart of this machine the stack is simply not there — no \
                 error, no notification, and nothing in any log you would think to look at.",
                Remedy::new(how),
            )
            .in_state(State::Guided),
        ),
        Prerequisite::Unconfirmed { why, how } => Verdict::Unverified {
            reason: format!(
                "{}: {why}, so whether the stack would come back is not established",
                Standing::EnabledUnverified.named()
            ),
            remedy: Remedy::new(how),
        },
    }
}

/// What this platform's autostart actually rests on, and whether it is there.
async fn prerequisite(check: &AutostartCheck) -> Prerequisite {
    match check.environment {
        // The distribution enabled the daemon at boot when it installed it, so the
        // only thing left to establish is that it is still enabled.
        Environment::LinuxNative => daemon_enabled(check).await,
        Environment::MacOs | Environment::LinuxDesktop | Environment::Windows => {
            desktop_at_login(check).await
        }
        Environment::Unsupported => Prerequisite::Unconfirmed {
            why: "lemonfiber does not know how this platform starts anything at login".to_owned(),
            how: "Arrange the container engine to start at login with whatever this system uses, \
                  then start the stack once by hand to confirm it comes back"
                .to_owned(),
        },
    }
}

/// The command that asks whether the service manager starts the daemon at boot.
///
/// The system service rather than a user one, because that is where a distribution
/// installs it — and asking does not need administrative rights, which is what makes
/// it a check rather than a prompt.
fn is_enabled() -> Vec<String> {
    ["systemctl", "is-enabled", "docker"]
        .map(str::to_owned)
        .to_vec()
}

/// Whether the Docker service is enabled at boot, as the service manager says.
async fn daemon_enabled(check: &AutostartCheck) -> Prerequisite {
    let Ok(output) = check.runner.run(&is_enabled()).await else {
        return Prerequisite::Unconfirmed {
            why: "the service manager could not be asked whether the Docker service starts at \
                  boot"
                .to_owned(),
            how: "Run `systemctl is-enabled docker` yourself to see what it says".to_owned(),
        };
    };
    match output.stdout.trim() {
        // Enabled by a unit file, or by a runtime link that lasts until the next
        // boot. The second is not the same promise as the first, and is named rather
        // than folded in, because a link that disappears at the next boot is exactly
        // the arrangement somebody believes is permanent.
        "enabled" => Prerequisite::InPlace {
            evidence: "the Docker service is enabled at boot, so the containers' restart \
                       policies bring the stack back"
                .to_owned(),
        },
        "enabled-runtime" => Prerequisite::Absent {
            what: "The Docker service is enabled only until the next restart".to_owned(),
            how: "Run `sudo systemctl enable docker` so it survives a restart".to_owned(),
        },
        "disabled" | "masked" | "masked-runtime" => Prerequisite::Absent {
            what: "The Docker service is not enabled at boot".to_owned(),
            how: "Run `sudo systemctl enable docker`, then restart this machine to confirm the \
                  stack comes back"
                .to_owned(),
        },
        said => Prerequisite::Unconfirmed {
            why: format!(
                "the service manager answered {} when asked whether Docker starts at boot",
                quoted(said)
            ),
            how: "Run `systemctl is-enabled docker` yourself to see what it says".to_owned(),
        },
    }
}

/// What the service manager said, or a word for its having said nothing.
///
/// Written out rather than interpolated bare, because an empty answer rendered into
/// a sentence leaves a gap the operator reads as a missing word rather than as the
/// finding itself.
fn quoted(said: &str) -> String {
    if said.is_empty() {
        return "nothing at all".to_owned();
    }
    format!("`{said}`")
}

/// Where Docker Desktop keeps the setting, relative to the operator's home.
///
/// Every candidate rather than the one this build's platform uses, and all of them
/// tried: the file moved between Docker Desktop versions and sits somewhere
/// different on each platform, so a check that knew one location would answer
/// `enabled-unverified` on a machine where the answer was on disk the whole time.
const SETTINGS: [&str; 6] = [
    "Library/Group Containers/group.com.docker/settings-store.json",
    "Library/Group Containers/group.com.docker/settings.json",
    ".docker/desktop/settings-store.json",
    ".docker/desktop/settings.json",
    "AppData/Roaming/Docker/settings-store.json",
    "AppData/Roaming/Docker/settings.json",
];

/// Whether Docker Desktop is set to open at login, as its own settings file says.
async fn desktop_at_login(check: &AutostartCheck) -> Prerequisite {
    let Some(home) = check.home.as_deref() else {
        return Prerequisite::Unconfirmed {
            why: "this machine would not say where your home directory is, so Docker Desktop's \
                  own settings could not be found"
                .to_owned(),
            how: TURN_IT_ON.to_owned(),
        };
    };
    match read_setting(check, home).await {
        Some(true) => Prerequisite::InPlace {
            evidence: "Docker Desktop is set to open at login, so the engine is running by the \
                       time the containers' restart policies are read"
                .to_owned(),
        },
        Some(false) => Prerequisite::Absent {
            what: "Docker Desktop is not set to open at login".to_owned(),
            how: TURN_IT_ON.to_owned(),
        },
        None => Prerequisite::Unconfirmed {
            why: "Docker Desktop's own settings could not be read, and that setting is its own \
                  rather than something lemonfiber writes"
                .to_owned(),
            how: TURN_IT_ON.to_owned(),
        },
    }
}

/// How to set the one thing lemonfiber cannot set.
///
/// Written once, because it is said in all three of the answers that are not a
/// confirmed yes — and an operator who meets it twice should meet the same sentence.
const TURN_IT_ON: &str = "Open Docker Desktop, and under Settings → General turn on \
                          \"Start Docker Desktop when you sign in\"";

/// Read the setting out of whichever settings file this machine has.
async fn read_setting(check: &AutostartCheck, home: &Path) -> Option<bool> {
    for candidate in SETTINGS {
        let answer = check
            .filesystem
            .read(&home.join(candidate))
            .await
            .as_deref()
            .and_then(at_login);
        if answer.is_some() {
            return answer;
        }
    }
    None
}

/// Whether a Docker Desktop settings document says it opens at login.
///
/// Matched without regard to case, because the key is `autoStart` in the settings
/// file and `AutoStart` in the settings store that replaced it, and a reader that
/// knew one spelling would report the other as unreadable.
fn at_login(text: &str) -> Option<bool> {
    let document: serde_json::Value = serde_json::from_str(text).ok()?;
    document
        .as_object()?
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("autostart"))
        .and_then(|(_, value)| value.as_bool())
}

#[cfg(test)]
mod tests {
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
        let check =
            AutostartCheck::new(bare(), saying(""), Environment::MacOs, false, Some(home()));
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
}

//! Driving the setup wizard to a working configuration.
//!
//! The wizard decides what to ask; a [`Prompt`] answers it, and this walks the two
//! together — asking each question the wizard presents here, in order, until every
//! one is answered, then applying what was gathered once the operator confirms it.
//!
//! The asking is a port, so the whole walk is driven in a test by a prompt that
//! answers from a script, with no terminal; a real one reads a line and renders.
//! Starting the stack and recovering an interrupted apply join this once there is
//! a surface to show their progress.

use std::path::{Path, PathBuf};

use crate::app::apply;
use crate::config::paths::Paths;
use crate::config::Protocols;
use crate::error::{Code, Problem, Remedy, Severity};
use crate::stack::Source;
use crate::wizard::{Answer, Library, Phase, Plan, Progress, Rejected, Step, Wizard};

/// How the operator is asked the questions the wizard cannot answer itself.
///
/// One method per question the wizard asks. A platform-aware implementation offers
/// only the choices that apply where it runs, so an answer the wizard would reject
/// is never gathered. Faked in tests; a real one reads and renders.
pub trait Prompt {
    /// Which download protocols to use: Usenet, torrents, both, or neither.
    fn protocols(&self) -> Protocols;
    /// Where the library and downloads are kept.
    fn data_location(&self) -> PathBuf;
    /// The user and group the containers run as, asked only where ownership shows;
    /// `None` where the operator declines and the image's own default is kept.
    fn service_user(&self) -> Option<(u32, u32)>;
    /// Whether to run Jellyfin, and how.
    fn library(&self) -> Library;
    /// Whether others in the home will use it.
    fn household(&self) -> bool;
    /// Whether the stack starts on boot.
    fn autostart(&self) -> bool;
    /// Whether the operator, shown the plan, confirms it.
    fn confirm(&self, plan: &Plan) -> bool;
}

/// How a run of setup ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The reviewed answers were applied.
    Applied,
    /// The operator saw the plan and chose not to apply it.
    Abandoned,
}

/// Gather answers through `prompt`, and — if the operator confirms the plan they
/// make — apply them.
///
/// # Errors
///
/// Returns a [`Problem`] where the wizard is past gathering (a run to review,
/// apply, or recover is not this), where an answer does not apply on this platform
/// (a prompt that offered a choice the wizard rejects), or where applying the
/// confirmed answers fails — the marker left for recovery in that case.
pub fn run(
    wizard: &mut Wizard,
    prompt: &dyn Prompt,
    paths: &Paths,
    source: Source,
    stamp: &str,
) -> Result<Outcome, Box<Problem>> {
    // Only a wizard still gathering is driven here. One that has been reviewed,
    // is part-way through an interrupted apply, or is already applied must be
    // routed by its caller — resumed, recovered, or pointed at reconfiguration —
    // because re-applying it would write over the very journal a recovery reads.
    if wizard.phase() != Phase::InProgress {
        return Err(Box::new(already_underway()));
    }
    gather(wizard, prompt).map_err(|rejected| Box::new(does_not_apply(rejected)))?;

    if !prompt.confirm(&wizard.plan()) {
        return Ok(Outcome::Abandoned);
    }

    // A gathering wizard whose questions are all answered moves into review, and
    // from there apply carries the lifecycle on. The two preconditions the move
    // needs — the phase and a complete set of answers — are the guard above and
    // gathering itself, so it always takes here.
    wizard.transition(Phase::Reviewing);
    apply::apply(wizard, paths, source, stamp)?;
    Ok(Outcome::Applied)
}

/// The setup progress saved at `path`, or nothing where none is there or it does
/// not read.
///
/// What a later run reads to tell where a previous one got to — which the wizard's
/// `Status` classifies, and which a resumed apply is restored from. Absence and an
/// unreadable or unparsable file are the same "no progress to resume from" answer,
/// not a fault: a fresh machine has none, and a torn one is better begun again.
#[must_use]
pub fn progress_at(path: &Path) -> Option<Progress> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Re-apply a setup whose apply was interrupted, from the answers it recorded.
///
/// A failed apply leaves the wizard restored to `applying` with every answer
/// intact — apply persists them before it writes. Rolling that one step back to
/// review is the wizard's single backward edge, and from review apply carries it
/// forward again. Apply is idempotent, so this either finishes what the
/// interrupted run started or leaves the same recoverable marker to try again.
///
/// # Errors
///
/// Returns a [`Problem`] where applying the recorded answers fails, leaving the
/// marker at `applying` for another attempt.
pub fn resume(
    wizard: &mut Wizard,
    paths: &Paths,
    source: Source,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    wizard.transition(Phase::Reviewing);
    apply::apply(wizard, paths, source, stamp)
}

/// Ask each question the wizard presents here and has no answer for yet, in order.
///
/// A question that does not apply on this platform, or that a resumed run already
/// answered, is passed over rather than asked again.
fn gather(wizard: &mut Wizard, prompt: &dyn Prompt) -> Result<(), Rejected> {
    // The answers to ask for are chosen first, then recorded — so which questions
    // apply is read off the wizard before any answer changes it, and the recording
    // is one plain loop rather than a `?` tucked inside each conditional.
    let mut answers = Vec::new();
    if wants(wizard, Step::Protocols) {
        answers.push(Answer::Protocols(prompt.protocols()));
    }
    if wants(wizard, Step::DataLocation) {
        answers.push(Answer::DataLocation(prompt.data_location()));
    }
    if wants(wizard, Step::ServiceUser) {
        answers.push(Answer::ServiceUser(prompt.service_user()));
    }
    if wants(wizard, Step::Library) {
        answers.push(Answer::Library(prompt.library()));
    }
    if wants(wizard, Step::Household) {
        answers.push(Answer::Household(prompt.household()));
    }
    if wants(wizard, Step::Autostart) {
        answers.push(Answer::Autostart(prompt.autostart()));
    }

    for answer in answers {
        wizard.answer(answer)?;
    }
    Ok(())
}

/// Whether a question applies here and is still unanswered — one to ask.
fn wants(wizard: &Wizard, step: Step) -> bool {
    wizard.applies(step) && !wizard.is_answered(step)
}

/// The problem of an answer that does not apply where setup is running.
///
/// Setup only asks what applies here, so a rejection means a prompt offered a
/// choice it should not have; the rejected answer names which in the detail.
fn does_not_apply(rejected: Rejected) -> Problem {
    Problem::new(
        DOES_NOT_APPLY,
        Severity::Error,
        "An answer does not apply on this platform",
        "Setup only asks what applies where it runs, so this should not be reachable through its own prompts. Nothing has been applied.",
        Remedy::new("Answer with a choice this platform offers"),
    )
    .with_detail(format!("{rejected:?}"))
}

/// Raised when an answer is not meaningful on the platform setup is running on.
pub const DOES_NOT_APPLY: Code = Code::new("SETUP-5");

/// The problem of running setup on a wizard that is no longer gathering answers.
fn already_underway() -> Problem {
    Problem::new(
        ALREADY_UNDERWAY,
        Severity::Error,
        "Setup is past the point of gathering answers",
        "This wizard has already been reviewed, is part-way through applying, or is finished, so running it again here would write over what a recovery needs. Nothing has been changed.",
        Remedy::new("Resume or recover the setup in progress, or reconfigure a finished one"),
    )
}

/// Raised when setup is asked to gather answers for a wizard already past it.
pub const ALREADY_UNDERWAY: Code = Code::new("SETUP-6");

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{progress_at, run, Outcome, Prompt};
    use crate::config::paths::Paths;
    use crate::config::{store, Protocols};
    use crate::platform::Environment;
    use crate::stack::Source;
    use crate::wizard::{Answer, Library, Phase, Plan, Progress, Wizard};

    /// A prompt that answers from a fixed script, so a run is driven with no
    /// terminal.
    struct Scripted {
        protocols: Protocols,
        data_location: PathBuf,
        service_user: Option<(u32, u32)>,
        library: Library,
        household: bool,
        autostart: bool,
        confirm: bool,
    }

    impl Scripted {
        /// A script that answers every question with a workable choice and confirms.
        fn workable(data_location: PathBuf) -> Self {
            Self {
                protocols: Protocols::both(),
                data_location,
                service_user: Some((1000, 1000)),
                library: Library::JellyfinDocker,
                household: true,
                autostart: false,
                confirm: true,
            }
        }
    }

    impl Prompt for Scripted {
        fn protocols(&self) -> Protocols {
            self.protocols
        }
        fn data_location(&self) -> PathBuf {
            self.data_location.clone()
        }
        fn service_user(&self) -> Option<(u32, u32)> {
            self.service_user
        }
        fn library(&self) -> Library {
            self.library
        }
        fn household(&self) -> bool {
            self.household
        }
        fn autostart(&self) -> bool {
            self.autostart
        }
        fn confirm(&self, _plan: &Plan) -> bool {
            self.confirm
        }
    }

    /// A scratch directory unique to this process and case, cleared first.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-setup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn layout(dir: &Path) -> Paths {
        Paths::rooted(&dir.join("config"), &dir.join("data"))
    }

    /// A stack already on disk, so a run does not materialise one.
    fn external() -> Source {
        Source::External(Path::new("/lemonfiber-not-a-real-stack"))
    }

    #[test]
    fn a_confirmed_run_gathers_the_answers_and_applies_them() {
        let dir = scratch("applied");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted::workable(dir.join("data-root"));

        let outcome = run(&mut wizard, &prompt, &paths, external(), "t");

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
        assert_eq!(
            file.get("PUID"),
            Some("1000"),
            "the container user was asked"
        );
    }

    #[test]
    fn a_run_the_operator_does_not_confirm_applies_nothing() {
        let dir = scratch("abandoned");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            confirm: false,
            ..Scripted::workable(dir.join("data-root"))
        };

        let outcome = run(&mut wizard, &prompt, &paths, external(), "t");

        assert!(matches!(outcome, Ok(Outcome::Abandoned)));
        assert!(!paths.env_file().exists(), "nothing was written");
    }

    #[test]
    fn where_the_container_user_does_not_apply_it_is_not_asked() {
        let dir = scratch("macos");
        let paths = layout(&dir);
        // On macOS ownership is mapped away, so the container-user question does not
        // apply and is passed over — a run still reaches applied without it.
        let mut wizard = Wizard::new(Environment::MacOs);
        let prompt = Scripted::workable(dir.join("data-root"));

        let outcome = run(&mut wizard, &prompt, &paths, external(), "t");

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("PUID"), None, "no container user was written");
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
            Answer::DataLocation(dir.join("data-root")),
            Answer::ServiceUser(Some((1000, 1000))),
            Answer::Library(Library::JellyfinDocker),
            Answer::Household(true),
            Answer::Autostart(false),
        ] {
            wizard.answer(answer).unwrap_or(());
        }
        assert!(wizard.transition(Phase::Reviewing));
        assert!(wizard.transition(Phase::Applying));

        assert!(super::resume(&mut wizard, &paths, external(), "t").is_ok());

        assert_eq!(wizard.phase(), Phase::Applied);
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
    }

    #[test]
    fn a_wizard_past_gathering_is_refused_rather_than_re_applied() {
        let dir = scratch("underway");
        let paths = layout(&dir);
        // A wizard already reviewed is past gathering. Running setup on it must
        // refuse — driving it again would apply over the journal a recovery reads —
        // rather than treat it as a fresh run.
        let mut wizard = Wizard::new(Environment::LinuxNative);
        for answer in [
            Answer::Protocols(Protocols::both()),
            Answer::DataLocation(dir.join("data-root")),
            Answer::ServiceUser(Some((1000, 1000))),
            Answer::Library(Library::JellyfinDocker),
            Answer::Household(true),
            Answer::Autostart(false),
        ] {
            wizard.answer(answer).unwrap_or(());
        }
        assert!(
            wizard.transition(Phase::Reviewing),
            "the wizard reaches review"
        );
        let prompt = Scripted::workable(dir.join("data-root"));

        let refused = run(&mut wizard, &prompt, &paths, external(), "t");

        assert!(matches!(refused, Err(problem) if problem.code == super::ALREADY_UNDERWAY));
    }

    #[test]
    fn an_answer_that_does_not_apply_here_stops_the_run() {
        let dir = scratch("rejected");
        let paths = layout(&dir);
        // Native Jellyfin buys nothing on native Linux, so a prompt that offers it
        // anyway is rejected rather than applied.
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            library: Library::JellyfinNative,
            ..Scripted::workable(dir.join("data-root"))
        };

        let stopped = run(&mut wizard, &prompt, &paths, external(), "t");

        assert!(matches!(stopped, Err(problem) if problem.code == super::DOES_NOT_APPLY));
        assert!(!paths.env_file().exists(), "nothing was applied");
    }
}

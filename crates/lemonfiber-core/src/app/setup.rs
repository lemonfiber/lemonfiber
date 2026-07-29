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
use crate::config::{store, Protocols};
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;
use crate::prerequisites::{prerequisites, PrerequisiteMap};
use crate::stack::Source;
use crate::storage::{self, Linked};
use crate::wizard::{Answer, Library, Phase, Plan, Progress, Rejected, Step, Wizard};

/// How the operator is asked the questions the wizard cannot answer itself.
///
/// One method per question the wizard asks. A platform-aware implementation offers
/// only the choices that apply where it runs, so an answer the wizard would reject
/// is never gathered. Faked in tests; a real one reads and renders.
pub trait Prompt {
    /// Which download protocols to use: Usenet, torrents, both, or neither.
    fn protocols(&self) -> Protocols;
    /// Show the operator the accounts their protocol choice needs, before any is
    /// asked for — derived from what they chose, and nothing they declined.
    fn prerequisites(&self, map: &PrerequisiteMap);
    /// Where the library and downloads are kept.
    fn data_location(&self) -> PathBuf;
    /// The chosen location was just tested and it hardlinks — report the good
    /// result, so the operator sees the test happen rather than a silent pause.
    /// `inferred_from` is the parent the test actually ran against where the
    /// location did not exist yet, so an answer proven on a parent is shown as
    /// inferred rather than as the chosen path's own proven capability.
    fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>);
    /// The chosen location cannot be used for instant, space-free imports: state
    /// what was found and its consequence, and ask whether to use it anyway
    /// (`true`) or name another location (`false`). The storage *mode* is never
    /// asked — it follows from what the location can do; only the location is the
    /// operator's to choose.
    fn storage_warning(&self, path: &Path, warning: &StorageWarning) -> bool;
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

/// Why a chosen data location is less than ideal, put to the operator so the
/// decision to use it anyway is theirs and informed — never a silent downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageWarning {
    /// The location works, but cannot hardlink: imports will copy. `limitation`
    /// names the filesystem reason where lemonfiber can state it — exFAT, a
    /// network share — and is absent where the type explains nothing.
    CopyOnly {
        /// The named reason the filesystem cannot link, where there is one.
        limitation: Option<String>,
    },
    /// The location could not be tested for hardlinks at all — it is not there
    /// yet and neither is any parent, or it could not be written — with the
    /// platform's own words for why.
    Untested {
        /// Why the test could not be run.
        reason: String,
    },
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
pub async fn run(
    wizard: &mut Wizard,
    prompt: &dyn Prompt,
    filesystem: &dyn FileSystem,
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
    gather(wizard, prompt, filesystem, paths)
        .await
        .map_err(|rejected| Box::new(does_not_apply(rejected)))?;

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
/// How one question is put to a prompt: the answer it gives for that step.
///
/// The data location is asked apart from the rest ([`How::Location`]) because its
/// answer is not merely read: the chosen place is tested for hardlinks, and a
/// place that cannot link is put back to the operator to accept or replace before
/// it is recorded.
enum How {
    /// Read the answer straight from the prompt.
    Sync(fn(&dyn Prompt) -> Answer),
    /// Ask for a data location and prove what it can do before recording it.
    Location,
}

async fn gather(
    wizard: &mut Wizard,
    prompt: &dyn Prompt,
    filesystem: &dyn FileSystem,
    paths: &Paths,
) -> Result<(), Rejected> {
    // Each question is paired with how to ask it, so the walk is one loop: ask,
    // record, and save before moving on, so quitting mid-setup resumes at the
    // question reached rather than restarting. One loop keeps the recording a
    // single `?` rather than one tucked inside each conditional.
    let questions: [(Step, How); 6] = [
        (
            Step::Protocols,
            How::Sync(|prompt| Answer::Protocols(prompt.protocols())),
        ),
        (Step::DataLocation, How::Location),
        (
            Step::ServiceUser,
            How::Sync(|prompt| Answer::ServiceUser(prompt.service_user())),
        ),
        (
            Step::Library,
            How::Sync(|prompt| Answer::Library(prompt.library())),
        ),
        (
            Step::Household,
            How::Sync(|prompt| Answer::Household(prompt.household())),
        ),
        (
            Step::Autostart,
            How::Sync(|prompt| Answer::Autostart(prompt.autostart())),
        ),
    ];

    for (step, how) in questions {
        if !wants(wizard, step) {
            continue;
        }
        let answer = match how {
            How::Sync(ask) => ask(prompt),
            How::Location => Answer::DataLocation(resolve_location(prompt, filesystem).await),
        };

        // The accounts a protocol needs are shown the moment the protocol is
        // chosen — derived from that answer, ahead of the questions that follow —
        // so the operator learns what they must go and obtain while there is still
        // a run to come back to. Matched off the answer's own variant, so the
        // choice's value is in hand without reaching back for an optional field.
        if let Answer::Protocols(protocols) = &answer {
            prompt.prerequisites(&prerequisites(*protocols));
        }

        wizard.answer(answer)?;
        save(wizard, paths);
    }
    Ok(())
}

/// Ask for a data location and test what it can do, until one is settled on.
///
/// A location that hardlinks is taken as chosen; one that cannot — or one that
/// could not be tested — is put back to the operator with what was found, to use
/// anyway or replace. The loop ends only when they accept a location, so it can
/// never wedge: the way out is always to say yes to the one in hand.
async fn resolve_location(prompt: &dyn Prompt, filesystem: &dyn FileSystem) -> PathBuf {
    loop {
        let chosen = prompt.data_location();
        match assess(filesystem, &chosen).await {
            Assessment::Links { inferred_from } => {
                prompt.hardlinks(&chosen, inferred_from.as_deref());
                return chosen;
            }
            Assessment::Warned(warning) => {
                if prompt.storage_warning(&chosen, &warning) {
                    return chosen;
                }
            }
        }
    }
}

/// What testing a prospective data location for hardlinks came to.
enum Assessment {
    /// The location hardlinks. `inferred_from` is the ancestor the test actually
    /// ran against where the location itself did not exist yet to be tested — so a
    /// result read off a parent is never presented as if the chosen path were
    /// proven. `None` where the chosen path was tested directly.
    Links { inferred_from: Option<PathBuf> },
    /// The location is usable only with a caveat the operator must weigh.
    Warned(StorageWarning),
}

/// Test a location for hardlinks — empirically, never inferred from its name.
///
/// The location itself is not created here: nothing setup does before the
/// operator confirms the plan touches disk beyond the resumable progress file, so
/// where the chosen path does not exist yet its filesystem is tested through the
/// deepest parent that does. That parent's answer is only a proxy — a separate
/// drive mounted there later could differ — so a result read off a parent is
/// carried back as such rather than dressed up as the chosen path's own. A place
/// with no reachable parent cannot be tested at all, and says so.
async fn assess(filesystem: &dyn FileSystem, chosen: &Path) -> Assessment {
    let Some((base, exact)) = nearest_existing(filesystem, chosen).await else {
        return Assessment::Warned(StorageWarning::Untested {
            reason: "it could not be reached, and neither could any parent of it".to_owned(),
        });
    };
    // A parent tested in the location's place is the inferred case; the location
    // itself, where it already exists, is a direct result.
    let inferred_from = (!exact).then(|| base.clone());

    match storage::test_link(filesystem, &base).await {
        Linked::Yes { .. } => Assessment::Links { inferred_from },
        Linked::No => {
            let facts = filesystem.describe(&base).await;
            Assessment::Warned(StorageWarning::CopyOnly {
                limitation: facts.kind.limitation().map(str::to_owned),
            })
        }
        Linked::Unwritable { message } => {
            Assessment::Warned(StorageWarning::Untested { reason: message })
        }
        Linked::Unconfirmed => Assessment::Warned(StorageWarning::Untested {
            reason: "a hardlink was made but could not be confirmed to point at one file"
                .to_owned(),
        }),
    }
}

/// The deepest ancestor of `path` already on disk, resolved through any symlinks,
/// and whether it is `path` itself — or nothing where not even the root of it can
/// be reached.
///
/// The flag is how a caller tells a location it tested directly from one whose
/// answer it had to read off a parent, since the chosen leaf does not exist yet.
async fn nearest_existing(filesystem: &dyn FileSystem, path: &Path) -> Option<(PathBuf, bool)> {
    let mut exact = true;
    for ancestor in path.ancestors() {
        if let Ok(real) = filesystem.canonicalize(ancestor).await {
            return Some((real, exact));
        }
        // Past the first step the resolved place is a parent, not the path asked
        // about, so a link proven there is inferred rather than direct.
        exact = false;
    }
    None
}

/// Save where the wizard has reached, so quitting mid-setup resumes rather than
/// restarts.
///
/// Best-effort: a progress file that could not be written costs the resume, not
/// the run, so it is not raised. The operator can still finish here; only a crash
/// before they do would lose what was gathered, which is what this guards against.
fn save(wizard: &Wizard, paths: &Paths) {
    let text = serde_json::to_string(wizard.progress()).unwrap_or_default();
    let _ = store::write(&paths.setup_progress(), &text);
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
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::{progress_at, run, Outcome, Prompt, StorageWarning};
    use crate::config::paths::Paths;
    use crate::config::{store, Protocols};
    use crate::platform::Environment;
    use crate::ports::filesystem::{Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts};
    use crate::prerequisites::PrerequisiteMap;
    use crate::stack::Source;
    use crate::wizard::{Answer, Library, Phase, Plan, Progress, Wizard};

    /// A filesystem the tests script, so a run is driven without touching a real
    /// disk. It answers the few calls the data-location probe makes; the rest it
    /// is never asked, and stubs plainly.
    struct ProbeFs {
        /// Whether `canonicalize` finds the path — false makes the probe walk up
        /// and, finding nothing, report the location untestable.
        reachable: bool,
        /// How many `canonicalize` calls fail before they start resolving, so a
        /// chosen leaf can be absent while a parent of it is found — the case where
        /// a link is proven on a parent rather than on the path itself.
        missing_leaves: AtomicUsize,
        /// Whether a probe file can be created there.
        writable: bool,
        /// How many link attempts fail before they start taking, so one chosen
        /// location can refuse to link and the next one accept it.
        failing_links: AtomicUsize,
        /// The file number the linked name reads back as; a value other than the
        /// probe's own models a link that could not be confirmed.
        confirmed_file: u64,
        /// The filesystem type reported, for naming why a link failed.
        kind: FsKind,
    }

    impl ProbeFs {
        /// A filesystem that links: reachable, writable, and confirming its links.
        fn links() -> Self {
            Self {
                reachable: true,
                missing_leaves: AtomicUsize::new(0),
                writable: true,
                failing_links: AtomicUsize::new(0),
                confirmed_file: 7,
                kind: FsKind::Linking("apfs".to_owned()),
            }
        }
    }

    #[async_trait]
    impl FileSystem for ProbeFs {
        async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
            if !self.reachable {
                return Err(Fault::new("no such file or directory"));
            }
            let missing = self.missing_leaves.load(Ordering::SeqCst);
            if missing > 0 {
                self.missing_leaves.store(missing - 1, Ordering::SeqCst);
                return Err(Fault::new("no such file or directory"));
            }
            Ok(path.to_owned())
        }
        async fn touch(&self, _path: &Path) -> Result<(), Fault> {
            if self.writable {
                Ok(())
            } else {
                Err(Fault::new("permission denied"))
            }
        }
        async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
            let remaining = self.failing_links.load(Ordering::SeqCst);
            if remaining > 0 {
                self.failing_links.store(remaining - 1, Ordering::SeqCst);
                Err(Fault::new("operation not permitted"))
            } else {
                Ok(())
            }
        }
        async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
            let file = if path.to_string_lossy().ends_with(".link") {
                self.confirmed_file
            } else {
                7
            };
            Ok(Identity { file, links: 2 })
        }
        async fn remove(&self, _path: &Path) {}
        async fn read(&self, _path: &Path) -> Option<String> {
            None
        }
        async fn write(&self, _path: &Path, _contents: &str) {}
        async fn ownership(&self, _path: &Path) -> Option<Ownership> {
            None
        }
        async fn describe(&self, _path: &Path) -> StorageFacts {
            StorageFacts {
                kind: self.kind.clone(),
                removable: false,
                available: 0,
                total: 0,
            }
        }
    }

    /// What the scripted operator answers when warned a location cannot hardlink.
    #[derive(Clone, Copy)]
    enum Accept {
        /// Use the location in hand anyway.
        Location,
        /// Decline it and be asked for another.
        Elsewhere,
    }

    /// A prompt that answers from a fixed script, so a run is driven with no
    /// terminal.
    struct Scripted {
        protocols: Protocols,
        service_user: Option<(u32, u32)>,
        library: Library,
        household: bool,
        autostart: bool,
        confirm: bool,
        /// The protocol choices each prerequisites checklist was derived from, in
        /// the order shown — so a test can prove the checklist reflects the answer.
        shown_prerequisites: std::cell::RefCell<Vec<Protocols>>,
        /// The locations offered in turn; each `data_location` call takes the next.
        /// Every test scripts as many as its run will ask for.
        locations: std::cell::RefCell<VecDeque<PathBuf>>,
        /// What the operator answers to "use this location anyway?".
        accept: Accept,
        /// The storage warnings put to the operator, in order.
        warnings: std::cell::RefCell<Vec<StorageWarning>>,
        /// The locations reported as hardlinking, each with whether the result was
        /// inferred from a parent rather than proven on the location itself.
        hardlinked: std::cell::RefCell<Vec<(PathBuf, bool)>>,
    }

    impl Scripted {
        /// A script that answers every question with a workable choice and confirms.
        fn workable(data_location: PathBuf) -> Self {
            Self {
                protocols: Protocols::both(),
                service_user: Some((1000, 1000)),
                library: Library::JellyfinDocker,
                household: true,
                autostart: false,
                confirm: true,
                shown_prerequisites: std::cell::RefCell::new(Vec::new()),
                locations: std::cell::RefCell::new(VecDeque::from([data_location])),
                accept: Accept::Elsewhere,
                warnings: std::cell::RefCell::new(Vec::new()),
                hardlinked: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Prompt for Scripted {
        fn protocols(&self) -> Protocols {
            self.protocols
        }
        fn prerequisites(&self, map: &PrerequisiteMap) {
            self.shown_prerequisites.borrow_mut().push(map.protocols);
        }
        fn data_location(&self) -> PathBuf {
            self.locations.borrow_mut().pop_front().unwrap_or_default()
        }
        fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>) {
            self.hardlinked
                .borrow_mut()
                .push((path.to_owned(), inferred_from.is_some()));
        }
        fn storage_warning(&self, _path: &Path, warning: &StorageWarning) -> bool {
            self.warnings.borrow_mut().push(warning.clone());
            matches!(self.accept, Accept::Location)
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
                &paths,
                external(),
                "t"
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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

        let refused = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(refused, Err(problem) if problem.code == super::ALREADY_UNDERWAY));
    }

    #[tokio::test]
    async fn an_answer_that_does_not_apply_here_stops_the_run() {
        let dir = scratch("rejected");
        let paths = layout(&dir);
        // Native Jellyfin buys nothing on native Linux, so a prompt that offers it
        // anyway is rejected rather than applied.
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            library: Library::JellyfinNative,
            ..Scripted::workable(dir.join("data-root"))
        };

        let stopped = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(stopped, Err(problem) if problem.code == super::DOES_NOT_APPLY));
        assert!(!paths.env_file().exists(), "nothing was applied");
    }

    /// A filesystem that cannot link, of a named type, so the copy-only warning
    /// carries the reason.
    fn cannot_link(kind: FsKind) -> ProbeFs {
        ProbeFs {
            failing_links: AtomicUsize::new(usize::MAX),
            kind,
            ..ProbeFs::links()
        }
    }

    #[tokio::test]
    async fn a_location_that_cannot_hardlink_is_put_to_the_operator_who_may_use_it() {
        let dir = scratch("copy-only");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        // The operator, told the location copies and why, chooses to use it anyway.
        let prompt = Scripted {
            accept: Accept::Location,
            ..Scripted::workable(dir.join("data-root"))
        };

        let outcome = run(
            &mut wizard,
            &prompt,
            &cannot_link(FsKind::ExFat),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        // The warning named the filesystem's own reason, so the operator weighed a
        // real fact rather than a bare "cannot hardlink".
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::CopyOnly { limitation: Some(reason) }] if reason.contains("exFAT")
        ));
    }

    #[tokio::test]
    async fn a_copy_only_location_names_no_reason_where_its_type_explains_nothing() {
        let dir = scratch("copy-only-unnamed");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        // A filesystem that normally links but did not is a fault to report plainly,
        // not one to blame on its type.
        let prompt = Scripted {
            accept: Accept::Location,
            ..Scripted::workable(dir.join("data-root"))
        };

        let outcome = run(
            &mut wizard,
            &prompt,
            &cannot_link(FsKind::Linking("ext4".to_owned())),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::CopyOnly { limitation: None }]
        ));
    }

    #[tokio::test]
    async fn a_location_that_cannot_link_can_be_swapped_for_one_that_can() {
        let dir = scratch("swap");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        // Two locations offered: the first cannot link and is declined, the second
        // links and is taken.
        let first = dir.join("copies");
        let second = dir.join("links");
        let prompt = Scripted {
            locations: std::cell::RefCell::new(VecDeque::from([first.clone(), second.clone()])),
            accept: Accept::Elsewhere,
            ..Scripted::workable(second.clone())
        };
        // The first link attempt fails; every one after it takes.
        let filesystem = ProbeFs {
            failing_links: AtomicUsize::new(1),
            ..ProbeFs::links()
        };

        let outcome = run(&mut wizard, &prompt, &filesystem, &paths, external(), "t").await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        // The declined location was warned about; the one taken was the linking one.
        assert_eq!(prompt.warnings.borrow().len(), 1);
        assert_eq!(
            prompt.hardlinked.borrow().as_slice(),
            [(second.clone(), false)]
        );
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(
            file.get("DATA_ROOT"),
            Some(second.to_string_lossy().as_ref())
        );
    }

    #[tokio::test]
    async fn a_location_that_does_not_exist_yet_is_tested_through_its_parent() {
        let dir = scratch("inferred");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted::workable(dir.join("data-root"));
        // The chosen leaf is not there yet, so the first resolve fails and its
        // parent stands in — the link is proven on the parent, not the path.
        let filesystem = ProbeFs {
            missing_leaves: AtomicUsize::new(1),
            ..ProbeFs::links()
        };

        let outcome = run(&mut wizard, &prompt, &filesystem, &paths, external(), "t").await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        // The good result is carried back as inferred, so the operator is not told a
        // path the probe never touched is proven to link.
        assert_eq!(
            prompt.hardlinked.borrow().as_slice(),
            [(dir.join("data-root"), true)]
        );
    }

    #[tokio::test]
    async fn a_location_that_cannot_be_written_is_reported_untestable() {
        let dir = scratch("unwritable");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            accept: Accept::Location,
            ..Scripted::workable(dir.join("data-root"))
        };
        let filesystem = ProbeFs {
            writable: false,
            ..ProbeFs::links()
        };

        let outcome = run(&mut wizard, &prompt, &filesystem, &paths, external(), "t").await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        // A location that could not be tested is reported as untested, with why —
        // not silently passed nor called copy-only.
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::Untested { reason }] if reason.contains("permission")
        ));
    }

    #[tokio::test]
    async fn a_location_with_no_reachable_parent_cannot_be_tested() {
        let dir = scratch("unreachable");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            accept: Accept::Location,
            ..Scripted::workable(dir.join("data-root"))
        };
        // Nothing on the path resolves — not the location, not any parent of it.
        let filesystem = ProbeFs {
            reachable: false,
            ..ProbeFs::links()
        };

        let outcome = run(&mut wizard, &prompt, &filesystem, &paths, external(), "t").await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::Untested { reason }] if reason.contains("could not be reached")
        ));
    }

    #[tokio::test]
    async fn the_probe_filesystem_stubs_the_calls_the_probe_never_makes() {
        // The data-location probe reaches only for what proves a hardlink; the rest
        // of the filesystem port it never touches. Pinning the double's answers to
        // those keeps a future probe that did start calling them from meeting a
        // surprise rather than a defined stub.
        let filesystem = ProbeFs::links();
        assert_eq!(filesystem.read(Path::new("/anything")).await, None);
        filesystem.write(Path::new("/anything"), "ignored").await;
        assert_eq!(filesystem.ownership(Path::new("/anything")).await, None);
    }

    #[tokio::test]
    async fn a_link_that_cannot_be_confirmed_is_reported_untestable() {
        let dir = scratch("unconfirmed");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = Scripted {
            accept: Accept::Location,
            ..Scripted::workable(dir.join("data-root"))
        };
        // The link is made, but the two names read back as different files.
        let filesystem = ProbeFs {
            confirmed_file: 999,
            ..ProbeFs::links()
        };

        let outcome = run(&mut wizard, &prompt, &filesystem, &paths, external(), "t").await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::Untested { reason }] if reason.contains("could not be confirmed")
        ));
    }
}

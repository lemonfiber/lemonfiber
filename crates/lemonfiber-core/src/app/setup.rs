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

mod answering;
mod proving;

pub use answering::{setting_up, SetupAction, ALREADY_SET_UP};
use proving::{resolve_credentials, resolve_location, resolve_provider, resolve_vpn};

use std::path::{Path, PathBuf};

use crate::alert::Appetite;
use crate::app::apply;
use crate::config::paths::Paths;
use crate::config::{store, Protocols};
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;
use crate::prerequisites::{prerequisites, PrerequisiteMap};
use crate::stack::Source;
use crate::validate::{Validation, Validator};
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
    /// Whether a VPN will carry the torrent traffic. Asked only where torrents
    /// were chosen, and after the checklist has said what one is for.
    fn vpn(&self) -> bool;
    /// Torrents were chosen and nothing will carry them: state what that exposes
    /// and ask whether to go on anyway (`true`) or reconsider (`false`). Going on
    /// is always available — this warns, it never refuses.
    fn unprotected(&self) -> bool;
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
    /// Ask for the indexer's URL and API key, or nothing where the operator has
    /// none to give now. The key is read without echo and never printed back — the
    /// review shows it redacted.
    fn credential(&self) -> Option<(String, String)>;
    /// The credential was proven — report the capability observed while proving it,
    /// so the operator sees the test succeed rather than a silent pass.
    fn credential_valid(&self, observed: &str);
    /// The credential could not be proven: show what the live test came to, told
    /// apart by cause, and ask whether to try again, proceed with it unverified, or
    /// leave it unset for now.
    fn credential_failed(&self, outcome: &Validation) -> CredentialChoice;
    /// Ask for the Usenet provider's host, port, login and TLS, or nothing where
    /// the operator has none to give now. The password is read without echo and
    /// never printed back — the review shows it redacted. Its live test reports
    /// through [`Prompt::credential_valid`] and [`Prompt::credential_failed`], the
    /// same as the indexer's.
    fn usenet_provider(&self) -> Option<ProviderEntry>;
    /// The user and group the containers run as, asked only where ownership shows;
    /// `None` where the operator declines and the image's own default is kept.
    fn service_user(&self) -> Option<(u32, u32)>;
    /// Whether to run Jellyfin, and how.
    fn library(&self) -> Library;
    /// Whether others in the home will use it.
    fn household(&self) -> bool;
    /// How much the operator wants to be told about — one question, three presets,
    /// never a checklist of every event.
    fn notifications(&self) -> Appetite;
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

/// A Usenet provider login as the operator enters it, before it is proven — the
/// same fields the wizard keeps, without the `validated` the test decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    /// The provider's hostname.
    pub host: String,
    /// The port it answers NNTP on.
    pub port: u16,
    /// The account username.
    pub user: String,
    /// The account password.
    pub pass: String,
    /// Whether to connect over TLS.
    pub tls: bool,
}

/// What the operator does with a credential the live test could not prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialChoice {
    /// Enter it again and test it afresh.
    Retry,
    /// Keep it as it is, unverified, and go on — it is their machine to do so.
    Proceed,
    /// Leave it unset for now.
    Skip,
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
    validator: &dyn Validator,
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
    gather(wizard, prompt, filesystem, validator, paths)
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
    clear_progress(paths);
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

/// Take a gathered setup through review and apply it, from the answers it holds.
///
/// The one way answers become configuration, whether they were gathered in this
/// process or over a sequence of requests, and however far a previous attempt got:
/// review is entered only from a complete set of answers, so an incomplete one is
/// refused here rather than judged by each caller.
///
/// A failed apply leaves the wizard restored to `applying` with every answer
/// intact — apply persists them before it writes. Rolling that one step back to
/// review is the wizard's single backward edge, and from review apply carries it
/// forward again. Apply is idempotent, so this either finishes what an interrupted
/// run started or leaves the same recoverable marker to try again.
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
    apply::apply(wizard, paths, source, stamp)?;
    clear_progress(paths);
    Ok(())
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
    /// Ask whether a VPN carries the torrents, and where none does, put what that
    /// means to the operator before recording that they accepted it.
    Vpn,
    /// Ask for a data location and prove what it can do before recording it.
    Location,
    /// Ask for a credential and prove it against its live service before keeping it.
    Credential,
    /// Ask for a Usenet provider and prove its login before keeping it.
    Provider,
}

async fn gather(
    wizard: &mut Wizard,
    prompt: &dyn Prompt,
    filesystem: &dyn FileSystem,
    validator: &dyn Validator,
    paths: &Paths,
) -> Result<(), Rejected> {
    // Each question is paired with how to ask it, so the walk is one loop: ask,
    // record, and save before moving on, so quitting mid-setup resumes at the
    // question reached rather than restarting. One loop keeps the recording a
    // single `?` rather than one tucked inside each conditional.
    let questions: [(Step, How); 10] = [
        (
            Step::Protocols,
            How::Sync(|prompt| Answer::Protocols(prompt.protocols())),
        ),
        (Step::Vpn, How::Vpn),
        (Step::DataLocation, How::Location),
        (Step::Credentials, How::Credential),
        (Step::Provider, How::Provider),
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
            Step::Notifications,
            How::Sync(|prompt| Answer::Notifications(prompt.notifications())),
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
            How::Vpn => resolve_vpn(prompt),
            How::Location => Answer::DataLocation(resolve_location(prompt, filesystem).await),
            How::Credential => resolve_credentials(prompt, validator).await,
            How::Provider => resolve_provider(prompt, validator).await,
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

/// Remove the saved progress once setup has fully applied.
///
/// The progress file exists to make an interrupted setup resumable, and to do
/// that it holds every gathered answer — the indexer key and the Usenet password
/// among them — in the clear. A finished apply has nothing left to resume and has
/// already written those secrets to their real home in the `.env`, so this second
/// copy is not left lying on disk. Best-effort, and safe as one: apply persisted
/// the durable `applied` marker immediately before this, so a removal that does
/// not happen leaves a file a later run still reads as a finished setup, not a
/// lost one — and removing a file already gone is not a failure.
fn clear_progress(paths: &Paths) {
    let _ = std::fs::remove_file(paths.setup_progress());
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
        "Setup only offers what applies where it runs, so the answer was not recorded. Nothing has been applied.",
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
    use crate::validate::Credential;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::{
        progress_at, run, CredentialChoice, Outcome, Prompt, ProviderEntry, StorageWarning,
    };
    use crate::alert::Appetite;
    use crate::config::paths::Paths;
    use crate::config::{store, Protocols};
    use crate::platform::Environment;
    use crate::ports::filesystem::{Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts};
    use crate::prerequisites::PrerequisiteMap;
    use crate::stack::Source;
    use crate::validate::{Validation, Validator};
    use crate::wizard::{Answer, Library, Phase, Plan, Progress, Vpn, Wizard};

    /// A validator that answers with scripted outcomes, so a run is driven with no
    /// network. Each call takes the next outcome, and the last is repeated once they
    /// run out — so one failing then passing drives a retry, and a single outcome
    /// answers however many times it is asked.
    struct Proving {
        outcomes: Vec<Validation>,
        calls: AtomicUsize,
    }

    impl Proving {
        fn giving(outcomes: Vec<Validation>) -> Self {
            Self {
                outcomes,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Validator for Proving {
        async fn validate(&self, _credential: &Credential) -> Validation {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let index = call.min(self.outcomes.len() - 1);
            self.outcomes[index].clone()
        }
    }

    /// A validator whose outcome does not matter because the run never enters a
    /// credential — the credential-free tests skip that step.
    fn proving() -> Proving {
        Proving::giving(vec![Validation::Valid {
            observed: "unused".to_owned(),
        }])
    }

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
        notifications: Appetite,
        autostart: bool,
        confirm: bool,
        /// The protocol choices each prerequisites checklist was derived from, in
        /// the order shown — so a test can prove the checklist reflects the answer.
        shown_prerequisites: std::cell::RefCell<Vec<Protocols>>,
        /// What the operator answers to "will a VPN carry it?", in turn — so a test
        /// can decline once and accept on the retry. Empty answers yes, which is the
        /// unremarkable case every other test wants.
        vpn: std::cell::RefCell<VecDeque<bool>>,
        /// What the operator answers to the unprotected-torrents warning, in turn.
        /// Empty answers yes, so a test that scripts a "no" to the VPN question and
        /// nothing here gets the accepted-the-exposure path.
        unprotected: std::cell::RefCell<VecDeque<bool>>,
        /// How many times the exposure was put to the operator — a warning that
        /// never appeared and one that appeared silently look the same otherwise.
        warned_unprotected: std::cell::Cell<usize>,
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
        /// The indexer the operator enters, or none to leave it unset. Returned on
        /// every `credential` call, so a retry re-enters the same one.
        credential: Option<(String, String)>,
        /// The Usenet provider the operator enters, or none to leave it unset.
        provider: Option<ProviderEntry>,
        /// What the operator does with a credential the test could not prove.
        on_failure: CredentialChoice,
        /// The observed facts of credentials proven, and the outcomes of those that
        /// were not — so a test can see which path the run took.
        proven: std::cell::RefCell<Vec<String>>,
        failures: std::cell::RefCell<Vec<Validation>>,
    }

    impl Scripted {
        /// A script that answers every question with a workable choice and confirms.
        /// It enters no credential, the supported path for a run that is not about
        /// them — the credential tests set one.
        fn workable(data_location: PathBuf) -> Self {
            Self {
                protocols: Protocols::both(),
                service_user: Some((1000, 1000)),
                library: Library::JellyfinDocker,
                household: true,
                notifications: Appetite::default_appetite(),
                autostart: false,
                confirm: true,
                shown_prerequisites: std::cell::RefCell::new(Vec::new()),
                vpn: std::cell::RefCell::new(VecDeque::new()),
                unprotected: std::cell::RefCell::new(VecDeque::new()),
                warned_unprotected: std::cell::Cell::new(0),
                locations: std::cell::RefCell::new(VecDeque::from([data_location])),
                accept: Accept::Elsewhere,
                warnings: std::cell::RefCell::new(Vec::new()),
                hardlinked: std::cell::RefCell::new(Vec::new()),
                credential: None,
                provider: None,
                on_failure: CredentialChoice::Skip,
                proven: std::cell::RefCell::new(Vec::new()),
                failures: std::cell::RefCell::new(Vec::new()),
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
        fn vpn(&self) -> bool {
            self.vpn.borrow_mut().pop_front().unwrap_or(true)
        }

        fn unprotected(&self) -> bool {
            self.warned_unprotected
                .set(self.warned_unprotected.get().saturating_add(1));
            self.unprotected.borrow_mut().pop_front().unwrap_or(true)
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
        fn credential(&self) -> Option<(String, String)> {
            self.credential.clone()
        }
        fn credential_valid(&self, observed: &str) {
            self.proven.borrow_mut().push(observed.to_owned());
        }
        fn credential_failed(&self, outcome: &Validation) -> CredentialChoice {
            self.failures.borrow_mut().push(outcome.clone());
            self.on_failure
        }
        fn usenet_provider(&self) -> Option<ProviderEntry> {
            self.provider.clone()
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

        fn notifications(&self) -> Appetite {
            self.notifications
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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
            &paths,
            external(),
            "t",
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
            &proving(),
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
            &proving(),
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
            &proving(),
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
            &proving(),
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
            &proving(),
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
            &proving(),
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

        let outcome = run(
            &mut wizard,
            &prompt,
            &filesystem,
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

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

        let outcome = run(
            &mut wizard,
            &prompt,
            &filesystem,
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

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

        let outcome = run(
            &mut wizard,
            &prompt,
            &filesystem,
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

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

        let outcome = run(
            &mut wizard,
            &prompt,
            &filesystem,
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

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

        let outcome = run(
            &mut wizard,
            &prompt,
            &filesystem,
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let warnings = prompt.warnings.borrow();
        assert!(matches!(
            warnings.as_slice(),
            [StorageWarning::Untested { reason }] if reason.contains("could not be confirmed")
        ));
    }

    /// A prompt that enters the given indexer and answers a failed test the given
    /// way — everything else the workable defaults.
    fn entering(dir: &Path, on_failure: CredentialChoice) -> Scripted {
        Scripted {
            credential: Some(("http://indexer.test/api".to_owned(), "the-key".to_owned())),
            on_failure,
            ..Scripted::workable(dir.join("data-root"))
        }
    }

    #[tokio::test]
    async fn a_credential_proven_is_kept_and_recorded_as_validated() {
        let dir = scratch("cred-valid");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering(&dir, CredentialChoice::Skip);
        let validator = Proving::giving(vec![Validation::Valid {
            observed: "answered a search — 12 result(s) offered".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("INDEXER_APIKEY"), Some("the-key"));
        assert_eq!(file.get("INDEXER_VALIDATED"), Some("on"));
        // The operator was shown what the test observed, not a bare pass.
        assert!(prompt
            .proven
            .borrow()
            .iter()
            .any(|observed| observed.contains("12 result")));
    }

    #[tokio::test]
    async fn a_credential_that_fails_then_passes_on_retry_is_kept() {
        let dir = scratch("cred-retry");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering(&dir, CredentialChoice::Retry);
        // The first test refuses the key, the second — after the operator re-enters
        // it — proves it.
        let validator = Proving::giving(vec![
            Validation::Rejected {
                detail: "the indexer refused the key".to_owned(),
            },
            Validation::Valid {
                observed: "answered a search".to_owned(),
            },
        ]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        assert_eq!(prompt.failures.borrow().len(), 1, "one refusal was shown");
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("INDEXER_VALIDATED"), Some("on"));
    }

    #[tokio::test]
    async fn a_credential_kept_unverified_records_that_it_was_not_proven() {
        let dir = scratch("cred-proceed");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering(&dir, CredentialChoice::Proceed);
        let validator = Proving::giving(vec![Validation::Unreachable {
            detail: "nothing answered".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        // Kept, so it is not lost — but recorded as unverified so a later diagnosis
        // can point at it rather than trusting it.
        assert_eq!(file.get("INDEXER_APIKEY"), Some("the-key"));
        assert_eq!(file.get("INDEXER_VALIDATED"), Some("off"));
    }

    #[tokio::test]
    async fn a_credential_skipped_leaves_the_indexer_unset() {
        let dir = scratch("cred-skip");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering(&dir, CredentialChoice::Skip);
        let validator = Proving::giving(vec![Validation::Rejected {
            detail: "refused".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("INDEXER_URL"), None, "nothing was written for it");
    }

    #[tokio::test]
    async fn no_indexer_is_a_supported_end_and_persists_nothing_before_a_test() {
        let dir = scratch("cred-none");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        // The workable prompt enters no credential — a supported path, and one that
        // never reaches the validator, so nothing about a credential is persisted
        // without a test having run.
        let prompt = Scripted::workable(dir.join("data-root"));

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &proving(),
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("INDEXER_URL"), None);
    }

    /// A prompt that enters the given Usenet provider and answers a failed login
    /// the given way — everything else the workable defaults.
    fn entering_provider(dir: &Path, on_failure: CredentialChoice) -> Scripted {
        Scripted {
            provider: Some(ProviderEntry {
                host: "news.provider.test".to_owned(),
                port: 563,
                user: "person".to_owned(),
                pass: "secret".to_owned(),
                tls: true,
            }),
            on_failure,
            ..Scripted::workable(dir.join("data-root"))
        }
    }

    #[tokio::test]
    async fn a_usenet_provider_proven_is_kept_and_recorded_as_validated() {
        let dir = scratch("provider-valid");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering_provider(&dir, CredentialChoice::Skip);
        let validator = Proving::giving(vec![Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("USENET_HOST"), Some("news.provider.test"));
        assert_eq!(file.get("USENET_USER"), Some("person"));
        assert_eq!(file.get("USENET_VALIDATED"), Some("on"));
        assert!(prompt
            .proven
            .borrow()
            .iter()
            .any(|observed| observed.contains("accepted the login")));
    }

    #[tokio::test]
    async fn a_provider_that_fails_then_passes_on_retry_is_kept() {
        let dir = scratch("provider-retry");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering_provider(&dir, CredentialChoice::Retry);
        let validator = Proving::giving(vec![
            Validation::Rejected {
                detail: "the provider refused the username or password".to_owned(),
            },
            Validation::Valid {
                observed: "the provider accepted the login".to_owned(),
            },
        ]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        assert_eq!(prompt.failures.borrow().len(), 1);
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("USENET_VALIDATED"), Some("on"));
    }

    #[tokio::test]
    async fn a_provider_kept_unverified_records_that_it_was_not_proven() {
        let dir = scratch("provider-proceed");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering_provider(&dir, CredentialChoice::Proceed);
        let validator = Proving::giving(vec![Validation::Unreachable {
            detail: "nothing answered".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("USENET_HOST"), Some("news.provider.test"));
        assert_eq!(file.get("USENET_VALIDATED"), Some("off"));
    }

    #[tokio::test]
    async fn a_provider_skipped_leaves_usenet_unset() {
        let dir = scratch("provider-skip");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        let prompt = entering_provider(&dir, CredentialChoice::Skip);
        let validator = Proving::giving(vec![Validation::Rejected {
            detail: "refused".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("USENET_HOST"), None);
    }

    #[tokio::test]
    async fn a_library_only_run_is_never_asked_for_a_credential() {
        let dir = scratch("cred-library-only");
        let paths = layout(&dir);
        let mut wizard = Wizard::new(Environment::LinuxNative);
        // Neither protocol chosen, so there is no download service to hold a
        // credential — the step does not apply and is passed over, even though the
        // prompt would have offered one.
        let prompt = Scripted {
            protocols: Protocols::none(),
            credential: Some(("http://indexer.test/api".to_owned(), "k".to_owned())),
            ..Scripted::workable(dir.join("data-root"))
        };
        // A validator that would fail if it were ever asked, proving it is not.
        let validator = Proving::giving(vec![Validation::Rejected {
            detail: "should never be reached".to_owned(),
        }]);

        let outcome = run(
            &mut wizard,
            &prompt,
            &ProbeFs::links(),
            &validator,
            &paths,
            external(),
            "t",
        )
        .await;

        assert!(matches!(outcome, Ok(Outcome::Applied)));
        assert!(
            prompt.failures.borrow().is_empty(),
            "no credential was tested"
        );
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("INDEXER_URL"), None);
    }
}

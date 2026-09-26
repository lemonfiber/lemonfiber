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

pub use answering::{setup, SetupAction};
use proving::{resolve_credentials, resolve_location, resolve_provider, resolve_vpn};

use std::path::{Path, PathBuf};

use crate::alert::Appetite;
use crate::app::apply::{self, Applying};
use crate::config::paths::Paths;
use crate::config::{store, Protocols};
use crate::error::codes::setup::{ALREADY_UNDERWAY, DOES_NOT_APPLY};
use crate::error::{Amiss, Diagnose as _, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;
use crate::prerequisites::{prerequisites, PrerequisiteMap};
use crate::validate::{Validation, Validator};
use crate::wizard::{
    described, Answer, Choice, Library, Phase, Plan, Progress, Recovery, Rejected, Resolution,
    Step, Wizard,
};

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
    applying: &Applying<'_>,
) -> Result<Outcome, Box<Problem>> {
    // Only a wizard still gathering is driven here. One that has been reviewed,
    // is part-way through an interrupted apply, or is already applied must be
    // routed by its caller — resumed, recovered, or pointed at reconfiguration —
    // because re-applying it would write over the very journal a recovery reads.
    if wizard.phase() != Phase::InProgress {
        return Err(Box::new(already_underway()));
    }
    gather(wizard, prompt, filesystem, validator, applying.paths)
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
    apply::apply(wizard, applying)?;
    clear_progress(applying.paths);
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
pub fn resume(wizard: &mut Wizard, applying: &Applying<'_>) -> Result<(), Box<Problem>> {
    wizard.transition(Phase::Reviewing);
    apply::apply(wizard, applying)?;
    clear_progress(applying.paths);
    Ok(())
}

/// What an apply would write, without writing any of it.
///
/// The same gate [`resume`] passes and the same refusal where it does not: review is
/// reached only from a complete set of answers, so a rehearsal is turned back exactly
/// where the run that writes is turned back rather than by a second judgement beside
/// it. Below that gate an apply is nothing but writes — the progress file, the journal,
/// the data directory, the stack, every setting, the appetite and the autostart answer
/// — so this stops at it, and the plan the wizard now holds is the whole of the report.
///
/// # Errors
///
/// Returns the [`Problem`] a real apply gives for answers that have not reached review.
pub(crate) fn would_apply(wizard: &mut Wizard) -> Result<(), Box<Problem>> {
    if wizard.transition(Phase::Reviewing) {
        return Ok(());
    }
    Err(Box::new(apply::not_reviewed()))
}

/// Take an interrupted apply the way it was chosen out of, and leave nothing of it
/// stranded.
///
/// The whole of the recovery rather than the deciding half of it: the reversal is
/// computed by [`Recovery`], carried out here, and followed by whatever the choice
/// asked for next — so a surface offers three words and does none of the work, and
/// three surfaces cannot come to disagree about what one of them means.
///
/// Starting over removes the journal as well as the answers, because a reversal
/// already carried out and still recorded is one a later recovery would try again.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render where a recorded change could
/// not be reversed, or where the apply that follows a resume or a roll back fails —
/// which leaves the marker at `applying` for another attempt, exactly as an apply
/// asked for directly does.
pub(crate) fn recovered(
    wizard: &mut Wizard,
    applying: &Applying<'_>,
    choice: Choice,
) -> Result<(), Box<Problem>> {
    let paths = applying.paths;
    let journal = crate::app::recover::journal_at(&paths.journal())
        .map_err(|failure| Box::new(failure.problem()))?;
    let env = paths.env_file();
    match Recovery::of(&journal).resolve(choice) {
        Resolution::Resume => resume(wizard, applying),
        Resolution::RollBack(undos) => {
            crate::app::recover::undo(&undos, &env, Vec::new())?;
            resume(wizard, applying)
        }
        Resolution::StartOver(undos) => {
            crate::app::recover::undo(&undos, &env, Vec::new())?;
            clear_progress(paths);
            let _ = std::fs::remove_file(paths.journal());
            Ok(())
        }
    }
}

/// What an apply that stopped part-way had already written, each said plainly.
///
/// The partial state a recovery is chosen about. Read from the change journal the
/// apply wrote as it went, so the choice is offered against what really happened
/// rather than against a guess — and said in the journal's own words, so the list
/// reads the same wherever the choice is put.
///
/// Empty where the apply stopped before it wrote anything, which is worth being
/// able to tell apart from having written something. A journal that cannot be read is
/// said as that, in the one line the list then holds, rather than as a list with
/// nothing in it.
#[must_use]
pub fn written_so_far(paths: &Paths) -> Vec<String> {
    match crate::app::recover::journal_at(&paths.journal()) {
        Ok(journal) => journal.changes().iter().map(described).collect(),
        Err(failure) => vec![failure.to_string()],
    }
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
    .lies_in(Amiss::Asking)
    .with_detail(format!("{rejected:?}"))
}

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

#[cfg(test)]
mod tests;

//! The setup wizard's state machine: the steps, in order, and where the operator
//! stands among them.
//!
//! The division of labour is "core decides the steps, a surface drives the
//! prompting". This module decides what to ask, in what order, and what may be
//! skipped because it was detected or does not apply on this platform; a surface
//! renders each step and collects the answer. Nothing here reads stdin or writes
//! to disk — the wizard is a value that advances, which is what makes it both
//! resumable (serialise its progress) and testable (drive it) without either.
//!
//! Read-only by construction: the wizard holds answers and never persists them,
//! so the steps before review cannot touch disk. Applying them — writing the
//! environment file, materialising the stack, starting services — is a separate
//! phase a surface performs against a reviewed set of answers.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::env::EnvFile;
use crate::config::{
    Protocols, DATA_ROOT_KEY, JELLYFIN_MODE_KEY, PGID_KEY, PUID_KEY, TORRENT_KEY, USENET_KEY,
};
use crate::journal::{Change, Journal, Kind, Undo};
use crate::platform::Environment;

/// How the operator wants their existing and downloaded media served, where they
/// want it served at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Library {
    /// No media server; an existing player, or downloads only.
    None,
    /// Jellyfin in a container — the portable default that works everywhere.
    JellyfinDocker,
    /// Jellyfin on the host, for a hardware encoder the container cannot reach.
    ///
    /// Only a valid answer where the platform offers it; the wizard rejects it
    /// elsewhere rather than letting a surface record a choice that buys nothing.
    JellyfinNative,
}

impl Library {
    /// The `JELLYFIN_MODE` this choice records, where it runs Jellyfin at all.
    ///
    /// `None` means no media server, which writes no mode rather than a third one.
    const fn mode(self) -> Option<&'static str> {
        match self {
            Self::JellyfinDocker => Some("docker"),
            Self::JellyfinNative => Some("native"),
            Self::None => None,
        }
    }
}

/// A step of setup, in the order the operator meets it.
///
/// Some steps only inform (they detect and state, and the operator acknowledges);
/// others ask a question whose answer the wizard records. The apply-and-onward
/// steps — writing config, pulling images, wiring services — are not modelled
/// here yet: they arrive with the features they drive, and this machine covers
/// the read-only phase that precedes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// States what is about to happen and roughly how long it takes. Informs.
    #[default]
    Welcome,
    /// Detects the environment: OS, Docker, Compose, daemon reachability. Informs.
    Preflight,
    /// The account checklist derived from the protocol choice. Informs.
    Prerequisites,
    /// Usenet, torrents, both, or neither.
    Protocols,
    /// Where downloads and the library are kept.
    DataLocation,
    /// The user and group the containers run as. Asked only where it is visible.
    ServiceUser,
    /// Whether to run Jellyfin, and if so how.
    Library,
    /// Whether others in the home will use it.
    Household,
    /// Whether to start on boot.
    Autostart,
    /// The complete summary, before anything is written. Informs.
    Review,
}

impl Step {
    /// The steps in presentation order.
    const ORDER: [Self; 10] = [
        Self::Welcome,
        Self::Preflight,
        Self::Protocols,
        Self::Prerequisites,
        Self::DataLocation,
        Self::ServiceUser,
        Self::Library,
        Self::Household,
        Self::Autostart,
        Self::Review,
    ];

    /// This step's position in presentation order.
    ///
    /// A total match rather than a search through [`Self::ORDER`], so there is no
    /// "not found" case to handle for a value that is always one of the steps.
    const fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Preflight => 1,
            Self::Protocols => 2,
            Self::Prerequisites => 3,
            Self::DataLocation => 4,
            Self::ServiceUser => 5,
            Self::Library => 6,
            Self::Household => 7,
            Self::Autostart => 8,
            Self::Review => 9,
        }
    }

    /// Whether this step asks a question, as opposed to only informing.
    ///
    /// The distinction is what the non-interactive guard reports on: an informing
    /// step needs no answer, so its absence in a piped run is not a blocker.
    #[must_use]
    pub const fn is_question(self) -> bool {
        matches!(
            self,
            Self::Protocols
                | Self::DataLocation
                | Self::ServiceUser
                | Self::Library
                | Self::Household
                | Self::Autostart
        )
    }
}

/// What the operator has answered so far.
///
/// Every field is optional because a wizard mid-flight is partly answered, and a
/// resumed one is restored to exactly that partial state. This is also the review
/// payload: the complete set of values that will be written, shown before any of
/// them is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Answers {
    /// Which download protocols the operator wants.
    pub protocols: Option<Protocols>,
    /// Where downloads and the library live.
    pub data_location: Option<PathBuf>,
    /// The container user and group, where the platform makes it meaningful.
    ///
    /// Distinguishes "answered: no id needed here" (`Some(None)`) from "not yet
    /// answered" (`None`), because the wizard must know the step is done.
    pub service_user: Option<Option<(u32, u32)>>,
    /// How the library is served.
    pub library: Option<Library>,
    /// Whether the household beyond the operator will use it.
    pub household: Option<bool>,
    /// Whether the stack starts on boot.
    pub autostart: Option<bool>,
}

/// One answer, tagged by the step it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The protocol choice.
    Protocols(Protocols),
    /// The data location.
    DataLocation(PathBuf),
    /// The container user and group, or none where it is not needed.
    ServiceUser(Option<(u32, u32)>),
    /// How the library is served.
    Library(Library),
    /// Whether the household uses it.
    Household(bool),
    /// Whether to start on boot.
    Autostart(bool),
}

/// Why an answer could not be accepted.
///
/// Not every value a surface could submit is meaningful on every platform, and
/// the wizard rejects the ones that are not rather than recording a choice that
/// would silently do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// Native Jellyfin was chosen where the platform cannot reach a hardware
    /// encoder, so the choice would buy nothing.
    NativeJellyfinUnavailable,
    /// A container user was given where ownership is mapped away, so it would have
    /// no observable effect.
    ServiceUserNotApplicable,
}

/// Where an in-flight or finished setup stands in its lifecycle.
///
/// The persisted marker a later run reads to tell answers still being gathered
/// from a half-written apply. Only these four are ever written: the two states a
/// run infers instead of storing — no setup at all, and an apply that stopped
/// mid-write — are read off the world rather than trusted from a file (see
/// [`Status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Still gathering answers. Resumable, and nothing has touched disk.
    #[default]
    InProgress,
    /// Every applicable question answered, awaiting the operator's confirmation.
    Reviewing,
    /// Writing configuration and starting services — the one non-atomic phase,
    /// and so the only marker whose persistence signals an interrupted run.
    Applying,
    /// Configuration written and valid.
    Applied,
}

/// The part of the wizard that survives quitting: the step reached and the
/// answers gathered.
///
/// Serialisable on its own, and everything a resumed run needs. The environment
/// is deliberately not part of it — it is detected fresh each run, never restored
/// from a file that may have moved machines.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Progress {
    /// The step the operator had reached.
    pub at: Step,
    /// What they had answered.
    pub answers: Answers,
    /// Where this setup stands in its lifecycle, so a later run can tell a
    /// half-written apply from answers still being gathered. Missing from
    /// progress files written before it was tracked, which read back as the
    /// gathering phase — the state those files were only ever left in.
    #[serde(default)]
    pub phase: Phase,
}

/// The setup wizard: where the operator is, what they have answered, and the
/// environment that decides which questions apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wizard {
    environment: Environment,
    progress: Progress,
}

impl Wizard {
    /// A wizard at the beginning, for this environment.
    #[must_use]
    pub fn new(environment: Environment) -> Self {
        Self {
            environment,
            progress: Progress::default(),
        }
    }

    /// A wizard restored to where a previous run left off.
    ///
    /// The environment is supplied fresh rather than read from the progress,
    /// because the machine may have changed since — and a question that applied
    /// on the old one may not on the new. Where it has, the restored answers are
    /// reconciled to the new environment first, so a run resumed on a different
    /// machine never carries a choice this one rejects into what gets applied.
    #[must_use]
    pub fn resume(environment: Environment, progress: Progress) -> Self {
        let mut wizard = Self {
            environment,
            progress,
        };
        wizard.reconcile();
        wizard
    }

    /// Drop any restored answer this environment would refuse, and re-home a
    /// cursor left on a step it does not present.
    ///
    /// A value the machine has changed out from under — native Jellyfin now on
    /// Linux, a container user now on a platform that maps ownership away — is
    /// cleared rather than silently kept, because `Answers` is the set that will
    /// be written and a stale one would be applied. The cleared question then
    /// reappears as unanswered, to be asked afresh where it now applies.
    fn reconcile(&mut self) {
        if self.progress.answers.library == Some(Library::JellyfinNative)
            && !self.environment.offers_native_jellyfin()
        {
            self.progress.answers.library = None;
        }
        if matches!(self.progress.answers.service_user, Some(Some(_)))
            && !self.environment.ownership_is_real()
        {
            self.progress.answers.service_user = None;
        }
        if !self.applies(self.progress.at) {
            if let Some(rehomed) = self.neighbour(self.progress.at, Direction::Forward) {
                self.progress.at = rehomed;
            }
        }
    }

    /// The resumable state, to be serialised and written by a surface.
    #[must_use]
    pub const fn progress(&self) -> &Progress {
        &self.progress
    }

    /// The step the operator is on.
    #[must_use]
    pub const fn at(&self) -> Step {
        self.progress.at
    }

    /// The answers gathered so far — the review payload once complete.
    #[must_use]
    pub const fn answers(&self) -> &Answers {
        &self.progress.answers
    }

    /// Whether a step is presented at all in this environment.
    ///
    /// Only `ServiceUser` is conditional: the container user is worth asking about
    /// only where file ownership is real rather than mapped. Everything else is
    /// asked or shown regardless.
    #[must_use]
    pub const fn applies(&self, step: Step) -> bool {
        match step {
            Step::ServiceUser => self.environment.ownership_is_real(),
            _ => true,
        }
    }

    /// Record an answer against the step it belongs to.
    ///
    /// # Errors
    ///
    /// Returns [`Rejected`] where the value is not meaningful on this platform —
    /// native Jellyfin where it buys nothing, or a container user where ownership
    /// is mapped away.
    pub fn answer(&mut self, answer: Answer) -> Result<(), Rejected> {
        match answer {
            Answer::Protocols(protocols) => self.progress.answers.protocols = Some(protocols),
            Answer::DataLocation(path) => self.progress.answers.data_location = Some(path),
            Answer::ServiceUser(user) => {
                if user.is_some() && !self.environment.ownership_is_real() {
                    return Err(Rejected::ServiceUserNotApplicable);
                }
                self.progress.answers.service_user = Some(user);
            }
            Answer::Library(Library::JellyfinNative)
                if !self.environment.offers_native_jellyfin() =>
            {
                return Err(Rejected::NativeJellyfinUnavailable);
            }
            Answer::Library(library) => self.progress.answers.library = Some(library),
            Answer::Household(shared) => self.progress.answers.household = Some(shared),
            Answer::Autostart(boot) => self.progress.answers.autostart = Some(boot),
        }
        Ok(())
    }

    /// Move to the next step that applies, if there is one.
    ///
    /// Returns the new step, or `None` at the end — review is the last step, and
    /// advancing from it goes nowhere.
    pub fn advance(&mut self) -> Option<Step> {
        let next = self.neighbour(self.progress.at, Direction::Forward)?;
        self.progress.at = next;
        Some(next)
    }

    /// Move back to the previous step that applies, if there is one.
    ///
    /// Returns the new step, or `None` at the welcome — there is nowhere before it.
    pub fn back(&mut self) -> Option<Step> {
        let previous = self.neighbour(self.progress.at, Direction::Back)?;
        self.progress.at = previous;
        Some(previous)
    }

    /// The applicable step adjacent to `from` in the given direction, skipping any
    /// that do not apply here.
    fn neighbour(&self, from: Step, direction: Direction) -> Option<Step> {
        let index = from.index();
        match direction {
            Direction::Forward => Step::ORDER
                .into_iter()
                .skip(index + 1)
                .find(|step| self.applies(*step)),
            Direction::Back => Step::ORDER
                .into_iter()
                .take(index)
                .rev()
                .find(|step| self.applies(*step)),
        }
    }

    /// The question steps that apply here but have no answer yet.
    ///
    /// What a non-interactive run reports as the reason it cannot proceed: rather
    /// than blocking on a stdin that will never come, a surface names these and
    /// points at their flag equivalents.
    #[must_use]
    pub fn unanswered(&self) -> Vec<Step> {
        Step::ORDER
            .into_iter()
            .filter(|step| step.is_question() && self.applies(*step) && !self.is_answered(*step))
            .collect()
    }

    /// Whether a step's answer has been recorded.
    ///
    /// A step that does not apply counts as answered: there is nothing to collect,
    /// so it never holds the wizard up.
    #[must_use]
    pub const fn is_answered(&self, step: Step) -> bool {
        if !self.applies(step) {
            return true;
        }
        let answers = &self.progress.answers;
        match step {
            Step::Protocols => answers.protocols.is_some(),
            Step::DataLocation => answers.data_location.is_some(),
            Step::ServiceUser => answers.service_user.is_some(),
            Step::Library => answers.library.is_some(),
            Step::Household => answers.household.is_some(),
            Step::Autostart => answers.autostart.is_some(),
            // Informing steps have no answer to hold.
            Step::Welcome | Step::Preflight | Step::Prerequisites | Step::Review => true,
        }
    }

    /// Whether every applicable question has an answer, so review can proceed.
    #[must_use]
    pub fn ready_for_review(&self) -> bool {
        self.unanswered().is_empty()
    }

    /// Which lifecycle phase this setup is in.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.progress.phase
    }

    /// Move the lifecycle to `phase`, but only along an edge setup actually takes.
    /// Returns whether it moved.
    ///
    /// The legal edges are few and named here rather than inferred from an
    /// ordering, because the lifecycle is not a straight line: review is reached
    /// only once every applicable question is answered; apply follows review, and
    /// applied follows apply; and an apply that is rolled back returns to review to
    /// be run again — the one backward edge. Every other move — skipping review,
    /// re-applying a finished setup, quietly downgrading a persisted `applying` to
    /// look unstarted — is refused, so a caller cannot reach a writing or written
    /// phase without having passed the gate the earlier one stands for.
    pub fn transition(&mut self, phase: Phase) -> bool {
        let allowed = match (self.progress.phase, phase) {
            (Phase::InProgress | Phase::Applying, Phase::Reviewing) => self.ready_for_review(),
            (Phase::Reviewing, Phase::Applying) | (Phase::Applying, Phase::Applied) => true,
            _ => false,
        };
        if allowed {
            self.progress.phase = phase;
        }
        allowed
    }

    /// The configuration these answers will be written as.
    ///
    /// What review shows and what apply writes, the same value for both, so the
    /// operator confirms exactly what lands. Built from whatever has been
    /// answered, so it is empty at the start and complete at review; an
    /// unanswered question contributes no setting rather than a guessed default.
    /// The household and autostart choices have no configuration home here — they
    /// are applied by their own features — so they are collected but not written.
    #[must_use]
    pub fn plan(&self) -> Plan {
        let mut settings = Vec::new();
        let answers = &self.progress.answers;
        if let Some(protocols) = answers.protocols {
            settings.push((USENET_KEY.to_owned(), on_off(protocols.usenet)));
            settings.push((TORRENT_KEY.to_owned(), on_off(protocols.torrent)));
        }
        if let Some(path) = &answers.data_location {
            settings.push((DATA_ROOT_KEY.to_owned(), path.display().to_string()));
        }
        if let Some(Some((uid, gid))) = answers.service_user {
            settings.push((PUID_KEY.to_owned(), uid.to_string()));
            settings.push((PGID_KEY.to_owned(), gid.to_string()));
        }
        if let Some(mode) = answers.library.and_then(Library::mode) {
            settings.push((JELLYFIN_MODE_KEY.to_owned(), mode.to_owned()));
        }
        Plan { settings }
    }
}

/// The environment settings a reviewed wizard will write.
///
/// The answers turned into the keys the compose driver reads, in a stable order
/// so the review renders and the write compares the same way every time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    settings: Vec<(String, String)>,
}

impl Plan {
    /// The settings, as ordered key/value pairs to write.
    #[must_use]
    pub fn settings(&self) -> &[(String, String)] {
        &self.settings
    }

    /// The journal changes that applying this plan makes to the environment file,
    /// against what that file holds now.
    ///
    /// One `Set` per setting, each carrying the value the file held for that key
    /// before — read from `current`, `None` where the key is new — so applying is
    /// a recorded, reversible act rather than a blind overwrite, and an interrupted
    /// apply can be unwound to exactly what was there. The plan has no clock, so
    /// the caller stamps the time.
    #[must_use]
    pub fn changes(&self, current: &EnvFile, stamp: &str) -> Vec<Change> {
        self.settings
            .iter()
            .map(|(key, value)| Change {
                at: stamp.to_owned(),
                operation: APPLY.to_owned(),
                target: ENV_FILE.to_owned(),
                kind: Kind::Set {
                    key: key.clone(),
                    previous: current.get(key).map(str::to_owned),
                    current: value.clone(),
                },
            })
            .collect()
    }
}

/// The change-journal target for the environment file setup writes into — the
/// name it is known by in a history, not a machine-specific path.
const ENV_FILE: &str = ".env";

/// The change-journal operation these writes belong to, so a browsable history
/// reads as the setup that made them.
const APPLY: &str = "apply";

/// A yes/no answer as the on/off a hand-editable setting records.
fn on_off(enabled: bool) -> String {
    if enabled { "on" } else { "off" }.to_owned()
}

/// Which way [`Wizard::neighbour`] looks.
#[derive(Clone, Copy)]
enum Direction {
    /// Toward review.
    Forward,
    /// Toward welcome.
    Back,
}

/// Whether setup should be offered, given whether configuration already exists.
///
/// Offered exactly when there is nothing configured. Where configuration exists,
/// setup is not re-run — a surface directs the operator to reconfiguration
/// instead, so a working stack is never walked back to its first question.
#[must_use]
pub const fn offer_setup(configuration_present: bool) -> bool {
    !configuration_present
}

/// What a later run makes of the setup it finds — the whole lifecycle, read from
/// the world rather than trusted from a file.
///
/// Two of these are inferred rather than stored. No saved progress at all is
/// `Absent`; a saved progress still marked applying is `FailedApply`, because the
/// only thing that writes the applying marker is a live apply, so finding it
/// persisted means that apply stopped before it finished. Every other state is
/// its marker read straight back.
///
/// A surface must consult this before it decides whether to offer setup or resume
/// a run: `FailedApply` takes precedence over both. An interrupted apply that got
/// as far as writing configuration looks "already configured" to a naive check,
/// and its half-written state must be recovered rather than mistaken for a
/// finished one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No setup has been started; the wizard is offered.
    Absent,
    /// Setup is part-answered and resumable.
    InProgress,
    /// Answers are complete and awaiting confirmation.
    Reviewing,
    /// Setup finished and configuration is on disk.
    Applied,
    /// An apply was interrupted and must be recovered before setup goes on.
    FailedApply,
}

impl Status {
    /// Classify the setup a run finds from its saved progress, if any.
    ///
    /// No progress read back is `Absent`. A saved applying marker is the
    /// interrupted case, surfaced as `FailedApply` for deliberate recovery rather
    /// than resumed as if nothing had gone wrong.
    #[must_use]
    pub const fn of(progress: Option<&Progress>) -> Self {
        match progress {
            None => Self::Absent,
            Some(progress) => match progress.phase {
                Phase::InProgress => Self::InProgress,
                Phase::Reviewing => Self::Reviewing,
                Phase::Applying => Self::FailedApply,
                Phase::Applied => Self::Applied,
            },
        }
    }
}

/// What to do about an interrupted apply, once one is found.
///
/// The three exits the setup wizard promises for its one dangerous state: keep
/// going, walk it back, or drop it entirely. A surface offers these; [`Recovery`]
/// turns the chosen one into the work it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Carry on from where apply stopped, keeping what was already written.
    Resume,
    /// Undo what was written and return to the reviewed answers, ready to apply
    /// again.
    RollBack,
    /// Undo what was written and discard the answers too, back to a clean slate.
    StartOver,
}

/// The recovery offered for an interrupted apply: what it wrote, and what each
/// choice does about it.
///
/// Built from the change journal — the record of what apply managed to write
/// before it stopped — so the operator is shown the real partial state, not a
/// guess, and a reversal touches exactly what was recorded. Only recorded changes
/// can be reversed: whatever apply writes and wants undone, it must journal.
#[derive(Debug)]
pub struct Recovery<'a> {
    written: &'a Journal,
}

impl<'a> Recovery<'a> {
    /// The recovery for an apply that stopped after writing what `written` holds.
    #[must_use]
    pub const fn of(written: &'a Journal) -> Self {
        Self { written }
    }

    /// What the interrupted apply wrote, in the order it wrote it — the partial
    /// state to report before a choice is offered.
    #[must_use]
    pub fn written(&self) -> &[Change] {
        self.written.changes()
    }

    /// The concrete work a choice resolves to, for a surface to carry out.
    ///
    /// Both walking back and starting over reverse what was written — the one
    /// difference is whether the answers survive, so neither leaves the partial
    /// apply stranded on disk. The reversal is computed here, so the whole of it
    /// stays testable without a service or a disk.
    #[must_use]
    pub fn resolve(&self, choice: Choice) -> Resolution {
        match choice {
            Choice::Resume => Resolution::Resume,
            Choice::RollBack => Resolution::RollBack(self.written.rewind()),
            Choice::StartOver => Resolution::StartOver(self.written.rewind()),
        }
    }
}

/// The work a [`Choice`] resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Continue the apply, keeping every write already made.
    Resume,
    /// Reverse the recorded writes, most recent first, keeping the answers so the
    /// apply can be reviewed and run again.
    RollBack(Vec<Undo>),
    /// Reverse the recorded writes, then discard the saved progress and journal —
    /// nothing of this attempt left behind.
    StartOver(Vec<Undo>),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        offer_setup, Answer, Choice, Library, Phase, Progress, Recovery, Resolution, Status, Step,
        Wizard,
    };
    use crate::config::env::EnvFile;
    use crate::config::Protocols;
    use crate::journal::{Action, Change, Journal, Kind, Undo};
    use crate::platform::Environment;

    /// A wizard on a platform where every step applies (native Linux asks for the
    /// container user; everything is on the table).
    fn on_native_linux() -> Wizard {
        Wizard::new(Environment::LinuxNative)
    }

    /// A wizard on a platform where the container user is not asked (macOS maps
    /// ownership away) but native Jellyfin is offered.
    fn on_macos() -> Wizard {
        Wizard::new(Environment::MacOs)
    }

    /// Answer every applicable question, so the wizard is ready for review.
    fn answer_all(wizard: &mut Wizard) {
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
        wizard.answer(Answer::Autostart(false)).unwrap_or(());
    }

    #[test]
    fn a_fresh_wizard_starts_at_welcome_with_nothing_answered() {
        let wizard = on_native_linux();
        assert_eq!(wizard.at(), Step::Welcome);
        assert_eq!(wizard.answers(), &super::Answers::default());
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
            Err(super::Rejected::ServiceUserNotApplicable)
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
            Err(super::Rejected::NativeJellyfinUnavailable)
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

    /// The value a plan records for a key, if any.
    fn setting<'a>(plan: &'a super::Plan, key: &str) -> Option<&'a str> {
        plan.settings()
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn a_reviewed_wizard_plans_every_setting_it_gathered() {
        let mut wizard = on_native_linux();
        answer_all(&mut wizard);
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
        assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("on"));
        assert_eq!(setting(&plan, "DATA_ROOT"), Some("/srv/media"));
        assert_eq!(setting(&plan, "PUID"), Some("1000"));
        // Distinct from PUID, so a swapped mapping would not pass unnoticed.
        assert_eq!(setting(&plan, "PGID"), Some("1001"));
        assert_eq!(setting(&plan, "JELLYFIN_MODE"), Some("docker"));
    }

    #[test]
    fn an_unanswered_question_contributes_no_setting() {
        // A fresh wizard writes nothing; a partly answered one writes only what it
        // has, never a guessed default for what it does not.
        assert!(on_native_linux().plan().settings().is_empty());

        let mut wizard = on_native_linux();
        wizard
            .answer(Answer::Protocols(Protocols {
                usenet: true,
                torrent: false,
            }))
            .unwrap_or(());
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
        // A declined protocol is written off, not omitted.
        assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("off"));
        assert_eq!(setting(&plan, "DATA_ROOT"), None);
        assert_eq!(setting(&plan, "JELLYFIN_MODE"), None);
    }

    #[test]
    fn a_declined_container_user_writes_no_ids() {
        let mut wizard = on_native_linux();
        wizard.answer(Answer::ServiceUser(None)).unwrap_or(());
        // The step is answered — recorded as "no id" rather than left open — and
        // that answered-with-nothing still writes neither id.
        assert_eq!(wizard.answers().service_user, Some(None));
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "PUID"), None);
        assert_eq!(setting(&plan, "PGID"), None);
    }

    #[test]
    fn the_library_choice_maps_to_its_mode_or_to_nothing() {
        let mut docker = on_native_linux();
        docker
            .answer(Answer::Library(Library::JellyfinDocker))
            .unwrap_or(());
        assert_eq!(setting(&docker.plan(), "JELLYFIN_MODE"), Some("docker"));

        let mut native = on_macos();
        native
            .answer(Answer::Library(Library::JellyfinNative))
            .unwrap_or(());
        assert_eq!(setting(&native.plan(), "JELLYFIN_MODE"), Some("native"));

        let mut none = on_native_linux();
        none.answer(Answer::Library(Library::None)).unwrap_or(());
        assert_eq!(setting(&none.plan(), "JELLYFIN_MODE"), None);
    }

    /// A saved progress sitting at a given lifecycle phase.
    fn saved_at(phase: Phase) -> Progress {
        Progress {
            phase,
            ..Progress::default()
        }
    }

    /// One `.env` write an apply made: a new key, with nothing there before.
    fn wrote(key: &str, value: &str) -> Change {
        Change {
            at: "t".to_owned(),
            operation: "apply".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: None,
                current: value.to_owned(),
            },
        }
    }

    /// The reversal of writing a fresh `.env` key: remove it again.
    fn removed(key: &str) -> Undo {
        Undo {
            target: ".env".to_owned(),
            action: Action::Restore {
                key: key.to_owned(),
                value: None,
            },
        }
    }

    /// The two writes an apply had managed to make before it was interrupted.
    fn partial_apply() -> Journal {
        Journal::replay(vec![
            wrote("DATA_ROOT", "/srv/media"),
            wrote("USENET", "on"),
        ])
    }

    #[test]
    fn a_fresh_setup_is_in_the_gathering_phase() {
        assert_eq!(Phase::default(), Phase::InProgress);
        assert_eq!(Progress::default().phase, Phase::InProgress);
    }

    #[test]
    fn a_progress_file_predating_the_phase_field_reads_as_gathering() {
        // A file written before the lifecycle was tracked carries no phase; it was
        // only ever left mid-gathering, so it must read back as that rather than
        // fail to load.
        let old = r#"{"at":"protocols","answers":{}}"#;
        let restored = serde_json::from_str::<Progress>(old).ok();
        assert_eq!(
            restored.map(|progress| (progress.phase, progress.at)),
            Some((Phase::InProgress, Step::Protocols)),
        );
    }

    #[test]
    fn the_phase_survives_a_round_trip() {
        for phase in [
            Phase::InProgress,
            Phase::Reviewing,
            Phase::Applying,
            Phase::Applied,
        ] {
            let line = serde_json::to_string(&saved_at(phase)).unwrap_or_default();
            let read = serde_json::from_str::<Progress>(&line).ok();
            assert_eq!(read.map(|progress| progress.phase), Some(phase), "{line}");
        }
    }

    #[test]
    fn no_saved_setup_is_absent() {
        assert_eq!(Status::of(None), Status::Absent);
    }

    #[test]
    fn each_stored_phase_maps_to_its_status() {
        assert_eq!(
            Status::of(Some(&saved_at(Phase::InProgress))),
            Status::InProgress,
        );
        assert_eq!(
            Status::of(Some(&saved_at(Phase::Reviewing))),
            Status::Reviewing,
        );
        assert_eq!(Status::of(Some(&saved_at(Phase::Applied))), Status::Applied);
    }

    #[test]
    fn a_persisted_applying_marker_is_a_failed_apply() {
        // The only writer of the applying marker is a live apply, so reading it
        // back off disk means that apply stopped before it finished.
        assert_eq!(
            Status::of(Some(&saved_at(Phase::Applying))),
            Status::FailedApply,
        );
    }

    #[test]
    fn recovery_reports_exactly_what_the_interrupted_apply_wrote() {
        // The report is the journal's writes unaltered and in order — the data
        // root first, then the protocol toggle.
        let journal = partial_apply();
        assert_eq!(
            Recovery::of(&journal).written(),
            [wrote("DATA_ROOT", "/srv/media"), wrote("USENET", "on")],
        );
    }

    #[test]
    fn resuming_keeps_every_write_already_made() {
        let journal = partial_apply();
        assert_eq!(
            Recovery::of(&journal).resolve(Choice::Resume),
            Resolution::Resume,
        );
    }

    #[test]
    fn rolling_back_reverses_the_writes_most_recent_first() {
        // The later write is undone before the earlier one, and each set with no
        // prior value is removed — the reversal of a two-step partial apply.
        let journal = partial_apply();
        assert_eq!(
            Recovery::of(&journal).resolve(Choice::RollBack),
            Resolution::RollBack(vec![removed("USENET"), removed("DATA_ROOT")]),
        );
    }

    #[test]
    fn starting_over_also_reverses_the_writes_before_discarding_them() {
        // Start over is not a bare discard: the partial apply is unwound first,
        // most recent write to earliest, so nothing is stranded on disk — the
        // reversal is the same as a roll back's; only what follows it differs.
        let journal = partial_apply();
        assert_eq!(
            Recovery::of(&journal).resolve(Choice::StartOver),
            Resolution::StartOver(vec![removed("USENET"), removed("DATA_ROOT")]),
        );
    }

    #[test]
    fn a_setup_moves_review_to_apply_to_applied_along_its_edges() {
        let mut wizard = on_native_linux();
        answer_all(&mut wizard);
        assert_eq!(wizard.phase(), Phase::InProgress);
        assert!(wizard.transition(Phase::Reviewing));
        assert!(wizard.transition(Phase::Applying));
        assert!(wizard.transition(Phase::Applied));
        assert_eq!(wizard.phase(), Phase::Applied);
        // A finished apply cannot be re-run, nor walked back to an earlier phase,
        // so a reached phase is never quietly rewritten.
        assert!(!wizard.transition(Phase::Applying));
        assert!(!wizard.transition(Phase::Reviewing));
        assert_eq!(wizard.phase(), Phase::Applied);
    }

    #[test]
    fn apply_cannot_be_reached_without_passing_review() {
        // The gate review stands for — every question answered — cannot be skipped
        // by jumping a fully-answered wizard straight to applying.
        let mut wizard = on_native_linux();
        answer_all(&mut wizard);
        assert!(!wizard.transition(Phase::Applying));
        assert!(!wizard.transition(Phase::Applied));
        assert_eq!(wizard.phase(), Phase::InProgress);
    }

    #[test]
    fn review_is_refused_until_every_question_is_answered() {
        let mut wizard = on_native_linux();
        assert!(!wizard.transition(Phase::Reviewing));
        assert_eq!(wizard.phase(), Phase::InProgress);

        answer_all(&mut wizard);
        assert!(wizard.transition(Phase::Reviewing));
        assert_eq!(wizard.phase(), Phase::Reviewing);
    }

    #[test]
    fn a_rolled_back_apply_returns_to_review_to_be_run_again() {
        // The one backward edge: an apply that was unwound goes back to review,
        // its answers intact, ready to apply once more.
        let mut wizard = on_native_linux();
        answer_all(&mut wizard);
        assert!(wizard.transition(Phase::Reviewing));
        assert!(wizard.transition(Phase::Applying));
        assert!(wizard.transition(Phase::Reviewing));
        assert_eq!(wizard.phase(), Phase::Reviewing);
    }

    #[test]
    fn applying_the_plan_records_each_setting_against_what_was_there() {
        let mut wizard = on_native_linux();
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        // The file already carries a usenet setting and nothing about torrents, so
        // one change has a previous value to restore and the other has none.
        let current = EnvFile::parse("LEMONFIBER_USENET=off\n");
        let set = |key: &str, previous: Option<&str>, value: &str| Change {
            at: "t".to_owned(),
            operation: "apply".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: previous.map(str::to_owned),
                current: value.to_owned(),
            },
        };
        assert_eq!(
            wizard.plan().changes(&current, "t"),
            vec![
                set("LEMONFIBER_USENET", Some("off"), "on"),
                set("LEMONFIBER_TORRENT", None, "on"),
            ],
        );
    }

    #[test]
    fn a_setting_that_was_present_but_empty_is_captured_as_empty_not_absent() {
        // An empty prior value is a value: rolling the write back must restore the
        // empty string, so `changes` records `Some("")`, distinct from the `None`
        // of a key that was never there.
        let mut wizard = on_native_linux();
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        let current = EnvFile::parse("LEMONFIBER_USENET=\n");
        let set = |key: &str, previous: Option<&str>, value: &str| Change {
            at: "t".to_owned(),
            operation: "apply".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: previous.map(str::to_owned),
                current: value.to_owned(),
            },
        };
        assert_eq!(
            wizard.plan().changes(&current, "t"),
            vec![
                set("LEMONFIBER_USENET", Some(""), "on"),
                set("LEMONFIBER_TORRENT", None, "on"),
            ],
        );
    }
}

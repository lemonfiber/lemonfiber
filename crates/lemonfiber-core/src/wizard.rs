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

use crate::config::env::EnvFile;
use crate::config::{
    Protocols, DATA_ROOT_KEY, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, INDEXER_VALIDATED_KEY,
    JELLYFIN_MODE_KEY, PGID_KEY, PROVIDER_HOST_KEY, PROVIDER_PASS_KEY, PROVIDER_PORT_KEY,
    PROVIDER_TLS_KEY, PROVIDER_USER_KEY, PROVIDER_VALIDATED_KEY, PUID_KEY, TORRENT_KEY, USENET_KEY,
};
use crate::journal::{Change, Journal, Kind, Undo};
use crate::platform::Environment;

mod answers;
mod plan;
mod recovery;
mod steps;

pub use answers::{
    Answer, Answers, Credentials, Indexer, Library, Provider, Rejected, Usenet, Vpn,
};
pub use plan::{on_off, Plan, APPLY, ENV_FILE};
pub use recovery::{described, Choice, Recovery, Resolution, Status};
pub use steps::{offer_setup, opened_by, Direction, Phase, Progress, Step};

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
    /// Only `ServiceUser` is conditional on the machine: the container user is worth
    /// asking about only where file ownership is real rather than mapped. Everything
    /// else turns on the protocols chosen, which is [`Step::wanted_by`]'s answer —
    /// the same one reconfiguration reads to say what adding a protocol opens, so the
    /// walk and the change cannot come to different lists. Nothing chosen yet reads as
    /// nothing configured, which is what leaves the three download steps out of a walk
    /// that has not reached the protocol question.
    #[must_use]
    pub const fn applies(&self, step: Step) -> bool {
        match step {
            Step::ServiceUser => self.environment.ownership_is_real(),
            asked => asked.wanted_by(match self.progress.answers.protocols {
                Some(protocols) => protocols,
                None => Protocols::none(),
            }),
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
            Answer::Vpn(vpn) => self.progress.answers.vpn = Some(vpn),
            Answer::DataLocation(path) => self.progress.answers.data_location = Some(path),
            Answer::Credentials(indexer) => {
                self.progress.answers.credentials = match indexer {
                    Some(indexer) => Credentials::Given(indexer),
                    None => Credentials::Empty,
                };
            }
            Answer::Provider(provider) => {
                self.progress.answers.usenet = match provider {
                    Some(provider) => Usenet::Given(provider),
                    None => Usenet::Empty,
                };
            }
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
            Answer::Notifications(appetite) => {
                self.progress.answers.notifications = Some(appetite);
            }
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
    pub(crate) const fn is_answered(&self, step: Step) -> bool {
        if !self.applies(step) {
            return true;
        }
        let answers = &self.progress.answers;
        match step {
            Step::Protocols => answers.protocols.is_some(),
            Step::Vpn => answers.vpn.is_some(),
            Step::DataLocation => answers.data_location.is_some(),
            Step::Credentials => !matches!(answers.credentials, Credentials::Unanswered),
            Step::Provider => !matches!(answers.usenet, Usenet::Unanswered),
            Step::ServiceUser => answers.service_user.is_some(),
            Step::Library => answers.library.is_some(),
            Step::Household => answers.household.is_some(),
            Step::Notifications => answers.notifications.is_some(),
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
    /// The household and autostart choices are not settings and so are not here:
    /// neither is a value Compose is handed, and the environment file is handed to
    /// Compose as it stands. The autostart answer is nonetheless kept — apply writes
    /// it to [`crate::config::paths::Paths::autostart`] beside the settings, where a
    /// backup carries it — because an answer gathered under a stated consequence and
    /// then discarded leaves the operator believing they decided something. The
    /// household choice is applied by its own feature.
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
        // Gated on the step still applying, not only on an indexer being held: an
        // operator who chose downloads, gave an indexer, then went back and chose
        // neither protocol leaves a `Given` answer the step no longer wants, and
        // its key must not be written for a stack that has no service to use it.
        if self.applies(Step::Credentials) {
            if let Credentials::Given(indexer) = &answers.credentials {
                settings.push((INDEXER_URL_KEY.to_owned(), indexer.url.clone()));
                settings.push((INDEXER_APIKEY_KEY.to_owned(), indexer.key.clone()));
                settings.push((INDEXER_VALIDATED_KEY.to_owned(), on_off(indexer.validated)));
            }
        }
        // Gated on the step still applying, so a provider given for a Usenet run
        // that was then changed to torrent-only leaves no stale login behind.
        if self.applies(Step::Provider) {
            if let Usenet::Given(provider) = &answers.usenet {
                settings.push((PROVIDER_HOST_KEY.to_owned(), provider.host.clone()));
                settings.push((PROVIDER_PORT_KEY.to_owned(), provider.port.to_string()));
                settings.push((PROVIDER_USER_KEY.to_owned(), provider.user.clone()));
                settings.push((PROVIDER_PASS_KEY.to_owned(), provider.pass.clone()));
                settings.push((PROVIDER_TLS_KEY.to_owned(), on_off(provider.tls)));
                settings.push((
                    PROVIDER_VALIDATED_KEY.to_owned(),
                    on_off(provider.validated),
                ));
            }
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

#[cfg(test)]
mod tests;

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

pub use answers::{Answer, Answers, Credentials, Indexer, Library, Provider, Rejected, Usenet};
pub use plan::{on_off, Plan, APPLY, ENV_FILE};
pub use recovery::{Choice, Recovery, Resolution, Status};
pub use steps::{offer_setup, Direction, Phase, Progress, Step};

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
            // Credentials are for the download services; a library-only run that
            // chose neither protocol has none to give, so the step is passed over.
            Step::Credentials => matches!(self.progress.answers.protocols, Some(p) if p.any()),
            // A Usenet provider is only for a Usenet run; a torrent-only or
            // library-only one has no provider to give.
            Step::Provider => matches!(self.progress.answers.protocols, Some(p) if p.usenet),
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
    pub const fn is_answered(&self, step: Step) -> bool {
        if !self.applies(step) {
            return true;
        }
        let answers = &self.progress.answers;
        match step {
            Step::Protocols => answers.protocols.is_some(),
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
mod tests {
    use crate::alert::Appetite;
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

    /// Answer every applicable question, so the wizard is ready for review.
    fn answer_all(wizard: &mut Wizard) {
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        wizard
            .answer(Answer::DataLocation(PathBuf::from("/srv/media")))
            .unwrap_or(());
        wizard.answer(Answer::Credentials(None)).unwrap_or(());
        wizard.answer(Answer::Provider(None)).unwrap_or(());
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
    fn a_given_indexer_is_planned_but_a_stale_one_is_dropped_when_it_no_longer_applies() {
        let indexer = Answer::Credentials(Some(super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        }));

        // Chosen with a download protocol, the indexer is written.
        let mut wizard = on_native_linux();
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        wizard.answer(indexer.clone()).unwrap_or(());
        let plan = wizard.plan();
        assert_eq!(
            setting(&plan, "INDEXER_URL"),
            Some("http://indexer.test/api")
        );
        assert_eq!(setting(&plan, "INDEXER_APIKEY"), Some("the-key"));
        assert_eq!(setting(&plan, "INDEXER_VALIDATED"), Some("on"));

        // Then neither protocol is chosen, so the step no longer applies. The
        // answer lingers, but its key must not be written for a stack with no
        // service to use it.
        wizard
            .answer(Answer::Protocols(Protocols::none()))
            .unwrap_or(());
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "INDEXER_URL"), None);
        assert_eq!(
            setting(&plan, "INDEXER_APIKEY"),
            None,
            "no stale key is written"
        );
    }

    #[test]
    fn a_given_provider_is_planned_but_dropped_when_usenet_is_no_longer_chosen() {
        let provider = Answer::Provider(Some(super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "secret".to_owned(),
            tls: true,
            validated: true,
        }));

        // Chosen with Usenet, the provider login is written.
        let mut wizard = on_native_linux();
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        wizard.answer(provider.clone()).unwrap_or(());
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "USENET_HOST"), Some("news.provider.test"));
        assert_eq!(setting(&plan, "USENET_PORT"), Some("563"));
        assert_eq!(setting(&plan, "USENET_USER"), Some("person"));
        assert_eq!(setting(&plan, "USENET_VALIDATED"), Some("on"));

        // Then torrents only, so the provider step no longer applies; its login,
        // password and all, must not be written for a stack that will not use it.
        wizard
            .answer(Answer::Protocols(Protocols {
                usenet: false,
                torrent: true,
            }))
            .unwrap_or(());
        let plan = wizard.plan();
        assert_eq!(setting(&plan, "USENET_HOST"), None);
        assert_eq!(
            setting(&plan, "USENET_PASS"),
            None,
            "no stale password is written"
        );
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

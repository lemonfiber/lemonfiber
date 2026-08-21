//! The one way in.
//!
//! A surface turns input into a [`Command`], hands it to [`dispatch`], and
//! renders the [`Outcome`]. A keypress, a subcommand and an HTTP route all
//! become the same value, so the three surfaces cannot grow behaviour that only
//! one of them has — which is the drift that otherwise happens quietly, one
//! convenience flag at a time.
//!
//! Whether this is a rehearsal is a property of the [`Ctx`], not a second code
//! path, so there is no parallel implementation to fall out of step.

use crate::audio::Format;
use crate::doctor::Category;
use crate::error::{Code, Problem};
use crate::model::{
    ConfigReport, DoctorReport, Envelope, FormsReport, HouseholdReport, LifecycleReport,
    MusicReport, QualityReport, ResetReport, StatusReport, StuckReport, TraceReport, UpgradeReport,
    VersionReport,
};
use crate::quality::Preset;
use crate::stack::closure::Plan;
use crate::stack::compose::Action;

pub mod accepted;
pub mod appetite;
pub mod apply;
pub mod backup;
pub mod bundle;
pub mod conditions;
mod configuring;
mod ctx;
pub mod dashboard;
pub mod egress;
mod engine;
#[cfg(test)]
mod fixtures;
pub mod forwarding;
mod household;
mod materialise;
mod music;
mod notify;
mod outbox;
mod quality;
pub mod queue;
mod record;
pub mod recover;
pub mod repair;
mod repairs;
mod reset;
pub mod restore;
mod screen;
mod seed;
pub mod seeding;
pub mod setup;
mod targets;
mod trace;
mod upgrade;
mod walkthrough;
pub mod watch;

pub use ctx::Ctx;

// The log-following reads a surface streams from live outside dispatch, so they are the
// engine module's functions re-exported for the binary and the log commands to reach.
pub use engine::{diagnose, logs, pull_progress};
pub use notify::{notify, Notified, CHANNEL_CHECK};
pub use walkthrough::{walkthrough, worth_offering};

// The data-location watch is a self-contained feature in its own module; these
// are the names the rest of the crate and the binary reach it by.
pub use watch::{supervise, ALREADY_GONE, NOTHING_TO_WATCH, WATCH};

/// What a surface is asking for.
///
/// Deliberately exhaustive. The surfaces ship in the same binary, so a new
/// command should stop the build until every surface has decided what to do
/// with it — silently rendering nothing is the failure this prevents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Report the binary's version, and the engine's where it can be reached.
    Version,
    /// List the forms this stack declares.
    Forms,
    /// Say what naming these forms would come to, without running anything.
    Preview {
        /// The forms to resolve, as they were named.
        forms: Vec<String>,
    },
    /// Start one or more forms.
    Up {
        /// The forms to start, resolved to the union of their closures.
        forms: Vec<String>,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop.
        forms: Vec<String>,
    },
    /// Restart services without touching the rest.
    Restart {
        /// The forms holding those services.
        forms: Vec<String>,
        /// The services to restart; empty restarts the whole form.
        services: Vec<String>,
    },
    /// Fetch newer images without applying them.
    Pull {
        /// The forms whose images to fetch.
        forms: Vec<String>,
    },
    /// Read one setting.
    ConfigGet {
        /// The setting to read.
        key: String,
    },
    /// Change one setting.
    ConfigSet {
        /// The setting to change.
        key: String,
        /// What to change it to.
        value: String,
    },
    /// Show every setting, with credentials withheld.
    ConfigShow,
    /// Report what each service is actually doing.
    Ps {
        /// The forms to report on; empty reports on the whole stack.
        forms: Vec<String>,
    },
    /// Run the diagnostic checks, or one category of them.
    Doctor {
        /// The category to run; empty runs every check there is.
        only: Option<Category>,
        /// Whether the operator opted into the checks that disturb the system.
        disruptive: bool,
        /// A check whose warning the operator is answering: they have weighed the
        /// cost and chosen it, so it stops leading from now on.
        accept: Option<String>,
    },
    /// Show or change the quality preset — how good media should look, and how
    /// much disk it should cost — in plain language.
    Quality(QualityAction),
    /// Upgrade existing content to the chosen preset — a separate, explicit action
    /// whose bandwidth cost is stated, and which does nothing until confirmed. Its
    /// own command rather than a quality action because it reaches the services
    /// asynchronously, where the others only read and write the recorded choice.
    QualityUpgrade {
        /// Whether the operator confirmed the cost; without it, only the cost is
        /// stated and nothing is triggered.
        confirm: bool,
    },
    /// Choose the audio format for music — media with no resolution — and apply it to
    /// the music service. Its own command, like the upgrade, because it reaches the
    /// service asynchronously rather than only recording a choice.
    QualityMusic {
        /// The audio format to record and apply.
        format: Format,
    },
    /// Follow one item across the services and report where it is — "where is my
    /// show?" — searched for by a human term rather than an internal id.
    Trace {
        /// The show, film, or request to follow.
        term: String,
        /// The season to narrow the per-part coverage to, or every season where absent.
        season: Option<u32>,
    },
    /// Report what the household has asked for and where each request stands, in the
    /// words the member who asked would use rather than the services' own.
    Household {
        /// The member to narrow to, or every member where absent.
        member: Option<String>,
    },
    /// List the items whose downloads are stuck, each named so it links to its own
    /// trace — the landing point for "N items stuck".
    Stuck,
    /// Wire the stack's services to each other, idempotently.
    Seed,
    /// Adopt the operator's current edits as lemonfiber's expected state, so they
    /// stop reporting as drift and are kept across future seeds and restores. Wires
    /// what is missing as a seed does, and promotes every drifted value to adopted.
    Adopt,
    /// Put the stack back to lemonfiber's own state, reverting every operator edit — the
    /// opposite of adopt. Because it discards their work, it names what will be lost and
    /// does nothing until confirmed: unconfirmed it only previews the reverts.
    Reset {
        /// Whether the operator confirmed the loss; without it, only the reverts are
        /// shown and nothing is written.
        confirm: bool,
    },
}

/// What a quality command asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityAction {
    /// Show the choice in force and what each preset means and costs.
    Show,
    /// Choose a preset — for everything, or for one media type — and record it.
    Set {
        /// The preset to choose.
        preset: Preset,
        /// The media type it applies to, or the whole library where absent.
        media_type: Option<String>,
        /// Whether the operator confirmed a choice this host would have to
        /// transcode in software, which is otherwise held rather than recorded.
        confirm: bool,
    },
    /// Re-assert the recorded preset over a hand-edited Recyclarr config — the
    /// explicit consent to let the preset win where a run would preserve the edit.
    Reapply,
}

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
    /// The answer to [`Command::Forms`].
    Forms(FormsReport),
    /// The answer to [`Command::Preview`].
    Preview(Plan),
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport),
    /// The answer to a configuration command.
    Config(ConfigReport),
    /// The quality choice, what it means, and what a command did with it.
    Quality(QualityReport),
    /// What upgrading existing content did, or would do, and its stated cost.
    Upgrade(UpgradeReport),
    /// The music format chosen, and what became of applying it.
    Music(MusicReport),
    /// Where one item is in the pipeline.
    Trace(TraceReport),
    /// What the household asked for, member by member.
    Household(HouseholdReport),
    /// The items whose downloads are stuck, each linkable to its trace.
    Stuck(StuckReport),
    /// What each service is doing.
    Status(StatusReport),
    /// What the diagnostic checks found.
    Doctor(DoctorReport),
    /// What seeding wired, and what it left for a re-run.
    Seed(crate::seed::Report),
    /// What a full reset did, or would do — the operator edits reverted to lemonfiber's.
    Reset(ResetReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => "version",
            Self::Forms(_) => "forms",
            Self::Preview(_) => "preview",
            Self::Lifecycle(_) => "lifecycle",
            Self::Config(_) => "config",
            Self::Quality(_) => "quality",
            Self::Upgrade(_) => "upgrade",
            Self::Music(_) => "music",
            Self::Trace(_) => "trace",
            Self::Household(_) => "household",
            Self::Stuck(_) => "stuck",
            Self::Status(_) => "status",
            Self::Doctor(_) => "doctor",
            Self::Seed(_) => "seed",
            Self::Reset(_) => "reset",
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
            Self::Forms(report) => report.serialize(serializer),
            Self::Preview(plan) => plan.serialize(serializer),
            Self::Lifecycle(report) => report.serialize(serializer),
            Self::Config(report) => report.serialize(serializer),
            Self::Quality(report) => report.serialize(serializer),
            Self::Upgrade(report) => report.serialize(serializer),
            Self::Music(report) => report.serialize(serializer),
            Self::Trace(report) => report.serialize(serializer),
            Self::Household(report) => report.serialize(serializer),
            Self::Stuck(report) => report.serialize(serializer),
            Self::Status(report) => report.serialize(serializer),
            Self::Doctor(report) => report.serialize(serializer),
            Self::Seed(report) => report.serialize(serializer),
            Self::Reset(report) => report.serialize(serializer),
        }
    }
}

/// Raised when a service never reached a state that starting could accept.
pub const NEVER_SETTLED: Code = Code::new("LIFE-1");

/// Carry out a command.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    match command {
        Command::Version => engine::version(ctx).await.map(Outcome::Version),
        Command::Forms => engine::forms(ctx).map(Outcome::Forms),
        Command::Preview { forms } => engine::preview(ctx, &forms).map(Outcome::Preview),
        Command::Up { forms } => engine::lifecycle(ctx, &forms, &Action::Up).await,
        Command::Down { forms } => engine::lifecycle(ctx, &forms, &Action::Down).await,
        Command::Restart { forms, services } => {
            engine::lifecycle(ctx, &forms, &Action::Restart(services)).await
        }
        Command::Pull { forms } => engine::lifecycle(ctx, &forms, &Action::Pull).await,
        Command::ConfigGet { key } => configuring::configuration(ctx, Some(&key), None),
        Command::ConfigSet { key, value } => {
            configuring::configuration(ctx, Some(&key), Some(&value))
        }
        Command::ConfigShow => configuring::configuration(ctx, None, None),
        Command::Quality(action) => quality::quality(ctx, action).map(Outcome::Quality),
        Command::QualityMusic { format } => music::music(ctx, format).await.map(Outcome::Music),
        Command::Trace { term, season } => {
            trace::trace(ctx, &term, season).await.map(Outcome::Trace)
        }
        Command::Household { member } => household::household(ctx, member.as_deref())
            .await
            .map(Outcome::Household),
        Command::Stuck => trace::stuck(ctx).await.map(Outcome::Stuck),
        Command::QualityUpgrade { confirm } => {
            upgrade::upgrade(ctx, confirm).await.map(Outcome::Upgrade)
        }
        Command::Ps { forms } => engine::status(ctx, &forms).await.map(Outcome::Status),
        Command::Doctor {
            only,
            disruptive,
            accept,
        } => {
            let report = engine::diagnose(ctx, only, disruptive).await?;
            accepted::acknowledge(ctx, accept.as_deref(), report).map(Outcome::Doctor)
        }
        Command::Seed => seed::seed(ctx, false).await.map(Outcome::Seed),
        Command::Adopt => seed::seed(ctx, true).await.map(Outcome::Seed),
        Command::Reset { confirm } => reset::reset(ctx, confirm).await.map(Outcome::Reset),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        dispatch, pull_progress, Category, Command, Ctx, Outcome, QualityAction, VersionReport,
    };
    use crate::config::Settings;
    use crate::docker::{Condition, State as ServiceState};
    use crate::ports::docker::{Engine, Failure as EngineFailure, Health, Lifecycle, LogQuery};
    use crate::ports::process::{Failure, Output, Progress};
    use crate::quality::Preset;
    use crate::stack::Source;
    use crate::test_support::{a_context, nowhere, refused, spoke, Recording, Reporting, Scripted};
    use lemonfiber_fixtures::http::Fake;
    use std::time::Duration;

    fn ctx(scripted: Result<Output, Failure>) -> Ctx {
        a_context()
            .runner(Arc::new(Scripted(scripted)))
            .engine(Arc::new(Reporting::default()))
            .build()
    }

    #[tokio::test]
    async fn a_dispatched_quality_show_serialises_under_its_own_kind() {
        // Through dispatch, a quality command reaches its outcome, envelope and
        // serialisation — the arms the handler's own tests, calling it directly,
        // never touch. With no config the choice is the default, shown.
        let json = dispatch(Command::Quality(QualityAction::Show), &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"quality\""),
            "envelope names the kind"
        );
        assert!(json.contains("everything"), "the global choice is reported");
    }

    #[tokio::test]
    async fn a_dispatched_quality_set_with_nowhere_to_record_is_an_error() {
        // The dispatch arm unboxes the handler's error: a set with no configured
        // env file has nowhere to write the choice, so it fails rather than lying.
        let refused = dispatch(
            Command::Quality(QualityAction::Set {
                preset: Preset::Balanced,
                media_type: None,
                confirm: false,
            }),
            &ctx(Ok(spoke(""))),
        )
        .await;
        assert!(
            refused.is_err(),
            "a set with nowhere to record cannot succeed"
        );
    }

    #[tokio::test]
    async fn a_dispatched_trace_serialises_under_its_own_kind() {
        // No key opens a target, so no item matches and the trace stays offline while
        // exercising the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(
            Command::Trace {
                term: "the expanse".to_owned(),
                season: None,
            },
            &ctx(Ok(spoke(""))),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"trace\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_household_serialises_under_its_own_kind() {
        // Nothing is recorded to sign in with, so the view reports itself unavailable —
        // which still exercises the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(Command::Household { member: None }, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"household\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_stuck_serialises_under_its_own_kind() {
        // No key opens a target, so nothing is stuck — but the command still runs through
        // dispatch, envelope and serialise for its outcome.
        let json = dispatch(Command::Stuck, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"stuck\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_reset_serialises_under_its_own_kind() {
        // The test stack is external, so a reset reverts nothing — but the command still
        // runs through dispatch, envelope and serialise for its outcome.
        let json = dispatch(Command::Reset { confirm: false }, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"reset\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_music_choice_serialises_under_its_own_kind() {
        // A rehearsal records nothing and reaches no service, so it stays offline while
        // exercising the dispatch, envelope and serialise arms for its outcome.
        let mut context = ctx(Ok(spoke("")));
        context.dry_run = true;
        let json = dispatch(
            Command::QualityMusic {
                format: crate::audio::Format::Lossless,
            },
            &context,
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"music\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_quality_upgrade_serialises_under_its_own_kind() {
        // Unconfirmed, it states the cost and reaches no service, so it stays offline
        // while exercising the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(
            Command::QualityUpgrade { confirm: false },
            &ctx(Ok(spoke(""))),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"upgrade\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_upgrade_over_an_unreadable_stack_is_an_error() {
        // The dispatch arm unboxes the driver's error: a confirmed upgrade cannot read
        // an unreadable stack's services, so it fails rather than half-acting.
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere())
            .build();
        assert!(dispatch(Command::QualityUpgrade { confirm: true }, &ctx)
            .await
            .is_err());
    }

    /// What a version report looks like when the engine said `compose`.
    ///
    /// Tests assert against the whole outcome rather than picking it apart. A
    /// destructuring assertion needs a branch for the case that cannot happen,
    /// and that branch is a line no test can ever cover.
    fn reported(compose: Option<&str>) -> Outcome {
        Outcome::Version(VersionReport {
            binary: env!("CARGO_PKG_VERSION").to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: compose.map(str::to_owned),
        })
    }

    /// Forms come from the stack rather than from lemonfiber, so this reports what the
    /// manifest declares — including whether each one may be combined, which is what an
    /// operator choosing between two of them needs to know before they try.
    #[tokio::test]
    async fn lists_the_forms_the_stack_declares_in_its_own_words() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        let listed = dispatch(Command::Forms, &ctx).await;

        assert!(matches!(&listed, Ok(Outcome::Forms(report))
            if report.forms.len() > 1
                && report
                    .forms
                    .iter()
                    .any(|form| form.id == "search" && form.name == "Search" && form.composable)));
    }

    /// Also driven from `tests/forms.rs`, against the real stack. Kept here as well
    /// because this crate is compiled twice — once with its own test modules and once as
    /// the library those binaries link — and a command dispatched from only one of them
    /// leaves the other's copy of the arm counted as never run.
    #[tokio::test]
    async fn a_preview_is_dispatched_like_any_other_command() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        let previewed = dispatch(
            Command::Preview {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await;

        assert!(
            matches!(&previewed, Ok(Outcome::Preview(plan))
                if plan.services.contains(&"jellyfin".to_owned())),
            "{previewed:?}"
        );
        assert_eq!(
            previewed.ok().map(|outcome| outcome.envelope().kind),
            Some("preview"),
            "the kind names the question that was asked"
        );
    }

    #[tokio::test]
    async fn reports_the_engine_version_when_the_engine_answers() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        assert_eq!(
            dispatch(Command::Version, &ctx).await,
            Ok(reported(Some("v2.32.1")))
        );
    }

    #[tokio::test]
    async fn still_answers_when_the_engine_is_missing() {
        let ctx = ctx(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn treats_an_engine_that_fails_as_one_that_did_not_answer() {
        let ctx = ctx(Ok(refused("permission denied")));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_unusable_engine_is_also_reported_as_absent() {
        let ctx = ctx(Err(Failure::Unusable {
            program: "docker".to_owned(),
            reason: "denied".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_outcome_serialises_inside_the_versioned_envelope() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        let rendered = dispatch(Command::Version, &ctx)
            .await
            .ok()
            .and_then(|outcome| outcome.envelope().to_json());
        assert_eq!(
            rendered.as_deref(),
            Some(concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":""#,
                env!("CARGO_PKG_VERSION"),
                r#"","supported_schema":[1],"stack":"0.1.0","compose":"v2.32.1"}}"#
            ))
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_reported_rather_than_left_out() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("v2.32.1")))))
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let refusal = dispatch(Command::Version, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them"
        );
    }

    /// A context that runs against the checked-out stack, in rehearsal.
    fn rehearsing(protocols: crate::config::Protocols) -> Ctx {
        let settings = Settings {
            protocols,
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build()
            .rehearsing()
    }

    fn report(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<crate::model::LifecycleReport> {
        match outcome {
            Ok(Outcome::Lifecycle(report)) => Some(report),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::Stuck(_)
                | Outcome::Status(_)
                | Outcome::Doctor(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_),
            )
            | Err(_) => None,
        }
    }

    fn diagnosis(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<crate::model::DoctorReport> {
        match outcome {
            Ok(Outcome::Doctor(report)) => Some(report),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::Stuck(_)
                | Outcome::Status(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn doctor_runs_the_checks_and_reports_them_in_the_envelope() {
        // The engine here does not host the torrent pair, so the findings are
        // not green — but dispatch's job is only to run the checks and hand back
        // what they found, named in the machine-readable envelope.
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let command = Command::Doctor {
            only: Some(Category::Vpn),
            disruptive: false,
            accept: None,
        };
        let outcome = dispatch(command, &ctx).await;

        let json = outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.clone().envelope().to_json());
        assert!(
            json.as_deref()
                .is_some_and(|json| json.contains(r#""kind":"doctor""#)
                    && json.contains(r#""category":"vpn""#)),
            "the doctor envelope should name itself and carry vpn findings: {json:?}"
        );

        let report = diagnosis(outcome);
        assert!(report.is_some_and(|report| !report.findings.is_empty()
            && report
                .findings
                .iter()
                .all(|finding| finding.category == Category::Vpn)));
    }

    #[tokio::test]
    async fn a_full_doctor_run_includes_the_quality_guide_check() {
        // The guide-source check is wired into the suite: an unfiltered run carries
        // its finding. The ctx's offline http makes it unverified rather than
        // reaching the real upstream.
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let outcome = dispatch(
            Command::Doctor {
                only: None,
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;

        let names = diagnosis(outcome)
            .map(|report| {
                report
                    .findings
                    .into_iter()
                    .map(|finding| finding.check)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(
            names.iter().any(|check| check == "services.quality-guides"),
            "the guide-source check should appear in a full run: {names:?}"
        );
    }

    #[tokio::test]
    async fn doctor_reports_an_unreadable_stack_rather_than_guessing() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("v2.32.1")))))
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let outcome = dispatch(
            Command::Doctor {
                only: None,
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
        assert!(diagnosis(outcome).is_none());
    }

    #[tokio::test]
    async fn starting_a_form_reports_what_it_would_run_and_runs_nothing() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.action,
                report.plan.profiles.into_iter().collect::<Vec<String>>(),
                report.rehearsed,
                report.status,
                report.command.last().cloned()
            )),
            Some((
                "up".to_owned(),
                vec!["media".to_owned()],
                true,
                None,
                Some("--detach".to_owned())
            )),
            "a rehearsal reports the command and never ran it"
        );
    }

    #[tokio::test]
    async fn what_the_configuration_left_out_is_reported_rather_than_dropped() {
        let ctx = rehearsing(crate::config::Protocols {
            usenet: true,
            torrent: false,
        });
        let command = Command::Up {
            forms: vec!["tv".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| report.plan.dropped),
            Some(vec![crate::stack::closure::Dropped {
                profile: "torrent".to_owned(),
                needs: lemonfiber_manifest::Protocol::Torrent,
            }]),
            "the operator hears which service is missing, and why"
        );
    }

    #[tokio::test]
    async fn a_real_run_reports_how_the_command_exited() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();
        let command = Command::Down {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.rehearsed, report.status)),
            Some(("down".to_owned(), false, Some(0)))
        );
    }

    #[tokio::test]
    async fn an_engine_that_will_not_start_is_reported_to_the_operator() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();
        let command = Command::Pull {
            forms: vec!["library".to_owned()],
        };
        let refusal = dispatch(command, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn pull_progress_streams_composes_output_line_by_line_then_the_exit() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(Output {
                status: Some(0),
                stdout: "library Pulling\nlibrary Pulled\n".to_owned(),
                stderr: String::new(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();

        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);
        let mut progress = pull_progress(&ctx, &["library".to_owned()])
            .await
            .unwrap_or(silent);
        let mut lines = Vec::new();
        let mut status = None;
        while let Some(event) = progress.recv().await {
            match event {
                Progress::Line(line) => lines.push(line),
                Progress::Ended(code) => status = code,
            }
        }
        // Each of Compose's per-image lines arrives on the stream, then the exit —
        // what a surface renders as it happens rather than after.
        assert_eq!(
            lines,
            vec!["library Pulling".to_owned(), "library Pulled".to_owned()]
        );
        assert_eq!(status, Some(0));
    }

    #[tokio::test]
    async fn a_pull_that_cannot_spawn_compose_is_a_problem_not_a_stream() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();
        let refusal = pull_progress(&ctx, &["library".to_owned()])
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn restarting_names_the_services_and_nothing_else() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Restart {
            forms: vec!["library".to_owned()],
            services: vec!["jellyfin".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.command.last().cloned())),
            Some(("restart".to_owned(), Some("jellyfin".to_owned())))
        );
    }

    #[tokio::test]
    async fn a_form_this_stack_does_not_have_never_reaches_the_engine() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["telly".to_owned()],
        };
        let outcome = dispatch(command, &ctx).await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
        assert_eq!(report(outcome), None, "nothing ran, so there is no report");
    }

    #[tokio::test]
    async fn an_unreadable_stack_is_reported_before_anything_is_started() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn an_embedded_stack_with_nowhere_to_go_stops_before_starting_anything() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            stack_dir: None,
            ..Settings::default()
        };
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(Source::Embedded(&EMBEDDED))
            .settings(settings)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_NOT_SET_UP),
            "an operator who has not run setup is told to, not shown a path error"
        );
    }

    #[tokio::test]
    async fn a_stack_that_contradicts_itself_is_refused_with_every_fault_at_once() {
        let invalid = Source::External(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/invalid"
        )));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(invalid)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let problem = dispatch(command, &ctx).await.err();
        assert_eq!(
            problem.as_ref().map(|problem| problem.code),
            Some(crate::stack::STACK_INVALID)
        );

        let detail = problem
            .and_then(|problem| problem.detail)
            .unwrap_or_default();
        for expected in [
            "names profile telly, which is not declared",
            "that is not a pin",
            "not a recognised OSI identifier",
        ] {
            assert!(
                detail.contains(expected),
                "missing {expected:?} in: {detail}"
            );
        }
    }

    /// A context whose settings live in a scratch file.
    fn with_config(path: &std::path::Path) -> Ctx {
        let settings = Settings {
            env_file: Some(path.to_path_buf()),
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build()
    }

    fn config_scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    fn settings_of(outcome: Result<Outcome, Box<super::Problem>>) -> Option<Vec<(String, String)>> {
        match outcome {
            Ok(Outcome::Config(report)) => Some(
                report
                    .settings
                    .into_iter()
                    .map(|setting| (setting.key, setting.value))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::Stuck(_)
                | Outcome::Status(_)
                | Outcome::Doctor(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_setting_can_be_written_and_read_back() {
        let path = config_scratch("round-trip");
        let ctx = with_config(&path);

        let written = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_USENET".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(written),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let read = dispatch(
            Command::ConfigGet {
                key: "LEMONFIBER_USENET".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(read),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_rehearsed_change_reports_itself_and_writes_nothing() {
        let path = config_scratch("rehearsed");
        let ctx = with_config(&path).rehearsing();

        let outcome = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_TORRENT".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert!(
            matches!(&outcome, Ok(Outcome::Config(report)) if report.changed && report.rehearsed),
            "a rehearsal reports the change it would make, and that it was a rehearsal"
        );
        assert!(!path.exists(), "a rehearsal writes nothing");
    }

    #[tokio::test]
    async fn a_lifecycle_command_with_a_config_file_reports_no_edits_for_an_external_stack() {
        // With an environment file in hand, a lifecycle command derives where it would
        // keep the materialised-stack record beside it. The stack here is external —
        // the operator's own on disk — so nothing is written and no edit is reported.
        let path = config_scratch("lifecycle-config");
        let ctx = with_config(&path).rehearsing();
        let outcome = dispatch(
            Command::Up {
                forms: vec!["tv".to_owned()],
            },
            &ctx,
        )
        .await;
        let edits = report(outcome)
            .map(|report| report.stack_edits)
            .unwrap_or_default();
        assert!(
            edits.is_empty(),
            "an external stack is left as it is, so nothing is reported"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn showing_settings_withholds_credentials() {
        let path = config_scratch("secrets");
        let ctx = with_config(&path);
        for (key, value) in [("DATA_ROOT", "/media"), ("WIREGUARD_PRIVATE_KEY", "abc123")] {
            let _ = dispatch(
                Command::ConfigSet {
                    key: key.to_owned(),
                    value: value.to_owned(),
                },
                &ctx,
            )
            .await;
        }

        let shown = settings_of(dispatch(Command::ConfigShow, &ctx).await).unwrap_or_default();
        assert_eq!(
            shown,
            vec![
                ("DATA_ROOT".to_owned(), "/media".to_owned()),
                (
                    "WIREGUARD_PRIVATE_KEY".to_owned(),
                    crate::config::store::REDACTED.to_owned()
                ),
            ]
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_configuration_answer_serialises_under_its_own_kind() {
        let path = config_scratch("envelope");
        let ctx = with_config(&path);
        let _ = dispatch(
            Command::ConfigSet {
                key: "DATA_ROOT".to_owned(),
                value: "/media".to_owned(),
            },
            &ctx,
        )
        .await;

        let rendered = dispatch(Command::ConfigShow, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"config","data":{"settings":["#),
                json.contains(r#""changed":false"#)
            )),
            Some(("config", true, true))
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn asking_a_non_configuration_outcome_for_settings_gets_none() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(settings_of(dispatch(Command::Version, &ctx).await), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn settings_that_cannot_be_saved_reach_the_operator() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(format!("lemonfiber-app-{}-ro", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));

        let ctx = with_config(&dir.join(".env"));
        let refusal = dispatch(
            Command::ConfigSet {
                key: "A".to_owned(),
                value: "1".to_owned(),
            },
            &ctx,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_NOT_WRITTEN));

        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn settings_that_cannot_be_read_reach_the_operator() {
        // A file where the directory holding settings should be.
        let blocker =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-blocked", std::process::id()));
        let _ = std::fs::remove_dir_all(&blocker);
        let _ = std::fs::write(&blocker, "in the way");

        let ctx = with_config(&blocker.join(".env"));
        let refusal = dispatch(Command::ConfigShow, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_UNREADABLE));

        let _ = std::fs::remove_file(&blocker);
    }

    #[tokio::test]
    async fn settings_with_nowhere_to_live_say_setup_has_not_run() {
        let ctx = a_context().engine(Arc::new(Reporting::default())).build();
        assert_eq!(
            dispatch(Command::ConfigShow, &ctx)
                .await
                .err()
                .map(|p| p.code),
            Some(crate::config::store::CONFIG_NOWHERE)
        );
    }

    #[tokio::test]
    async fn a_lifecycle_outcome_serialises_under_its_own_kind() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"lifecycle","data":{"action":"up""#),
                json.contains(r#""rehearsed":true"#)
            )),
            Some(("lifecycle", true, true))
        );
    }

    /// A real run against an engine reporting whatever the test put in it.
    fn watching(engine: Reporting) -> Ctx {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(engine))
            .settings(settings)
            .build()
            // An HTTP port that answers nothing, so a diagnostic check reaching one — the
            // guide-source probe, a credential — resolves to unreachable rather than the
            // real network. Keeps the doctor tests self-contained and offline.
            .with_http(Fake::scripted(Vec::new()))
    }

    /// Everything the `library` form declares.
    const LIBRARY: [&str; 4] = [
        "jellyfin",
        "seerr",
        "calibre-web-automated",
        "audiobookshelf",
    ];

    #[tokio::test]
    async fn starting_waits_until_the_services_are_usable() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.condition,
                report.services.len(),
                report
                    .services
                    .iter()
                    .all(|service| service.state == ServiceState::Healthy)
            )),
            Some((Some(Condition::Active), LIBRARY.len(), true)),
            "started means every service answered, not that a process exists"
        );
    }

    #[tokio::test]
    async fn starting_keeps_asking_until_the_services_are_ready() {
        // Unsettled on the first two listings and healthy on the third, which
        // is what a stack that is genuinely starting looks like. A gate that
        // only ever read the engine once would pass this test by luck and fail
        // every real start.
        let engine =
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting).settling_after(2);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Active)),
            "waiting is the point: the answer changed while it waited"
        );
    }

    /// The requirement stated directly, and the only way to state it: "nothing was
    /// torn down" is a claim about commands that were never issued, so it is asserted
    /// against everything the runner was handed rather than against what came back.
    #[tokio::test]
    async fn one_service_failing_to_start_never_takes_down_the_rest() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(runner.clone())
            .engine(Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Starting,
            )))
            .settings(settings)
            .build()
            .with_http(Fake::scripted(Vec::new()))
            .waiting(Duration::ZERO);

        let refused = dispatch(
            Command::Up {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await
        .err();

        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::NEVER_SETTLED),
            "the start is reported as not having finished"
        );
        assert!(
            runner.ran("up"),
            "it did try to start the form, so the claim below is about a real run"
        );
        assert!(
            !runner.ran("down"),
            "and never tore it down again for the one service that would not settle"
        );
        assert!(!runner.ran("stop"), "nor stopped what had already started");
    }

    /// A report about a container is a report about the wrong thing. What the operator
    /// lost is what the stack says the service was there to do.
    #[tokio::test]
    async fn a_service_that_will_not_start_says_what_its_absence_costs() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let refused = dispatch(
            Command::Up {
                forms: vec!["library".to_owned()],
            },
            &watching(engine).waiting(Duration::ZERO),
        )
        .await
        .err();

        assert_eq!(
            refused.as_ref().map(|problem| problem
                .meaning
                .contains("Files on disk, no way to watch them")),
            Some(true),
            "the manifest's own words for what jellyfin is for: {refused:?}"
        );
        assert_eq!(
            refused
                .as_ref()
                .map(|problem| problem.meaning.contains("left alone")),
            Some(true),
            "and the operator is told the rest of the form was not taken down with it"
        );
    }

    #[tokio::test]
    async fn a_service_that_never_becomes_usable_stops_the_start_and_says_which() {
        // A container that is running but still inside its start period is
        // exactly the case a process check would have called success.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting)
            .saying("jellyfin", "Cannot open database, disk is full");
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::NEVER_SETTLED)
        );
        assert_eq!(
            refused
                .as_ref()
                .map(|problem| problem.summary.contains("jellyfin")),
            Some(true),
            "the operator is told which service, not that something went wrong"
        );
        assert_eq!(
            refused
                .and_then(|problem| problem.detail)
                .map(|detail| detail.contains("disk is full")),
            Some(true),
            "the explanation is already on screen rather than left to be found"
        );
    }

    #[tokio::test]
    async fn a_service_that_will_not_start_and_says_nothing_still_reports_which() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.map(|problem| (problem.code, problem.detail)),
            Some((super::NEVER_SETTLED, None)),
            "silence is reported as silence rather than as an empty quotation"
        );
    }

    #[tokio::test]
    async fn a_crash_loop_is_not_something_starting_waits_out() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Restarting, Health::None);
        let ctx = watching(engine).waiting(Duration::from_secs(3600));
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        // Patience of an hour, and this must still return at once: a loop has
        // settled, and waiting for it is waiting forever.
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Degraded))
        );
    }

    #[tokio::test]
    async fn stopping_does_not_wait_for_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Down {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.condition, report.services.is_empty())),
            Some((None, true)),
            "stopping is finished when Compose says so"
        );
    }

    #[tokio::test]
    async fn a_compose_invocation_that_failed_is_not_then_waited_on() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(refused("no such image")))))
            .engine(Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Starting,
            )))
            .settings(settings)
            .build()
            .waiting(Duration::ZERO);

        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.status, report.condition)),
            Some((Some(1), None)),
            "waiting for health after Compose refused would report the wrong fault"
        );
    }

    #[tokio::test]
    async fn starting_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::ports::docker::ENGINE_UNREACHABLE)
        );
    }

    /// The status a survey produced, as pairs of service and state.
    fn stated(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<Vec<(String, ServiceState)>> {
        match outcome {
            Ok(Outcome::Status(report)) => Some(
                report
                    .services
                    .into_iter()
                    .map(|service| (service.id, service.state))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::Stuck(_)
                | Outcome::Doctor(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_non_status_outcome_has_no_services_to_report() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(stated(dispatch(Command::Version, &ctx).await), None);
    }

    #[tokio::test]
    async fn asking_what_is_running_names_every_service_a_form_declares() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let seen = stated(dispatch(command, &ctx).await).unwrap_or_default();
        assert_eq!(seen.len(), LIBRARY.len());
        assert!(
            seen.iter()
                .any(|(id, state)| id == "jellyfin" && *state == ServiceState::Healthy),
            "{seen:?}"
        );
        assert!(
            seen.iter()
                .any(|(id, state)| id == "seerr" && *state == ServiceState::Absent),
            "a service that was never started is absent, not missing: {seen:?}"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_without_naming_a_form_covers_the_whole_stack() {
        let ctx = watching(Reporting::holding(&[], Lifecycle::Running, Health::None));
        let seen = stated(dispatch(Command::Ps { forms: Vec::new() }, &ctx).await);

        assert_eq!(
            seen.map(|services| services.len() > LIBRARY.len()),
            Some(true),
            "what is running is a question about the machine, not about a form"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let refusal = dispatch(Command::Ps { forms: Vec::new() }, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::ports::docker::ENGINE_UNREACHABLE),
            "an unreachable engine is not a stack with nothing in it"
        );
    }

    #[tokio::test]
    async fn asking_about_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let command = Command::Ps {
            forms: vec!["telly".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        assert_eq!(
            dispatch(Command::Ps { forms: Vec::new() }, &ctx)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them here too"
        );
    }

    #[tokio::test]
    async fn a_status_serialises_under_its_own_kind() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":["library"]"#),
                json.contains(r#""state":"healthy""#)
            )),
            Some(("status", true, true))
        );
    }

    /// The services a log stream actually carried lines for.
    async fn heard(ctx: &Ctx, forms: &[String], services: &[String]) -> Vec<String> {
        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);

        let query = LogQuery::recent(10);
        let mut lines = super::logs(ctx, forms, services, query)
            .await
            .unwrap_or(silent);

        let mut seen = Vec::new();
        while let Some(line) = lines.recv().await {
            seen.push(line.service);
        }
        seen.sort();
        seen.dedup();
        seen
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_narrows_to_what_that_form_declares() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &[]).await,
            vec!["jellyfin".to_owned()],
            "a form's log view must not carry another form's output"
        );
    }

    #[tokio::test]
    async fn naming_a_service_narrows_further_still() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("seerr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &["seerr".to_owned()]).await,
            vec!["seerr".to_owned()]
        );
    }

    #[tokio::test]
    async fn naming_no_form_reads_everything_that_is_saying_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &[], &[]).await,
            vec!["jellyfin".to_owned(), "sonarr".to_owned()]
        );
    }

    #[tokio::test]
    async fn reading_logs_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &[], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::docker::ENGINE_UNREACHABLE));
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &["telly".to_owned()], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::stack::closure::NO_SUCH_FORM));
    }

    #[tokio::test]
    async fn reading_logs_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let query = LogQuery::recent(10);
        assert_eq!(
            super::logs(&ctx, &[], &[], query)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn a_context_can_be_told_how_long_to_wait() {
        let ctx = watching(Reporting::default()).waiting(Duration::from_secs(7));
        assert_eq!(ctx.patience, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn the_engine_these_tests_use_answers_the_whole_port() {
        // Worth asserting rather than assuming. A fake that answers a method
        // more agreeably than a real engine would makes the path it shortcuts
        // untestable, which is how the log stream's own failure case went
        // missing until it was written down here.
        let engine = Reporting::absent();

        let ran = engine.exec("gluetun", &["true".to_owned()]).await;
        assert!(
            matches!(&ran, Err(EngineFailure::NoSuchContainer { name }) if name == "gluetun"),
            "{ran:?}"
        );

        let sampled = engine.stats("lemonfiber").await;
        assert_eq!(
            sampled.ok().map(|mut samples| samples.try_recv().is_err()),
            Some(true),
            "nothing is sampled, and the stream says so by ending"
        );
    }

    #[test]
    fn a_rehearsing_context_changes_nothing_else() {
        let rehearsal = ctx(Ok(spoke(""))).rehearsing();
        assert!(rehearsal.dry_run);
    }
}

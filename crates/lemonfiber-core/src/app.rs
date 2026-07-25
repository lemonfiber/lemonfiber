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

use std::sync::Arc;

use crate::config::Settings;
use crate::error::{Diagnose, Problem};
use crate::model::{Envelope, LifecycleReport, VersionReport};
use crate::platform::Environment;
use crate::ports::{Clock, Runner};
use crate::stack::closure::resolve;
use crate::stack::compose::{build, Action};
use crate::stack::Source;

/// What a surface is asking for.
///
/// Deliberately exhaustive. The surfaces ship in the same binary, so a new
/// command should stop the build until every surface has decided what to do
/// with it — silently rendering nothing is the failure this prevents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Report the binary's version, and the engine's where it can be reached.
    Version,
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
}

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => "version",
            Self::Lifecycle(_) => "lifecycle",
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
            Self::Lifecycle(report) => report.serialize(serializer),
        }
    }
}

/// Everything a command needs that is not part of the command itself.
pub struct Ctx {
    /// Whether to report what would happen and change nothing.
    pub dry_run: bool,
    /// How programs are run.
    pub runner: Arc<dyn Runner>,
    /// What time it is, for the one rule that depends on it.
    pub clock: Arc<dyn Clock>,
    /// Which stack is being operated.
    pub stack: Source,
    /// What the operator chose.
    pub settings: Settings,
    /// Which of the four environments this is.
    ///
    /// Supplied rather than decided here. Telling Docker Engine from Docker
    /// Desktop means asking the daemon, and a core that guessed would be wrong
    /// silently — so the surface answers, and today it answers with what it can
    /// see until the engine adapter can tell it the rest.
    pub environment: Environment,
}

impl Ctx {
    /// A context that runs programs for real, against a given stack.
    #[must_use]
    pub fn new(
        runner: Arc<dyn Runner>,
        clock: Arc<dyn Clock>,
        stack: Source,
        settings: Settings,
        environment: Environment,
    ) -> Self {
        Self {
            dry_run: false,
            runner,
            clock,
            stack,
            settings,
            environment,
        }
    }

    /// Today, as the manifest's date rules mean it.
    ///
    /// A clock before the epoch, or one far enough ahead to overflow a calendar,
    /// falls back to the epoch: refusing to do anything because the machine's
    /// clock is absurd would be a worse answer than checking dates against a
    /// date that is merely wrong.
    fn today(&self) -> lemonfiber_manifest::Date {
        let seconds = self
            .clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or_default();
        lemonfiber_manifest::Date::from_unix_seconds(seconds).unwrap_or(EPOCH)
    }

    /// The same context, in rehearsal.
    #[must_use]
    pub fn rehearsing(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

/// The first day the calendar rules can name, used when the clock cannot be
/// believed at all.
const EPOCH: lemonfiber_manifest::Date = lemonfiber_manifest::Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// Carry out a command.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Problem> {
    match command {
        Command::Version => version(ctx).await.map(Outcome::Version),
        Command::Up { forms } => lifecycle(ctx, &forms, &Action::Up).await,
        Command::Down { forms } => lifecycle(ctx, &forms, &Action::Down).await,
        Command::Restart { forms, services } => {
            lifecycle(ctx, &forms, &Action::Restart(services)).await
        }
        Command::Pull { forms } => lifecycle(ctx, &forms, &Action::Pull).await,
    }
}

/// Resolve forms, build the command, and run it unless this is a rehearsal.
///
/// Nothing here decides anything a surface could have decided differently, which
/// is the point: `up` from a keypress and `up` from a subcommand reach this same
/// function with the same arguments.
async fn lifecycle(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Outcome, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;
    let plan = resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| err.problem())?;
    let stack = ctx
        .stack
        .materialise(ctx.settings.stack_dir.as_deref())
        .map_err(|err| err.problem())?;

    let command = build(&plan, &ctx.settings, &stack, action, ctx.environment);

    let mut report = LifecycleReport {
        action: action.name().to_owned(),
        profiles: plan.profiles.into_iter().collect(),
        dropped: plan.dropped.into_iter().collect(),
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
    };

    // A rehearsal stops here deliberately: it has already done everything except
    // the one irreversible step, so what it reports is what would run rather
    // than an approximation of it.
    if ctx.dry_run {
        return Ok(Outcome::Lifecycle(report));
    }

    let output = ctx
        .runner
        .run(&command)
        .await
        .map_err(|err| err.problem())?;
    report.status = output.status;
    Ok(Outcome::Lifecycle(report))
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
async fn version(ctx: &Ctx) -> Result<VersionReport, Problem> {
    let argv = ["docker", "compose", "version", "--short"].map(str::to_owned);
    let compose = match ctx.runner.run(&argv).await {
        Ok(output) if output.succeeded() => Some(output.stdout.trim().to_owned()),
        Ok(_) | Err(_) => None,
    };

    // The stack is the one thing here that can genuinely be wrong: an
    // unreadable directory is the operator's own `--stack-dir`, and they need
    // to hear about it rather than see a version report with a hole in it.
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    Ok(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{dispatch, Command, Ctx, Environment, Outcome, Settings, Source, VersionReport};
    use crate::ports::process::{Failure, Output, Runner};

    /// A runner that answers with whatever the test scripted.
    struct Scripted(Result<Output, Failure>);

    #[async_trait]
    impl Runner for Scripted {
        async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
            match &self.0 {
                Ok(output) => Ok(output.clone()),
                Err(Failure::NotFound { program }) => Err(Failure::NotFound {
                    program: program.clone(),
                }),
                Err(Failure::Unusable { program, reason }) => Err(Failure::Unusable {
                    program: program.clone(),
                    reason: reason.clone(),
                }),
            }
        }
    }

    /// The stack this repository carries, read from disk.
    fn stack() -> Source {
        Source::External(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/media-stack"
        )))
    }

    fn ctx(scripted: Result<Output, Failure>) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(scripted)),
            Arc::new(crate::adapters::System),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
    }

    fn spoke(stdout: &str) -> Output {
        Output {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        }
    }

    fn refused(stderr: &str) -> Output {
        Output {
            status: Some(1),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
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
                r#"{"api_version":1,"kind":"version","data":{"binary":"0.0.0","#,
                r#""supported_schema":[1],"stack":"0.1.0","compose":"v2.32.1"}}"#
            ))
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_reported_rather_than_left_out() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("v2.32.1")))),
            Arc::new(crate::adapters::System),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
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
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(crate::adapters::System),
            stack(),
            settings,
            Environment::MacOs,
        )
        .rehearsing()
    }

    fn report(outcome: Result<Outcome, super::Problem>) -> Option<crate::model::LifecycleReport> {
        match outcome {
            Ok(Outcome::Lifecycle(report)) => Some(report),
            Ok(Outcome::Version(_)) | Err(_) => None,
        }
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
                report.profiles,
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
            produced.map(|report| report.dropped),
            Some(vec!["torrent".to_owned()]),
            "the operator hears which service is missing, and why"
        );
    }

    #[tokio::test]
    async fn a_real_run_reports_how_the_command_exited() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(crate::adapters::System),
            stack(),
            settings,
            Environment::MacOs,
        );
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
        let ctx = Ctx::new(
            Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))),
            Arc::new(crate::adapters::System),
            stack(),
            settings,
            Environment::MacOs,
        );
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
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(crate::adapters::System),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
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
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(crate::adapters::System),
            Source::Embedded(&EMBEDDED),
            settings,
            Environment::MacOs,
        );
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
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(crate::adapters::System),
            invalid,
            Settings::default(),
            Environment::MacOs,
        );
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

    #[test]
    fn a_rehearsing_context_changes_nothing_else() {
        let rehearsal = ctx(Ok(spoke(""))).rehearsing();
        assert!(rehearsal.dry_run);
    }
}

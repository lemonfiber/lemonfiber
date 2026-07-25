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
use crate::model::{Envelope, VersionReport};
use crate::ports::Runner;
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
}

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => "version",
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
        }
    }
}

/// Everything a command needs that is not part of the command itself.
pub struct Ctx {
    /// Whether to report what would happen and change nothing.
    pub dry_run: bool,
    /// How programs are run.
    pub runner: Arc<dyn Runner>,
    /// Which stack is being operated.
    pub stack: Source,
    /// What the operator chose.
    pub settings: Settings,
}

impl Ctx {
    /// A context that runs programs for real, against a given stack.
    #[must_use]
    pub fn new(runner: Arc<dyn Runner>, stack: Source, settings: Settings) -> Self {
        Self {
            dry_run: false,
            runner,
            stack,
            settings,
        }
    }

    /// The same context, in rehearsal.
    #[must_use]
    pub fn rehearsing(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

/// Carry out a command.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Problem> {
    match command {
        Command::Version => version(ctx).await.map(Outcome::Version),
    }
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
    let stack = ctx.stack.manifest().map_err(|err| err.problem())?;

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

    use super::{dispatch, Command, Ctx, Outcome, Settings, Source, VersionReport};
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
        Ctx::new(Arc::new(Scripted(scripted)), stack(), Settings::default())
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
            .and_then(|outcome| serde_json::to_string(&outcome.envelope()).ok());
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
            nowhere,
            Settings::default(),
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

    #[test]
    fn a_rehearsing_context_changes_nothing_else() {
        let rehearsal = ctx(Ok(spoke(""))).rehearsing();
        assert!(rehearsal.dry_run);
    }
}

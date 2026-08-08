//! What setup checks before it asks, and what it starts after it writes.
//!
//! The two ends of the walk that reach the machine rather than the operator: the
//! environment has to work before a single question is worth asking, and the
//! stack has to come up once the answers are applied.

use std::process::ExitCode;

use lemonfiber_core::app::{diagnose, dispatch, Command, Ctx, Outcome};
use lemonfiber_core::docker::Condition;
use lemonfiber_core::doctor::{Category, Overall};

use crate::engine::pull_showing;
use crate::exit::{complain, settled, PREFLIGHT};
use crate::render::render;

use super::first_content;
use super::Surface;

/// The form setup brings up once the answers are applied.
///
/// The television form is the one the product is measured on — a fresh machine to
/// a working stack — and it is what a first run wants: an operator after only
/// movies or music switches with `up` once they are running.
const STARTER_FORM: &str = "tv";

/// Check the environment before setup asks anything.
///
/// It runs the very check `lemonfiber doctor` runs for the environment — not a
/// second copy of it — so a missing container engine and one whose daemon is down
/// are told apart and remedied here in the same words as everywhere else. A broken
/// or undetermined result stops setup before a single question is asked; a healthy
/// one passes without a word.
pub(super) async fn preflight(ctx: &Ctx) -> Result<(), ExitCode> {
    // Asked for as a report rather than through the command enum: a dispatched
    // diagnosis comes back as an outcome that has to be destructured, with an arm
    // for every answer it could never be.
    let report = diagnose(ctx, Some(Category::Environment), false)
        .await
        .map_err(|problem| complain(&problem))?;

    if matches!(report.overall, Overall::Broken | Overall::Unknown) {
        render(&Outcome::Doctor(report), false);
        eprintln!("\nSetup needs these put right before it can go on.");
        return Err(ExitCode::from(PREFLIGHT));
    }
    Ok(())
}

/// Bring the stack up and report how it settled, the last step of a fresh setup.
///
/// The images are pulled first, with their progress on screen, so the several
/// gigabytes come down where the operator can watch rather than as a silent wait
/// inside `up`. Only once they are down is the stack brought up and waited on for
/// health; a pull that failed stops here rather than starting against images that
/// never arrived.
pub(super) async fn start(ctx: &Ctx, surface: &dyn Surface) -> ExitCode {
    let forms = vec![STARTER_FORM.to_owned()];
    if let Err(code) = pull_showing(ctx, &forms, false).await {
        return code;
    }

    match dispatch(Command::Up { forms }, ctx).await {
        Ok(outcome) => {
            render(&outcome, false);
            // The offer is the last thing setup does, and it needs to know what the stack
            // actually settled to — which is right here, and nowhere else afterwards.
            first_content::offer(ctx, surface, condition(&outcome), settled(&outcome)).await
        }
        Err(problem) => complain(&problem),
    }
}

/// What the stack settled to, where the outcome is one a lifecycle produced.
fn condition(outcome: &Outcome) -> Option<Condition> {
    match outcome {
        Outcome::Lifecycle(report) => report.condition,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{condition, preflight, start};
    use crate::exit::{shown, success};
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::config::Protocols;
    use lemonfiber_core::stack::Source;

    use crate::setup::tests::{ctx, working_ctx, FakeEngine, Scripted};

    #[tokio::test]
    async fn an_environment_that_cannot_work_stops_setup_before_a_question() {
        // Nothing setup does works without a container engine, so it is checked
        // before the first question rather than after eleven answers.
        assert!(preflight(&ctx()).await.is_err());
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_stops_setup_with_its_own_words() {
        // The checks need the stack before any of them can run, so a stack that
        // will not read is reported as itself rather than as a failed environment.
        let mut ctx = working_ctx();
        ctx.stack = Source::External(std::path::Path::new("/lemonfiber-not-a-real-stack"));
        assert!(preflight(&ctx).await.is_err());
    }

    #[tokio::test]
    async fn an_environment_that_works_passes_without_a_word() {
        assert!(preflight(&working_ctx()).await.is_ok());
    }

    #[tokio::test]
    async fn a_pull_that_failed_stops_before_starting_against_images_that_never_came() {
        // Starting against images that never arrived is worse than not starting.
        let code = start(&ctx(), &Scripted::saying(false, &[])).await;
        assert_ne!(shown(code), success());
    }

    /// A stack with no services of its own — enough for a real `up` to run and
    /// settle, without the fifteen containers the shipped one would wait on.
    static QUIET: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/quiet-stack");

    #[test]
    fn only_a_lifecycle_says_what_the_stack_settled_to() {
        // Asked of whatever `up` handed back: anything that is not a lifecycle report has
        // no condition to read, and offering a walk over one would be a guess.
        let report = lemonfiber_core::model::VersionReport {
            binary: "0".to_owned(),
            supported_schema: Vec::new(),
            stack: String::new(),
            compose: None,
        };
        assert_eq!(condition(&Outcome::Version(report)), None);
    }

    #[tokio::test]
    async fn a_stack_that_came_up_reports_how_it_settled() {
        // The far end of a first run: images down, stack up, and how it settled put
        // on screen. It needs somewhere to write the stack Docker reads and an
        // engine that answers, which is what an applied setup leaves behind.
        let stack_dir =
            std::env::temp_dir().join(format!("lemonfiber-boot-{}-started", std::process::id()));
        let _ = std::fs::remove_dir_all(&stack_dir);
        let mut ctx = working_ctx();
        ctx.engine = std::sync::Arc::new(FakeEngine::quiet());
        ctx.stack = Source::Embedded(&QUIET);
        ctx.settings.protocols = Protocols::both();
        ctx.settings.stack_dir = Some(stack_dir.clone());

        let code = start(&ctx, &Scripted::saying(false, &[])).await;

        assert_eq!(
            shown(code),
            success(),
            "a stack that came up is not reported as a failure"
        );
        let _ = std::fs::remove_dir_all(&stack_dir);
    }
}

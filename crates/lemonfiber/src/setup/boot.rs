//! What setup checks before it asks, and what it starts after it writes.
//!
//! The two ends of the walk that reach the machine rather than the operator: the
//! environment has to work before a single question is worth asking, and the
//! stack has to come up once the answers are applied.

use std::process::ExitCode;

use lemonfiber_core::app::{diagnose, dispatch, seeding, Command, Ctx, Outcome};
use lemonfiber_core::docker::Condition;
use lemonfiber_core::doctor::{autostart, overall, Category, Finding, Narrowing, Overall};
use lemonfiber_core::model::DoctorReport;

use crate::engine::pull_showing;
use crate::exit::{complain, settled, PREFLIGHT};
use crate::render::render;

use super::first_content;
use super::{door, Surface};
use crate::say::{complain, say};

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
    let report = diagnose(ctx, &Narrowing::Category(Category::Environment), false)
        .await
        .map_err(|problem| complain(&problem))?;

    // Whether this machine would bring the stack back after a restart is in this
    // family and is not this question. It answers `enabled-unverified` wherever a
    // Docker Desktop setting cannot be read — which is a fact about a machine that is
    // working perfectly well today — and an undetermined finding stops setup below.
    // Left in, somebody who had answered yes to autostart could not run setup again.
    // Re-summed rather than judged on the verdict that came back, because a word about
    // findings that are no longer here is not a word about these.
    let findings = gating(report.findings);
    let report = DoctorReport {
        overall: overall(&findings),
        findings,
    };

    if matches!(report.overall, Overall::Broken | Overall::Unknown) {
        render(&Outcome::Doctor(report), false);
        complain!("\nSetup needs these put right before it can go on.");
        return Err(ExitCode::from(PREFLIGHT));
    }
    Ok(())
}

/// The findings that decide whether setup can go on at all.
///
/// One is dropped, and it is the one whose honest answer would stop setup for a
/// reason setup is not about. Named here rather than filtered inline so the rule can
/// be read, and exercised, without standing up a machine whose Docker Desktop
/// settings cannot be read.
fn gating(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| finding.check != autostart::CHECK)
        .collect()
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
            for line in afterwards(ctx).await {
                say!("{line}");
            }
            // The offer is the last thing setup does, and it needs to know what the stack
            // actually settled to — which is right here, and nowhere else afterwards.
            first_content::offer(ctx, surface, condition(&outcome), settled(&outcome)).await
        }
        Err(problem) => complain(&problem),
    }
}

/// What setup says once the stack it started is up, in the order it says it.
///
/// Assembled rather than said as it goes, so what a first run leaves an operator
/// with is one value that can be read back rather than two calls nothing watches.
///
/// The forwarded-port cost is said here and nowhere else: setup is the moment this
/// stack was decided, and a stack that forwards no port is not broken — nothing
/// would come of repeating it on every run except an operator who skims. The
/// address comes after it, because a caveat about what this stack cannot do is
/// worth less than the answer they are about to be asked for by somebody in the
/// next room, and the last thing on the screen is the thing that gets read.
async fn afterwards(ctx: &Ctx) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(cost) = seeding::at_setup(ctx.settings.protocols, &ctx.settings.port_forward) {
        lines.push(format!("\n{cost}"));
    }
    lines.extend(door::handed(ctx).await);
    lines
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
    use super::{afterwards, condition, gating, overall, preflight, start};
    use crate::exit::{shown, success};
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::config::Protocols;
    use lemonfiber_core::doctor::{autostart, Category, Finding, Overall, Verdict};
    use lemonfiber_core::error::Remedy;
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

    /// Autostart it could not confirm does not stop somebody setting up.
    ///
    /// The check that answers it is in this very family and answers
    /// `enabled-unverified` wherever Docker Desktop's own setting cannot be read —
    /// which is most machines, and says nothing about whether this one can run the
    /// stack today. An undetermined finding stops setup, so left in this would have
    /// meant that anybody who had answered yes to starting on boot could not run
    /// setup a second time.
    #[test]
    fn a_prerequisite_nobody_could_confirm_does_not_stop_setup() {
        let unverified = Finding::in_category(
            Category::Environment,
            autostart::CHECK,
            "The stack comes back after a restart",
            Verdict::Unverified {
                reason: "enabled-unverified".to_owned(),
                remedy: Remedy::new("Open Docker Desktop"),
            },
        );
        let engine = Finding::in_category(
            Category::Environment,
            "environment.engine",
            "Docker engine",
            Verdict::Pass { note: None },
        );

        let kept = gating(vec![engine, unverified]);

        assert_eq!(kept.len(), 1, "the autostart finding is the one dropped");
        assert_eq!(overall(&kept), Overall::Healthy, "so setup goes on");
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
            changelog: lemonfiber_core::changelog::Notes::unread(),
        };
        assert_eq!(condition(&Outcome::Version(report)), None);
    }

    #[tokio::test]
    async fn a_first_run_ends_by_saying_what_to_send_the_people_who_live_here() {
        // The whole of what setup adds once the stack is up, read back as one value:
        // the cost this stack was decided at, and then the address, which is the
        // question setup is about to be asked and the one nothing above it answers.
        let mut ctx = working_ctx();
        ctx.engine = std::sync::Arc::new(FakeEngine::quiet());
        ctx.settings.protocols = Protocols::both();

        let said = afterwards(&ctx).await.join("\n");

        assert!(said.contains("No port is forwarded"), "{said}");
        assert!(said.contains("front door"), "{said}");
        let cost = said.find("No port is forwarded");
        let door = said.find("front door");
        assert!(
            cost.is_some_and(|cost| door.is_some_and(|door| door > cost)),
            "the address is the last thing on the screen: {said}"
        );
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

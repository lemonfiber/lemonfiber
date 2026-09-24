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
mod tests;

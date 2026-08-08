//! What setup checks before it asks, and what it starts after it writes.
//!
//! The two ends of the walk that reach the machine rather than the operator: the
//! environment has to work before a single question is worth asking, and the
//! stack has to come up once the answers are applied.

use std::process::ExitCode;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::doctor::{Category, Overall};

use crate::engine::pull_showing;
use crate::exit::{complain, settled, FAILURE, PREFLIGHT};
use crate::render::render;

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
    let report = match dispatch(
        Command::Doctor {
            only: Some(Category::Environment),
            disruptive: false,
        },
        ctx,
    )
    .await
    {
        Ok(Outcome::Doctor(report)) => report,
        // Asking for a diagnosis and being handed anything else cannot happen, but
        // is not worth a crash if it somehow does.
        Ok(_) => return Err(ExitCode::from(FAILURE)),
        Err(problem) => return Err(complain(&problem)),
    };

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
pub(super) async fn start(ctx: &Ctx) -> ExitCode {
    let forms = vec![STARTER_FORM.to_owned()];
    if let Err(code) = pull_showing(ctx, &forms, false).await {
        return code;
    }

    match dispatch(Command::Up { forms }, ctx).await {
        Ok(outcome) => {
            render(&outcome, false);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

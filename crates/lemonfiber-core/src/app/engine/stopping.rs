//! Refusing to stop what something else still needs.
//!
//! Forms overlap. `tv` and `movies` both reach the indexer, and an operator who
//! started both and then stops one of them is not asking for the other to lose its
//! searching — they are thinking about the form they named, and the services it
//! shares are not visible from there.
//!
//! So stopping asks first, and refuses by naming the form that would be hurt. The
//! refusal is the useful part: an operator told *which* form still needs the service
//! can decide to stop that one too, and one told only "cannot stop" cannot.

use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::docker::survey;
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::stack::standing::needed_by;

/// What stopping these forms would take from something else that is running.
///
/// `Ok(())` where nothing else is up that holds any of it, which is the ordinary
/// case: one form started, that form stopped.
///
/// # Errors
///
/// Returns the [`Problem`] naming the forms that still need what would be stopped,
/// or the one a surface should render when the engine cannot be reached — an engine
/// that will not say what is running cannot be overruled into stopping it.
pub(crate) async fn permitted(
    ctx: &Ctx,
    manifest: &Manifest,
    forms: &[String],
) -> Result<(), Box<Problem>> {
    // Surveyed across every profile the stack declares. What is running is a question
    // about the machine rather than about the form being stopped, and the forms that
    // would be deprived are by definition ones the operator did not name.
    let declared: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let running = survey(manifest, &declared, &containers, ctx.settings.protocols);

    let needed = needed_by(manifest, ctx.settings.protocols, &running, forms);
    if needed.is_empty() {
        return Ok(());
    }

    Err(Box::new(refusal(forms, &needed)))
}

/// What to tell an operator whose stop would have taken something else down with it.
fn refusal(forms: &[String], needed: &[String]) -> Problem {
    let stopping = forms.join(", ");
    let named = needed.join(", ");

    Problem::new(
        super::super::STILL_NEEDED,
        Severity::Error,
        format!(
            "{stopping} shares services with {named}, which {} running",
            is(needed)
        ),
        format!(
            "Stopping {stopping} would take services out from under {named}. Nothing was \
             stopped. Stop {named} as well if that is what you meant, or leave both up — \
             a service two forms reach belongs to whichever of them you are using."
        ),
        Remedy::new("Stop both, if neither is wanted")
            .with_detail(format!("lemonfiber down {stopping} {named}")),
    )
    .in_state(State::Guided)
}

/// Whether the named forms take a singular or a plural verb.
fn is(forms: &[String]) -> &'static str {
    if forms.len() == 1 {
        "is still"
    } else {
        "are still"
    }
}

#[cfg(test)]
mod tests;

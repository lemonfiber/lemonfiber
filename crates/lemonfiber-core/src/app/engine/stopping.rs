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
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let running = survey(manifest, &declared, &containers);

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
mod tests {
    use super::{is, refusal};

    #[test]
    fn a_refusal_names_the_form_that_still_needs_the_service() {
        let problem = refusal(&["tv".to_owned()], &["movies".to_owned()]);
        assert!(problem.summary.contains("tv") && problem.summary.contains("movies"));
        assert!(
            problem.meaning.contains("Nothing was stopped"),
            "the operator is told the stack is as they left it: {}",
            problem.meaning
        );
    }

    /// The remedy is the point of naming them: it is the command that does what the
    /// operator probably meant, ready to be run.
    #[test]
    fn a_refusal_offers_the_command_that_stops_both() {
        let problem = refusal(&["tv".to_owned()], &["movies".to_owned()]);
        assert_eq!(
            problem
                .remedies
                .first()
                .and_then(|remedy| remedy.detail.clone()),
            Some("lemonfiber down tv movies".to_owned())
        );
    }

    #[test]
    fn one_form_and_several_take_the_verb_they_are_owed() {
        assert_eq!(is(&["movies".to_owned()]), "is still");
        assert_eq!(is(&["movies".to_owned(), "music".to_owned()]), "are still");
    }
}

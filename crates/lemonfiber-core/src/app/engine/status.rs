//! What every service is doing, and what the stack amounts to.

use super::super::Ctx;
use crate::docker::{condition, condition_of_the_stack, survey, undeclared, State};
use crate::error::{Diagnose, Problem};
use crate::model::StatusReport;
use crate::stack::closure::resolve;
use crate::stack::standing::{brought, left_out};

/// What every service in the named forms is doing.
///
/// Naming no form reports the whole stack, because "what is running" is a
/// question about the machine rather than about a form — and an operator asking
/// it has usually forgotten which form they started.
pub(crate) async fn status(ctx: &Ctx, forms: &[String]) -> Result<StatusReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let protocols = ctx.settings.protocols;
    let whole: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let profiles: Vec<String> = if forms.is_empty() {
        whole.clone()
    } else {
        resolve(&manifest, forms, protocols)
            .map_err(|err| Box::new(err.problem()))?
            .profiles
            .into_iter()
            .collect()
    };

    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    // Surveyed whole and narrowed after, because which forms are up is a question
    // about every service they hold rather than about the ones asked after.
    let everything = survey(&manifest, &whole, &containers, protocols);
    let brought = brought(&manifest, protocols, &everything);
    let active_forms: Vec<String> = brought.iter().map(|(form, _)| form.clone()).collect();
    let filtered = left_out(&manifest, &brought);
    // A service an active form's closure left out, and that is not there, is reported
    // among what was filtered. Listing it again as absent would say it failed to start.
    let services: Vec<_> = everything
        .into_iter()
        .filter(|service| profiles.contains(&service.profile))
        .filter(|service| {
            service.state != State::Absent || !filtered.iter().any(|out| out.id == service.id)
        })
        .collect();
    let condition = if forms.is_empty() {
        condition_of_the_stack(&services, &active_forms)
    } else {
        condition(&services)
    };

    // Read from the manifest rather than from what is running, because a declaration
    // this build cannot reach is unreachable whether or not the container is up — and
    // an operator who stopped the service would otherwise be told the declaration had
    // healed.
    let project =
        super::super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let unsupported =
        super::super::targets::unsupported_here(&manifest.services, project.as_deref());

    Ok(StatusReport {
        forms: forms.to_vec(),
        active_forms,
        filtered,
        condition,
        undeclared: undeclared(&manifest, &containers),
        services,
        disturbs: crate::model::Disturbances::all(ctx.patience),
        unsupported,
    })
}

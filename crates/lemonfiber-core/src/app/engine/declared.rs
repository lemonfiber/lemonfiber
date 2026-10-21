//! What the stack declares and which versions are in play, read without acting on
//! anything: the forms, where each service comes from, what each is for, and the versions.

use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{CatalogueReport, FormReport, FormsReport, ProvenanceReport, VersionReport};

/// Every form the stack declares, in its own words.
///
/// A read of the manifest and nothing else: forms come from the stack rather than from
/// lemonfiber, so this reports what is declared rather than what lemonfiber expects to
/// find. An unreadable stack is the one thing that can genuinely be wrong here, and it is
/// the operator's own `--stack-dir` when it is.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read. Boxed
/// as a capture's refusals are: a refusal carries a good deal more than the listing it is
/// refusing to give.
pub(crate) fn forms(ctx: &Ctx) -> Result<FormsReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(FormsReport {
        forms: manifest
            .forms
            .iter()
            .map(|form| FormReport {
                id: form.id.clone(),
                name: form.name.clone(),
                description: form.description.clone(),
                composable: form.composable,
            })
            .collect(),
    })
}

/// Where every service this stack declares comes from, in the stack's own words.
///
/// A read of the manifest and nothing else, the way the forms listing above is — and
/// through the checked read rather than a bare one, which is the whole strength of the
/// answer. The check holds every service's licence against the OSI identifier list, so
/// a stack carrying one published under something else is refused here rather than
/// listed with the rest, and an operator asking what they are running is answered by a
/// report that stands on that check instead of repeating the claim it is about.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read, or
/// when what it declares does not hold together. Boxed as the listing beside it is.
pub(crate) fn provenance(ctx: &Ctx) -> Result<ProvenanceReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(ProvenanceReport::of(&manifest))
}

/// What each service this stack declares is for, and what became of the ones it
/// dropped.
///
/// A read of the manifest and nothing else, the way the forms listing above is — and
/// through the checked read rather than a bare one, which is what lets the answer be
/// taken at face value: a removal listed here is one that named a reason, and one that
/// named a replacement named something this stack knows about. A listing assembled
/// from an unchecked manifest could say a service was replaced by something that does
/// not exist, which is worse than saying nothing.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read, or
/// when what it declares does not hold together. Boxed as the listing beside it is.
pub(crate) fn catalogue(ctx: &Ctx) -> Result<CatalogueReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(CatalogueReport::of(&manifest))
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
pub(crate) async fn version(ctx: &Ctx) -> Result<VersionReport, Box<Problem>> {
    let argv = ["docker", "compose", "version", "--short"].map(str::to_owned);
    let compose = match ctx.seams.runner.run(&argv).await {
        Ok(output) if output.succeeded() => Some(output.stdout.trim().to_owned()),
        Ok(_) | Err(_) => None,
    };

    // The stack is the one thing here that can genuinely be wrong: an
    // unreadable directory is the operator's own `--stack-dir`, and they need
    // to hear about it rather than see a version report with a hole in it.
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let binary = env!("CARGO_PKG_VERSION").to_owned();
    Ok(VersionReport {
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
        attestation_status: crate::device_keys::attestation::status::taken(),
        // Read from what this build carries rather than from anything outside it,
        // for the same reason the engine's absence does not fail this read: asking
        // what is in play is what an operator does when something is wrong, and an
        // answer that needed a network would be missing exactly then.
        changelog: crate::changelog::notes(&binary),
        binary,
    })
}

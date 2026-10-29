//! The certificate a service declares it serves with: made by lemonfiber before the service
//! first starts, beneath its configuration directory where it says, and presented unchanged
//! after; and made afresh for a recording or a run proof.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::Certificate;

use super::{kept_as, made, Kept, Unkept};
use crate::app::Ctx;
use crate::error::codes::stack::CERTIFICATE_NOT_MADE;
use crate::error::{Problem, Remedy};
use crate::ports::filesystem::{Confined, Readable};

/// The certificate `laid` declares, kept beneath `directory`, made first where none has
/// been; its key and the certificate readable by their owner alone.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read or only half of it is there, or a new one
/// cannot be made.
pub(crate) fn kept_or_made(
    confined: &dyn Confined,
    directory: &Path,
    laid: &Certificate,
) -> Result<Kept, Unkept> {
    match kept_as(confined, directory, laid)? {
        Some(held) => Ok(held),
        None => made(confined, directory, laid, Readable::Private),
    }
}

/// A new certificate `laid` declares, written beneath `directory`, a throwaway copy, over
/// whatever is there.
///
/// # Errors
///
/// [`Unkept::Unmade`] where it could not be made or written.
pub(crate) fn throwaway(
    confined: &dyn Confined,
    directory: &Path,
    laid: &Certificate,
) -> Result<Kept, Unkept> {
    made(confined, directory, laid, Readable::Owner)
}

/// Where the certificate a service serves with is written and who it is made out to: the
/// guarded door's own for a service that provides `stream.guard`, what it declares
/// otherwise, and nothing where it does neither.
pub(crate) fn served_with(
    provides: &[String],
    declared: Option<&Certificate>,
) -> Option<Certificate> {
    if provides
        .iter()
        .any(|name| name == lemonfiber_manifest::container::STREAM_GUARD)
    {
        return Some(super::own());
    }
    declared.cloned()
}

/// Every certificate a service of the stack or of an installed plugin declares, made
/// before the stack starts where it has not been.
///
/// # Errors
///
/// The refusal of the start, naming the service, where one could not be made or what is
/// kept for it cannot be read.
pub(crate) fn made_before_start(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
) -> Result<(), Box<Problem>> {
    let Some(project) =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())
    else {
        return Ok(());
    };
    let installed = crate::app::plugins::read(ctx).ok();
    let placed = installed
        .iter()
        .flat_map(crate::plugin::Register::installed)
        .flat_map(|one| &one.services);
    let declared = declaring(&project, &manifest.services, placed);
    for (service, directory, laid) in declared {
        kept_or_made(ctx.seams.confined.as_ref(), &directory, &laid)
            .map_err(|unkept| Box::new(unmade(&service, &unkept)))?;
    }
    Ok(())
}

/// Each service declaring a certificate, the stack's and each installed plugin's, with its
/// configuration directory beneath `project` and what it declares.
fn declaring<'a>(
    project: &Path,
    services: &[lemonfiber_manifest::Service],
    placed: impl Iterator<Item = &'a crate::plugin::Placed>,
) -> Vec<(String, PathBuf, Certificate)> {
    let stack = services.iter().filter_map(|service| {
        service.certificate.as_ref().map(|laid| {
            (
                service.id.clone(),
                crate::app::targets::service_config_dir(project, &service.id),
                laid.clone(),
            )
        })
    });
    let plugins = placed.filter_map(|placed| {
        let laid = placed.fields.certificate.as_ref()?;
        let directory = crate::app::targets::plugin_config_dir(project, placed)?;
        Some((placed.service.clone(), directory, laid.clone()))
    });
    stack.chain(plugins).collect()
}

/// The refusal of a start whose service's certificate could not be made or read.
fn unmade(service: &str, why: &Unkept) -> Problem {
    Problem::new(
        CERTIFICATE_NOT_MADE,
        format!("The certificate {service} declares could not be made, so nothing was started"),
        "The service serves with the certificate lemonfiber makes for it, and starting it \
         without one would start it unable to serve.",
        Remedy::new(
            "Where only one of its two files is there, or either is not what it should be, \
             move both aside and start again",
        ),
    )
    .with_detail(why.to_string())
}

#[cfg(test)]
mod tests;

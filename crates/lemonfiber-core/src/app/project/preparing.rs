//! The project a lifecycle command runs: generated for what is installed here, over every
//! network of the project made first where it starts, and written as the command writes it.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use super::{
    current, hosting, materialised_with, mounting, moved, networks_ensured, pinned, retired,
    rooted, secrets_written, Networking, Pass,
};
use crate::error::{Diagnose, Problem};
use crate::model::StackEdit;
use crate::project::Project;

/// The project a lifecycle command runs, and what writing it came to.
pub(crate) struct Prepared {
    /// The project.
    pub(crate) project: Project,
    /// An operator's edit to it, kept and reported, where there is one.
    pub(crate) edits: Vec<StackEdit>,
    /// The profiles of the services the planned ones run inside.
    pub(crate) hosting: BTreeSet<String>,
    /// The networks a start created and created again before writing it.
    pub(crate) networking: Networking,
}

/// Generate the project for `manifest` and what is installed here, and write it as a
/// lifecycle command running `plan` does: previewed by a rehearsal, and otherwise written
/// with each service's secrets file and the stack's own Compose files retired and, where
/// `starting`, over every network of the project made first, with the profiles of the
/// start beside it, each service's state moved into its mount tree and each mount source
/// not there yet made. Where `DATA_ROOT` is
/// refused, a start is refused and anything else leaves the project on disk as it stands.
///
/// # Errors
///
/// Where the register cannot be read, a network cannot be made, the project, a secrets
/// file or the journal cannot be written, or a start is asked over a refused `DATA_ROOT`.
pub(crate) async fn prepared(
    ctx: &Ctx,
    manifest: &Manifest,
    plan: &crate::stack::closure::Plan,
    starting: bool,
) -> Result<Prepared, Box<Problem>> {
    let register = super::super::plugins::read(ctx)?;
    let installed = register.installed();
    let networking = if starting {
        let topology = crate::project::topology(
            manifest,
            installed,
            &super::super::targets::chosen_fillers(ctx),
            pinned(ctx).is_some(),
        );
        networks_ensured(ctx, &topology).await?
    } else {
        Networking::default()
    };
    let project = current(ctx, manifest, installed);
    let pass = if ctx.dry_run {
        Pass::Preview
    } else {
        Pass::Write
    };
    let hosting = hosting(manifest, installed, &project, &plan.services);
    if let Err(refused) = rooted(ctx) {
        return if starting {
            Err(refused)
        } else {
            Ok(Prepared {
                project,
                edits: Vec::new(),
                hosting,
                networking,
            })
        };
    }
    let profiles: Option<BTreeSet<String>> =
        starting.then(|| plan.profiles.iter().chain(&hosting).cloned().collect());
    let edits = materialised_with(ctx, &project.text, pass, profiles.as_ref())
        .map_err(|failure| Box::new(failure.problem()))?
        .into_iter()
        .collect();
    if !ctx.dry_run {
        retired(ctx, installed);
        secrets_written(ctx, manifest, installed).map_err(|failure| Box::new(failure.problem()))?;
        if starting {
            moved(ctx, manifest)?;
            mounting::made(ctx, manifest, installed)?;
        }
    }
    Ok(Prepared {
        project,
        edits,
        hosting,
        networking,
    })
}

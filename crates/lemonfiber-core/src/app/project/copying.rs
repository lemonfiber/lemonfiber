//! The state a stack service kept in a volume the engine holds, copied once into its
//! mount tree, where the project mounts it from.
//!
//! The stack's static Compose files gave a service's mount a volume of the project's,
//! named for the service and the mount, where the project now mounts the service's own
//! directory. Before a start, each such volume the engine still keeps is copied into a
//! mount that holds nothing yet, journalled first, and the volume is left as it was. A
//! copy that cannot be made, or an engine that cannot say whether it keeps the volume,
//! stops the start: a service started on an empty mount would write its own state there,
//! and the copy would never be made again.

use std::path::PathBuf;

use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use crate::error::codes::stack::VOLUME_NOT_COPIED;
use crate::error::{Diagnose, Problem, Remedy};
use crate::journal::{Change, Kind};
use crate::ports::filesystem::Lying;
use crate::project::entry::{mounts, MOUNT_TREE};
use crate::stack::Source;

/// One volume to copy, and where to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Copy {
    /// The volume, by the engine's name.
    volume: String,
    /// The image of the service that kept it, which reaches it.
    image: String,
    /// The mount it goes to.
    to: PathBuf,
}

/// The volumes a start copies before it runs anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Copies {
    /// Each one.
    pending: Vec<Copy>,
    /// The project directory they are copied beneath.
    within: PathBuf,
    /// What the engine answered where it could not say whether it keeps a volume.
    unasked: Option<Problem>,
}

/// Every volume the engine keeps for a mount of the stack this build carries whose own
/// directory holds nothing yet. None for a stack directory the operator named, which is
/// run as it stands.
pub(crate) async fn pending(ctx: &Ctx, manifest: &Manifest) -> Copies {
    let (Source::Embedded(_), Some(within)) = (ctx.stack, super::directory(ctx)) else {
        return Copies::default();
    };
    let mut copies = Copies {
        within,
        ..Copies::default()
    };
    for service in crate::project::declared(manifest) {
        for mount in mounts(&service.mount, service.takes_data) {
            let to = copies
                .within
                .join(MOUNT_TREE)
                .join(&service.id)
                .join(&mount.from);
            if !empty(ctx, &to, &copies.within) {
                continue;
            }
            let volume = format!("{}_{}_{}", ctx.settings.project, service.id, mount.from);
            match ctx.seams.engine_volumes.kept(&volume).await {
                Ok(true) => copies.pending.push(Copy {
                    volume,
                    image: crate::project::reference(service),
                    to,
                }),
                Ok(false) => {}
                Err(failure) => {
                    copies.unasked.get_or_insert_with(|| failure.problem());
                }
            }
        }
    }
    copies
}

/// Whether nothing stands at `to` but, at most, an empty directory, reached through no
/// link.
fn empty(ctx: &Ctx, to: &std::path::Path, within: &std::path::Path) -> bool {
    match ctx.seams.confined.lies(to, within) {
        Lying::Absent => true,
        Lying::Plain => std::fs::read_dir(to).is_ok_and(|mut held| held.next().is_none()),
        Lying::Escaped => false,
    }
}

impl Copies {
    /// Copy each volume, journalling the copy before it is made.
    ///
    /// # Errors
    ///
    /// Where the engine could not say whether it keeps a volume, the journal cannot be
    /// written, or a copy cannot be made.
    pub(crate) async fn made(&self, ctx: &Ctx) -> Result<(), Box<Problem>> {
        if let Some(unasked) = &self.unasked {
            return Err(Box::new(unasked.clone()));
        }
        let Some(paths) = super::super::targets::layout(ctx) else {
            return Ok(());
        };
        for copy in &self.pending {
            let to = copy.to.display().to_string();
            let change = Change {
                at: ctx.stamp(),
                operation: super::OPERATION.to_owned(),
                target: to.clone(),
                kind: Kind::Copied {
                    volume: copy.volume.clone(),
                    to,
                },
            };
            crate::app::recover::journalled(&paths.journal(), &[change])
                .map_err(|failure| Box::new(failure.problem()))?;
            ctx.seams
                .engine_volumes
                .copied(&copy.volume, &copy.image, &copy.to, &self.within)
                .await
                .map_err(|failure| Box::new(uncopied(copy, &failure.to_string())))?;
        }
        Ok(())
    }
}

/// The refusal of a start whose volume could not be copied.
fn uncopied(copy: &Copy, why: &str) -> Problem {
    Problem::new(
        VOLUME_NOT_COPIED,
        format!(
            "The volume {} could not be copied to {}, so nothing was started",
            copy.volume,
            copy.to.display()
        ),
        "The service keeps its state in its own directory from now on, and starting it \
         before the copy is made would start it on nothing.",
        Remedy::new("Check the container engine is running, then start again"),
    )
    .with_detail(why.to_owned())
}

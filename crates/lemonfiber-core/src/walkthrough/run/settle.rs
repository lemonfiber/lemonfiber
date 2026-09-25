//! What happens after it lands: the note about the file, the nudge to the library, and
//! where the operator is left.
//!
//! Three things, each of which is the difference between a walkthrough that ends well and
//! one that ends in a shrug. The file was either hardlinked or copied, and an operator who
//! is not told will discover the second one as a full disk in four months. The media
//! server scans on its own schedule, so content that has genuinely arrived is genuinely
//! invisible for as long as an hour unless it is asked to look. And the moment it plays is
//! the moment they have nothing to do, which is where a product either points somewhere or
//! loses them.

use super::choose::Chosen;
use super::walk::Walk;
use crate::app::targets::{data_root, jellyfin_reader, seerr_reader};
use crate::app::Ctx;
use crate::model::WalkthroughReport;
use crate::ports::service::Library;
use crate::storage::Linked;
use crate::walkthrough::{Line, Link, Reason, Shape, Step};

/// Finish the walk: note what the import did, tell the library to look, and check it can
/// be found there.
pub(super) async fn settle(
    walk: &mut Walk<'_>,
    services: &[lemonfiber_manifest::Service],
    chosen: &Chosen<'_>,
) -> WalkthroughReport {
    let link = linked(walk.ctx).await;
    walk.say(Line::saying(Step::Importing, note(link)));

    let Some(jellyfin) = jellyfin_reader(walk.ctx, services) else {
        // Imported, with nothing running to play it from. Complete through the import and
        // said plainly, because "it worked and you cannot watch it" is not a failure of
        // the pipeline — it is a form that does not include a media server.
        return walk.stopped(
            Shape::Pipeline,
            Some(chosen.named.clone()),
            Reason::NoMediaServer,
        );
    };

    walk.say(Line::at(Step::Scanning));
    // A scan that will not run is not fatal: the library will find it on its own schedule,
    // and the check below is what decides whether the operator is told it is there.
    let _ = jellyfin.rescan().await;

    let visible = jellyfin
        .has_item(chosen.kind(), &chosen.entry.title)
        .await
        .unwrap_or(false);
    if !visible {
        return walk.stopped(
            Shape::Pipeline,
            Some(chosen.named.clone()),
            Reason::NotVisible,
        );
    }

    walk.say(Line::saying(Step::Available, chosen.named.clone()));
    let household = seerr_reader(walk.ctx, services).is_some();
    walk.finished(Shape::Pipeline, &chosen.named, link, household)
}

/// Whether an import here links or copies, asked at the moment the operator can see the
/// file it happened to.
///
/// Not read from the file: neither service reports how it filed something, and the path it
/// filed it at is not on the port. What is knowable — and what actually decides it — is
/// whether the library location can hardlink at all, which is the same empirical probe
/// setup runs when it accepts a data location. A location that links, links; one that
/// cannot, copies, every time, for every import.
///
/// A location that could not be probed leaves the question unanswered rather than guessed.
/// An operator told "this was copied" when it was not would go and fix a volume that is
/// already correct, which is worse than not being told.
async fn linked(ctx: &Ctx) -> Option<Link> {
    let root = data_root(ctx)?;
    link_of(&crate::storage::test_link(ctx.seams.filesystem.as_ref(), &root).await)
}

/// What to say about the file that just landed, where anything is known about it.
fn note(link: Option<Link>) -> String {
    match link {
        Some(link) => link.consequence().to_owned(),
        None => String::new(),
    }
}

/// What a probe's result means for an import — pure and total, because a probe that could
/// not be run is a different answer from one that ran and said no.
const fn link_of(probed: &Linked) -> Option<Link> {
    match probed {
        Linked::Yes { .. } => Some(Link::Hardlinked),
        Linked::No => Some(Link::Copied),
        Linked::Unwritable { .. } | Linked::Unconfirmed => None,
    }
}

#[cfg(test)]
mod tests;

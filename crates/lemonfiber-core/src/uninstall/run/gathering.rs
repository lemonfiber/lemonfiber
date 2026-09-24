//! Reading this machine, without letting any part of it stop the rest.
//!
//! A wedged stack is a common reason to uninstall, so nothing here is allowed to
//! refuse. Every source is asked, every refusal is written down in the words of
//! whatever refused, and what could not be read becomes a gap the manifest states
//! rather than an error the operator is handed instead of a way out.
//!
//! Gathered whole before anything is judged, for the reason the disk reckoning is:
//! a container list taken before a service stopped and an image list taken after
//! would describe two machines.

use std::path::PathBuf;

use crate::app::engine::in_flight;
use crate::app::targets::{data_root, layout};
use crate::app::Ctx;
use crate::config::paths::Paths;
use crate::ports::docker::Image;
use crate::ports::occupancy::Occupant;
use crate::uninstall::{Coming, Confidence, Tier};

/// Everything one survey read, and what it could not.
pub(super) struct Gathered {
    /// The services the stack declares, empty where it could not be read.
    pub(super) services: Vec<lemonfiber_manifest::Service>,
    /// The containers the engine is holding for this project, by the name Compose
    /// gives them.
    pub(super) containers: Vec<String>,
    /// Every image on this machine, with the projects standing on each.
    pub(super) images: Vec<Image>,
    /// Whether the engine said what it is holding, as against having said nothing.
    ///
    /// The difference decides what an absent container means: an engine that
    /// answered and named none is a stack with none, and one that would not answer
    /// is a stack whose containers are still there and are somebody's to remove by
    /// hand.
    pub(super) engine_answered: bool,
    /// Whether the engine said what it has pulled, for the reason above.
    pub(super) images_answered: bool,
    /// Where lemonfiber keeps its own files, where this run could say.
    pub(super) paths: Option<Paths>,
    /// The operator's data location, where this run could say.
    pub(super) root: Option<PathBuf>,
    /// Every file beneath the data location.
    pub(super) walked: Vec<Occupant>,
    /// What lemonfiber's own two directories hold, where this tier asks.
    pub(super) kept: Vec<Occupant>,
    /// What the data location's own volume is, where it is worth saying.
    pub(super) volume: Option<String>,
    /// What is still coming down.
    pub(super) coming: Vec<Coming>,
    /// The places this platform keeps things an uninstall leaves behind, and whether
    /// each was found — one entry per [`crate::uninstall::BESIDE`], in that order.
    pub(super) found: Vec<bool>,
    /// What could not be read, and why.
    pub(super) confidence: Confidence,
}

/// The Compose project name this stack runs under, and the network it creates.
///
/// Compose names its default network `<project>_default` and its containers
/// `<project>-<service>-<n>`. Spelled here because a manifest that named a container
/// differently from the engine would be a list an operator could not check.
pub(super) fn network_of(project: &str) -> String {
    format!("{project}_default")
}

/// Read everything a manifest for this tier is built from.
pub(super) async fn gather(ctx: &Ctx, tier: Tier) -> Gathered {
    let mut confidence = Confidence::whole();

    let services = match ctx.stack.checked_manifest(ctx.today()) {
        Ok(manifest) => manifest.services,
        Err(failure) => {
            confidence = confidence.short(format!(
                "the stack description could not be read, so the services are listed \
                 from what the engine is holding rather than from what this build \
                 declares: {failure}"
            ));
            Vec::new()
        }
    };

    let engine = engine(ctx, tier, &mut confidence).await;
    let paths = whereabouts(ctx, &mut confidence);
    let root = data_root(ctx).or_else(|| ctx.settings.data_root.clone());
    let (walked, volume) = disk(ctx, root.as_deref(), &mut confidence).await;
    let kept = ours(ctx, tier, paths.as_ref(), &mut confidence).await;

    Gathered {
        services,
        containers: engine.containers,
        images: engine.images,
        engine_answered: engine.containers_read,
        images_answered: engine.images_read,
        paths,
        root,
        walked,
        kept,
        volume,
        // Read for every tier rather than only the ones that stop things. Every
        // removal begins by stopping the services, because a service still writing
        // its database while its directory goes is the corruption a backup exists to
        // undo — so what is in flight is something the operator is owed whichever
        // removal they chose.
        coming: in_flight(ctx, &[])
            .await
            .into_iter()
            .map(|download| Coming {
                name: download.name,
                progress: download.progress,
            })
            .collect(),
        found: beside(ctx).await,
        confidence,
    }
}

/// What the engine said, and whether it said it.
struct Engine {
    containers: Vec<String>,
    containers_read: bool,
    images: Vec<Image>,
    images_read: bool,
}

/// What the engine is holding and what it has pulled, or the reasons it would not
/// say.
///
/// Asked only where the tier reaches it. A run removing configuration has no
/// question for a daemon, and one that asked anyway would report a machine as
/// half-unreadable over something it was never going to touch.
async fn engine(ctx: &Ctx, tier: Tier, confidence: &mut Confidence) -> Engine {
    if !tier.touches_containers() {
        return Engine {
            containers: Vec::new(),
            containers_read: true,
            images: Vec::new(),
            images_read: true,
        };
    }

    let (containers, containers_read) = match ctx.engine.list(&ctx.settings.project).await {
        Ok(held) => (
            held.into_iter()
                .map(|container| format!("{}-{}", container.project, container.service))
                .collect(),
            true,
        ),
        Err(failure) => {
            *confidence = confidence.clone().short(format!(
                "the container engine could not be reached, so its containers are \
                 listed from what this stack declares and are left for you to remove \
                 by hand: {failure}"
            ));
            (Vec::new(), false)
        }
    };

    // Asked only where the tier takes them. Stopping leaves every image where it is,
    // so a stop that listed the whole machine's images would be reading the engine
    // for a command that removes none of them.
    if !tier.touches_images() {
        return Engine {
            containers,
            containers_read,
            images: Vec::new(),
            images_read: true,
        };
    }

    let (images, images_read) = match ctx.images.images().await {
        Ok(pulled) => (pulled, true),
        Err(failure) => {
            *confidence = confidence.clone().short(format!(
                "the container engine would not say what it has pulled, so no image \
                 is removed and each is left for you to remove by hand: {failure}"
            ));
            (Vec::new(), false)
        }
    };

    Engine {
        containers,
        containers_read,
        images,
        images_read,
    }
}

/// What lemonfiber's own two directories hold, for the tier that removes them.
///
/// Walked rather than estimated, because the figure an operator reads before
/// agreeing has to be the one on their disk. A directory that is not there walks to
/// nothing, which is the ordinary reading on a machine that never got past setup.
async fn ours(
    ctx: &Ctx,
    tier: Tier,
    paths: Option<&Paths>,
    confidence: &mut Confidence,
) -> Vec<Occupant> {
    let Some(paths) = paths.filter(|_| tier.touches_configuration()) else {
        return Vec::new();
    };

    let mut held = Vec::new();
    for root in [paths.config_dir(), paths.data_dir()] {
        match ctx.occupancy.beneath(root).await {
            Ok(found) => held.extend(found),
            Err(fault) => {
                *confidence = confidence.clone().short(format!(
                    "{} is there and would not be read, so what it holds is named \
                     without a size: {}",
                    root.display(),
                    fault.message
                ));
            }
        }
    }
    held
}

/// Where lemonfiber keeps its own files.
///
/// The surface's answer first, and the settings' own second — a run whose
/// configuration home could not be resolved may still hold an environment file and a
/// stack directory, and those two name the same layout. Where neither answers, the
/// gap is stated: nothing is removed from a path this run had to guess at, because a
/// guess at the usual place is a guess at somebody else's directory.
fn whereabouts(ctx: &Ctx, confidence: &mut Confidence) -> Option<Paths> {
    let found = ctx
        .archives
        .as_ref()
        .map(|archiving| archiving.paths.clone())
        .or_else(|| layout(ctx));
    if found.is_none() {
        *confidence = confidence.clone().short(
            "this run could not say where lemonfiber keeps its own files, so they are \
             named by the locations this platform conventionally uses and nothing \
             under them is removed",
        );
    }
    found
}

/// What is beneath the data location, and what sort of volume it is on.
///
/// A location nobody configured is not a gap in the reading — it is a machine that
/// never chose one, and there is nothing beneath it to report.
async fn disk(
    ctx: &Ctx,
    root: Option<&std::path::Path>,
    confidence: &mut Confidence,
) -> (Vec<Occupant>, Option<String>) {
    let Some(root) = root else {
        return (Vec::new(), None);
    };

    let walked = match ctx.occupancy.beneath(root).await {
        Ok(found) => found,
        Err(fault) => {
            *confidence = confidence.clone().short(format!(
                "the data location is there and would not be read, so what is beneath \
                 it could not be sized or checked for anything that is not the \
                 stack's: {}",
                fault.message
            ));
            Vec::new()
        }
    };

    (walked, mounted(ctx, root).await)
}

/// What to say about the volume the data location sits on, where it is worth saying
/// anything.
///
/// A network share and a drive that unplugs are the two an operator forgets is what
/// they pointed the stack at, and removing across either reaches somewhere they were
/// not thinking about. An ordinary local disk needs no sentence.
async fn mounted(ctx: &Ctx, root: &std::path::Path) -> Option<String> {
    let facts = ctx.filesystem.describe(root).await;
    if facts.kind.is_network() {
        return Some(format!(
            "the data location is on a network share ({}), so removing reaches \
             another machine",
            facts.kind.label()
        ));
    }
    if facts.removable {
        return Some(
            "the data location is on a drive that unplugs, so removing reaches a disk \
             you may take elsewhere"
                .to_owned(),
        );
    }
    None
}

/// Whether each thing an uninstall leaves behind is on this machine.
///
/// Looked for rather than assumed, so what an operator is told is a list of what is
/// actually here. A platform with no single place worth naming, or a path that will
/// not resolve, reads as not found — which is why the entry is listed either way.
async fn beside(ctx: &Ctx) -> Vec<bool> {
    let mut found = Vec::new();
    for entry in crate::uninstall::BESIDE {
        let looked = crate::uninstall::looked_for(entry, ctx.environment);
        let there = match looked {
            Some(at) => ctx
                .filesystem
                .canonicalize(std::path::Path::new(at))
                .await
                .is_ok(),
            None => false,
        };
        found.push(there);
    }
    found
}

//! Gathering what the reckoning is made of, and taking what it offered.
//!
//! Everything read here is read before anything is judged, so that one report
//! describes one moment: a figure taken before a download landed and another taken
//! after would disagree about a disk nobody could see change.
//!
//! The two halves of this command are deliberately unequal. Reading is exhaustive
//! — both volumes, the whole tree, every client and every service queue — and
//! removing is the narrowest thing it can be: only the paths the reading already
//! named as costing nothing, and only when an answer arrives. There is no level of
//! fullness at which the second half runs by itself.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::{Amiss, Diagnose, Problem, Remedy, Severity, State};
use crate::ports::service::{Queued, Queues, Seeded, Seeding};
use crate::space::{
    reckon, Left, Level, Measured, Reckoning, Reclaimed, Role, Stalled, Volume, HALTED,
    NOWHERE_TO_MEASURE, WALK_REFUSED,
};

use crate::app::targets::{
    committed_bytes, download_targets, project_directory, servarr_targets, torrent_client,
};
use crate::app::Ctx;

/// The directory beneath the project root that the services' own files live in.
///
/// The stack's own convention, spelled here because this is the one command that
/// asks about the whole of it rather than about one service's file inside it.
const SERVICE_FILES: &str = "config";

/// Where the disk stands, and — where an answer was given — what taking the offer
/// came to.
///
/// # Errors
///
/// Returns a [`Problem`] where there is no data location to measure, where the
/// stack could not be read, or where the data location is there and will not be
/// walked.
pub(crate) async fn space(ctx: &Ctx, confirm: bool) -> Result<Reckoning, Box<Problem>> {
    let gathered = measure(ctx).await?;
    let mut reckoned = reckon(&gathered.measured);
    if confirm {
        reckoned.reclaimed = Some(reclaim(ctx, &reckoned, &gathered.measured).await);
    }
    Ok(reckoned)
}

/// Whether new acquisitions may be started, given where the disk stands.
///
/// The one place a level turns into a refusal, so that every command which brings
/// more content onto the disk refuses in the same words and for the same reading.
///
/// # Errors
///
/// Returns a [`Problem`] where the volume is full.
pub(crate) async fn admits(ctx: &Ctx) -> Result<(), Box<Problem>> {
    // Measured rather than remembered: a disk somebody emptied by hand between two
    // commands is not full any more, and a halt that outlived the condition would
    // be a stack nobody could restart without finding this rule.
    //
    // The volumes and nothing else. What a halt turns on is what is free *now*, and
    // neither a walk of the library nor a client's queue can change that number — so
    // the guard in front of every acquisition costs two readings of the platform,
    // rather than the whole reckoning it would otherwise sit behind.
    //
    // A disk nobody could measure has not been established to be full. Refusing over
    // a reading that could not be taken would stop work on a guess, on exactly the
    // machines least likely to deserve it — one with no data location configured has
    // not filled anything yet.
    let Ok(watched) = watched(ctx, false).await else {
        return Ok(());
    };
    if !Level::worst(watched.volumes.iter().map(|volume| volume.level)).halts() {
        return Ok(());
    }
    Err(Box::new(
        Problem::new(
            HALTED,
            Severity::Critical,
            "There is no room left, so nothing new is being fetched",
            "A service that cannot write its database may not merely stop — it can \
             take the file with it, which turns a disk that is full into work that \
             is gone. Fetching more onto it is what this is protecting against.",
            Remedy::new("Free space, then run this again")
                .with_detail("lemonfiber space --confirm"),
        )
        .in_state(State::Guided),
    ))
}

/// The two volumes, where each of them is, and what the stack they belong to says.
///
/// The stack is carried rather than read again by whatever needs it next: reading
/// it twice in one command would be two parses of one file, and a second reading is
/// a second chance for the two halves of an answer to describe different stacks.
struct Watched {
    /// The data location.
    root: PathBuf,
    /// Where the services keep their own files, or nowhere on a stack that has not
    /// been materialised and so has no project directory to hold them.
    services: Option<PathBuf>,
    /// The project directory the services are reached beneath.
    project: Option<PathBuf>,
    /// The stack as it declares itself.
    stack: lemonfiber_manifest::Manifest,
    /// What the download clients still have to write, zero where nobody asked.
    landing: u64,
    /// Both volumes, in the order they are reported.
    volumes: Vec<Volume>,
}

/// Measure the volumes, and — where `projecting` — ask the clients what is still
/// committed to landing on them.
///
/// The guard in front of an acquisition does not ask, because what it decides turns
/// on what is free now; the reckoning does, because a projection is the whole
/// difference between a warning and a description.
async fn watched(ctx: &Ctx, projecting: bool) -> Result<Watched, Box<Problem>> {
    let Some(root) = ctx.settings.data_root.clone() else {
        return Err(Box::new(nowhere()));
    };
    let stack = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let services = project.as_ref().map(|at| at.join(SERVICE_FILES));

    let landing = if projecting {
        committed_bytes(ctx, &stack.services, project.as_deref()).await
    } else {
        0
    };
    let taken = now(ctx);
    let mut volumes = vec![Volume::measured(
        Role::Data,
        &root,
        &ctx.filesystem.describe(&root).await,
        landing,
        taken,
    )];
    if let Some(at) = services.as_deref() {
        volumes.push(Volume::measured(
            Role::Services,
            at,
            &ctx.filesystem.describe(at).await,
            0,
            taken,
        ));
    }
    Ok(Watched {
        root,
        services,
        project,
        stack,
        landing,
        volumes,
    })
}

/// Everything one reckoning is made of, and the client the completed downloads in
/// it were read from.
///
/// The client is carried rather than resolved again by whatever acts next. Resolving
/// it a second time is a second parse of the stack and a second chance for the two
/// halves of one errand to be addressed to different clients — and the errand that
/// removes one of those downloads has to be addressed to the client that reported it.
pub(crate) struct Gathered {
    /// What was measured.
    pub(crate) measured: Measured,
    /// The torrent client the completed downloads came from, where the stack has one
    /// this run can authenticate to.
    pub(crate) holder: Option<crate::qbittorrent::Qbittorrent>,
}

/// Read everything one reckoning is made of.
pub(crate) async fn measure(ctx: &Ctx) -> Result<Gathered, Box<Problem>> {
    let watched = watched(ctx, true).await?;
    let project = watched.project.as_deref();

    let data = ctx
        .occupancy
        .beneath(&watched.root)
        .await
        .map_err(|fault| Box::new(unreadable(&watched.root, &fault.message)))?;
    // The services' own files are read best-effort. Where they cannot be walked the
    // line for them is absent rather than the whole reckoning being refused: what an
    // operator came here for is where the media went.
    let services = match watched.services.as_deref() {
        Some(at) => ctx.occupancy.beneath(at).await.unwrap_or_default(),
        None => Vec::new(),
    };

    let holder = torrent_client(ctx, &download_targets(&watched.stack.services, project));
    let held = holding(holder.as_ref()).await;
    let (awaited, stalled) = queued(ctx, &watched.stack.services, project).await;
    let marked = marked(ctx, &held);
    Ok(Gathered {
        measured: Measured {
            volumes: watched.volumes,
            root: watched.root,
            data,
            services,
            landing: watched.landing,
            held,
            awaited,
            stalled,
            marked,
        },
        holder,
    })
}

/// The moment this reading was taken, in seconds since the epoch.
fn now(ctx: &Ctx) -> u64 {
    ctx.clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The completed downloads the torrent client is still holding.
///
/// Only a torrent client has an answer — Usenet has no seeding to have — so a
/// stack with no torrent client, or one lemonfiber cannot authenticate to, holds
/// nothing rather than failing the reading.
async fn holding(holder: Option<&crate::qbittorrent::Qbittorrent>) -> Vec<Seeded> {
    match holder {
        Some(client) => client.seeding().await.unwrap_or_default(),
        None => Vec::new(),
    }
}

/// What the services are still waiting for, and which of it has stopped moving.
///
/// A service that will not answer contributes nothing to either. That is the safe
/// direction for the first — a download nothing is known to be waiting for is
/// judged on the filesystem's evidence rather than on a queue nobody read — and it
/// is the honest one for the second, since silence is not a stalled import.
async fn queued(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> (BTreeSet<String>, Vec<Stalled>) {
    let targets = servarr_targets(services, project);
    let read = futures_util::future::join_all(targets.iter().map(|target| async move {
        let service = target.open(&ctx.http, ctx.filesystem.as_ref()).await?;
        service.queue().await.ok()
    }))
    .await;

    let items: Vec<Queued> = read
        .into_iter()
        .flatten()
        .flat_map(|queue| queue.items)
        .collect();
    let awaited = items.iter().map(|item| item.title.clone()).collect();
    let stalled = items
        .iter()
        .filter(|item| item.is_stuck())
        .map(|item| Stalled {
            name: item.title.clone(),
            said: item.message.clone(),
        })
        .collect();
    (awaited, stalled)
}

/// Which of the downloads the operator has already asked to be left alone.
///
/// Read from the answers they gave the queue check rather than from a marker of
/// this command's own: having said once that an item is theirs to manage is having
/// said it, and asking again somewhere else would be this product forgetting.
fn marked(ctx: &Ctx, held: &[Seeded]) -> BTreeSet<String> {
    let accepted = crate::app::accepted::load(ctx);
    held.iter()
        .filter(|download| accepted.has(&format!("{}.{}", crate::queue::run::CHECK, download.name)))
        .map(|download| download.name.clone())
        .collect()
}

/// Take what the reading offered, and nothing else.
///
/// Each path is removed on its own so that one refusal does not stop the rest: a
/// file the operator's own account cannot touch is reported as left behind, with
/// the platform's words for why, and the room the others freed is still freed.
async fn reclaim(ctx: &Ctx, reckoned: &Reckoning, measured: &Measured) -> Reclaimed {
    let mut taken = Reclaimed {
        gone: Vec::new(),
        bytes: 0,
        left: Vec::new(),
    };
    for occupant in reckoned.offering(measured) {
        // A rehearsal says what would go and takes nothing, which is the same
        // promise every other write in this product makes.
        if ctx.dry_run {
            taken.gone.push(occupant.path.display().to_string());
            taken.bytes = taken.bytes.saturating_add(occupant.bytes);
            continue;
        }
        match ctx.eraser.erase(&occupant.path).await {
            Ok(()) => {
                taken.gone.push(occupant.path.display().to_string());
                taken.bytes = taken.bytes.saturating_add(occupant.bytes);
            }
            Err(fault) => taken.left.push(Left {
                at: occupant.path.display().to_string(),
                why: fault.message,
            }),
        }
    }
    taken
}

/// There is nowhere to measure.
fn nowhere() -> Problem {
    Problem::new(
        NOWHERE_TO_MEASURE,
        Severity::Error,
        "No data location is configured, so there is no disk to account for",
        "Where the media lives is what everything here is measured against, and \
         nothing has said where that is yet.",
        Remedy::new("Set the data location").with_detail("lemonfiber setup"),
    )
    .lies_in(Amiss::Asking)
}

/// The data location is there and will not be read.
fn unreadable(root: &Path, said: &str) -> Problem {
    Problem::new(
        WALK_REFUSED,
        Severity::Error,
        format!("The data location at {} could not be read", root.display()),
        "Nothing can be said about where the disk went without looking at what is \
         on it, and reporting an empty answer would read as an empty disk.",
        Remedy::new("Check that the account lemonfiber runs as can read the data location"),
    )
    .with_detail(said)
}

#[cfg(test)]
mod tests;

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

use super::targets::{
    committed_bytes, download_targets, project_directory, servarr_targets, torrent_client,
};
use super::Ctx;

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
pub(super) async fn space(ctx: &Ctx, confirm: bool) -> Result<Reckoning, Box<Problem>> {
    let measured = measure(ctx).await?;
    let mut reckoned = reckon(&measured);
    if confirm {
        reckoned.reclaimed = Some(reclaim(ctx, &reckoned, &measured).await);
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
pub(super) async fn admits(ctx: &Ctx) -> Result<(), Box<Problem>> {
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

/// Read everything one reckoning is made of.
async fn measure(ctx: &Ctx) -> Result<Measured, Box<Problem>> {
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

    let held = holding(ctx, &watched.stack.services, project).await;
    let (awaited, stalled) = queued(ctx, &watched.stack.services, project).await;
    let marked = marked(ctx, &held);
    Ok(Measured {
        volumes: watched.volumes,
        root: watched.root,
        data,
        services,
        landing: watched.landing,
        held,
        awaited,
        stalled,
        marked,
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
async fn holding(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<Seeded> {
    let targets = download_targets(services, project);
    match torrent_client(ctx, &targets) {
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
    let accepted = super::accepted::load(ctx);
    held.iter()
        .filter(|download| accepted.has(&format!("{}.{}", super::queue::CHECK, download.name)))
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
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use lemonfiber_fixtures::erasing::Erasing;
    use lemonfiber_fixtures::http::{Answer, Fake};
    use lemonfiber_fixtures::walking::Walking;

    use super::{admits, space};
    use crate::config::Settings;
    use crate::ports::filesystem::{FsKind, Identity, StorageFacts};
    use crate::ports::occupancy::Occupant;
    use crate::space::{Level, Standing};
    use crate::test_support::{a_context, env_at, nowhere, SeedFs};

    /// A walked file with a given number of names pointing at it.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// A volume of a terabyte with the given room left.
    fn facts(available: u64) -> StorageFacts {
        StorageFacts {
            point: PathBuf::from("/srv"),
            kind: FsKind::classify("ext4"),
            removable: false,
            available,
            total: 1_000_000_000_000,
        }
    }

    /// Settings naming a data location, and nothing else.
    fn measuring() -> Settings {
        Settings {
            data_root: Some(PathBuf::from("/srv/media")),
            ..Settings::default()
        }
    }

    /// The tree every case here walks: one download that was imported and one that
    /// nothing ever took.
    fn a_tree() -> Vec<Occupant> {
        vec![
            file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2),
            file("/srv/media/films/Imported/a.mkv", 8_000, 41, 2),
            file("/srv/media/downloads/Never.Taken/b.mkv", 3_000, 42, 1),
        ]
    }

    /// A context measuring a volume with the given room left, over the tree above.
    fn measuring_a_volume(available: u64) -> crate::app::Ctx {
        a_context()
            .settings(measuring())
            .build()
            .with_filesystem(Arc::new(
                SeedFs::keyed(None, None).with_facts(facts(available)),
            ))
            .surveying(Walking::holding(a_tree()))
    }

    #[tokio::test]
    async fn a_machine_with_no_data_location_is_told_to_set_one() {
        let ctx = a_context().build();
        let refused = space(&ctx, false).await;
        assert!(refused.is_err_and(|problem| problem.code == crate::space::NOWHERE_TO_MEASURE));
    }

    #[tokio::test]
    async fn a_data_location_that_will_not_be_read_is_a_refusal_rather_than_an_empty_disk() {
        // Reporting nothing on the disk would read as an empty disk, which is the
        // opposite of what an unreadable one means.
        let ctx = a_context()
            .settings(measuring())
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts(500))))
            .surveying(Walking::refusing("permission denied"));
        let refused = space(&ctx, false).await;
        assert!(
            refused.is_err_and(|problem| problem.code == crate::space::WALK_REFUSED
                && problem.detail.as_deref() == Some("permission denied"))
        );
    }

    #[tokio::test]
    async fn the_reckoning_measures_the_volume_and_walks_what_is_on_it() {
        // Asserted through the answer rather than unwrapped out of it: a closure
        // for the case that cannot happen is a line no run ever reaches, and the
        // coverage gate counts it against a file every one of whose cases passed.
        let reckoned = space(&measuring_a_volume(900_000_000_000), false).await;
        assert!(
            reckoned.is_ok_and(|reckoned| {
                let headings: Vec<String> = reckoned
                    .consumption
                    .iter()
                    .map(|line| line.category.heading())
                    .collect();
                reckoned.level == Level::Ample
                    && !reckoned.halted
                    && reckoned.reclaimed.is_none()
                    && headings.contains(&"downloads".to_owned())
                    && headings.contains(&"films".to_owned())
            }),
            "an ample volume, walked, with nothing asked for and nothing taken"
        );
    }

    #[tokio::test]
    async fn a_full_volume_halts_what_would_fetch_more_and_says_what_it_protects() {
        let ctx = measuring_a_volume(500);
        assert!(space(&ctx, false)
            .await
            .is_ok_and(|reckoned| reckoned.halted));

        let refused = admits(&ctx).await;
        assert!(refused
            .is_err_and(|problem| problem.code == crate::space::HALTED
                && problem.meaning.contains("database")));
    }

    #[tokio::test]
    async fn a_volume_with_room_lets_new_work_start() {
        assert!(admits(&measuring_a_volume(900_000_000_000)).await.is_ok());
    }

    #[tokio::test]
    async fn a_disk_nobody_could_measure_does_not_stop_work_on_a_guess() {
        // A machine with no data location configured has filled nothing, and a halt
        // is a claim about a volume rather than about the absence of a reading.
        let ctx = a_context().build();
        assert!(
            space(&ctx, false).await.is_err(),
            "and the reading still says so"
        );
        assert!(admits(&ctx).await.is_ok());
    }

    /// A context whose torrent client answers with one completed download of that
    /// name, and whose disk is the tree above with room to spare.
    ///
    /// `scratch` names the directory this run keeps its own files in, and every
    /// caller passes a different one: those files are written to a real disk and
    /// the cases run at the same time, so two sharing a directory would be one
    /// wiping the other's while it was reading it.
    fn holding_one(name: &str, scratch: &str) -> crate::app::Ctx {
        let body =
            format!("[{{\"name\":\"{name}\",\"size\":3000,\"uploaded\":0,\"downloaded\":3000}}]");
        let http = Fake::by_path(vec![
            ("/api/v2/auth/login", Answer::reply(200, "Ok.")),
            ("/api/v2/torrents/info", Answer::reply(200, body)),
        ]);
        a_context()
            .settings(Settings {
                env_file: Some(env_at(scratch, "a-recorded-password")),
                ..measuring()
            })
            .build()
            .with_http(http)
            .with_filesystem(Arc::new(
                SeedFs::keyed(None, None).with_facts(facts(900_000_000_000)),
            ))
            .surveying(Walking::holding(a_tree()))
    }

    #[tokio::test]
    async fn a_download_nothing_ever_linked_is_named_as_costing_nothing() {
        let reckoned = space(&holding_one("Never.Taken", "space-named"), false).await;
        assert!(
            reckoned.is_ok_and(|reckoned| reckoned
                .candidates
                .iter()
                .any(|candidate| candidate.standing == Standing::NeverImported)),
            "one name on disk is one nothing ever imported"
        );
    }

    #[tokio::test]
    async fn nothing_is_removed_until_an_answer_arrives_and_then_only_what_was_offered() {
        let erasing = Erasing::willing();
        let ctx =
            holding_one("Never.Taken", "space-offered").erasing(Arc::clone(&erasing) as Arc<_>);

        assert!(space(&ctx, false).await.is_ok());
        assert!(
            erasing.asked().is_empty(),
            "a reading removes nothing, whatever it found"
        );

        let taken = space(&ctx, true).await;
        assert_eq!(
            erasing.asked(),
            vec![PathBuf::from("/srv/media/downloads/Never.Taken/b.mkv")],
            "the imported one is not touched"
        );
        assert!(taken.is_ok_and(|taken| taken
            .reclaimed
            .is_some_and(|reclaimed| reclaimed.bytes == 3_000 && reclaimed.left.is_empty())));
    }

    #[tokio::test]
    async fn what_could_not_be_removed_is_reported_rather_than_counted_as_freed() {
        let erasing = Erasing::refusing("permission denied");
        let ctx =
            holding_one("Never.Taken", "space-refused").erasing(Arc::clone(&erasing) as Arc<_>);
        let taken = space(&ctx, true).await;
        assert!(taken.is_ok_and(
            |taken| taken.reclaimed.is_some_and(|reclaimed| reclaimed.bytes == 0
                && reclaimed.gone.is_empty()
                && reclaimed
                    .left
                    .first()
                    .is_some_and(|left| left.why == "permission denied"))
        ));
    }

    #[tokio::test]
    async fn a_rehearsal_says_what_would_go_and_takes_nothing() {
        let erasing = Erasing::willing();
        let ctx = holding_one("Never.Taken", "space-rehearsed")
            .erasing(Arc::clone(&erasing) as Arc<_>)
            .rehearsing();
        let taken = space(&ctx, true).await;
        assert!(erasing.asked().is_empty(), "a rehearsal removes nothing");
        assert!(
            taken.is_ok_and(|taken| taken
                .reclaimed
                .is_some_and(|reclaimed| reclaimed.gone.len() == 1 && reclaimed.bytes == 3_000))
        );
    }

    #[tokio::test]
    async fn a_download_the_operator_already_answered_for_is_left_alone() {
        // The marker is the answer they gave the queue check rather than one of this
        // command's own: having said once that an item is theirs to manage is having
        // said it, and asking again somewhere else would be this product forgetting.
        let ctx = holding_one("Never.Taken", "space-answered");
        let beside = ctx
            .settings
            .env_file
            .as_deref()
            .map(|env| env.with_file_name("accepted.json"));
        assert!(beside.is_some(), "the scratch machine keeps its answers");
        if let Some(at) = beside {
            let _ = std::fs::write(&at, "{\"checks\":[\"queue.Never.Taken\"]}");
        }

        let reckoned = space(&ctx, false).await;
        assert!(
            reckoned.is_ok_and(|reckoned| reckoned.candidates.iter().all(|candidate| {
                candidate.standing == Standing::LeftAlone && !candidate.offered()
            })),
            "what they answered for is never named as waste"
        );
    }

    /// A Servarr-shape service's own configuration, with the key it wrote.
    const KEYED: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

    /// A queue holding one item the service has stopped making progress on.
    const STALLED: &str = r#"{"totalRecords":1,"records":[
        {"title":"Never.Taken","trackedDownloadStatus":"warning",
         "trackedDownloadState":"importPending",
         "errorMessage":"No space left on device"}
    ]}"#;

    #[tokio::test]
    async fn an_import_that_has_stopped_is_named_with_what_is_on_disk_for_it() {
        // The service is answering, so what it says about its own queue is what the
        // report carries — verbatim, because a permission denial or a full disk in
        // its own words is worth more than any reading of one.
        let http = Fake::by_path(vec![
            ("/api/v2/auth/login", Answer::reply(200, "Ok.")),
            (
                "/api/v2/torrents/info",
                Answer::reply(
                    200,
                    r#"[{"name":"Never.Taken","size":3000,"uploaded":0,"downloaded":3000}]"#,
                ),
            ),
            ("/queue", Answer::reply(200, STALLED)),
            ("/history", Answer::reply(200, r#"{"records":[]}"#)),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env_at("space-stalled", "a-recorded-password")),
                ..measuring()
            })
            .build()
            .with_http(http)
            .with_filesystem(Arc::new(
                SeedFs::keyed(Some(KEYED), None).with_facts(facts(900_000_000_000)),
            ))
            .surveying(Walking::holding(a_tree()));

        // A service still waiting for it is also what stops it being called waste,
        // whatever the filesystem says about how many names point at its file — so
        // both halves are asserted over the one answer.
        let reckoned = space(&ctx, false).await;
        assert!(
            reckoned.is_ok_and(|reckoned| {
                reckoned.interrupted.iter().any(|stopped| {
                    stopped.name == "Never.Taken"
                        && stopped.said == "No space left on device"
                        && stopped.partial == 3_000
                }) && reckoned
                    .candidates
                    .iter()
                    .all(|candidate| !candidate.offered())
            }),
            "the import that stopped is named with what is on disk, and offered to nobody"
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_at_all_is_a_refusal_rather_than_an_empty_one() {
        let ctx = a_context()
            .over(nowhere())
            .settings(measuring())
            .build()
            .with_filesystem(Arc::new(
                SeedFs::keyed(None, None).with_facts(facts(900_000_000_000)),
            ))
            .surveying(Walking::holding(a_tree()));
        assert!(space(&ctx, false).await.is_err());
    }

    #[tokio::test]
    async fn a_stack_with_nowhere_to_read_its_services_files_measures_the_data_alone() {
        // An embedded stack that has not been materialised has no project directory,
        // so there is no directory the services keep their own files in to measure —
        // which is a volume absent from the report rather than one reported as empty.
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

        let ctx = a_context()
            .over(crate::stack::Source::Embedded(&EMBEDDED))
            .settings(measuring())
            .build()
            .with_filesystem(Arc::new(
                SeedFs::keyed(None, None).with_facts(facts(900_000_000_000)),
            ))
            .surveying(Walking::holding(a_tree()));
        let reckoned = space(&ctx, false).await;
        assert!(
            reckoned.is_ok_and(|reckoned| reckoned.volumes.len() == 1
                && reckoned
                    .consumption
                    .iter()
                    .all(|line| line.category.heading() != "the services' own files")),
            "one volume, and no line for files there is nowhere to keep"
        );
    }
}

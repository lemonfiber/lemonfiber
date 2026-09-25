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
use crate::test_support::{a_context, a_password, env_at, nowhere, SeedFs};

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
        .with_occupancy(Walking::holding(a_tree()))
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
        .with_occupancy(Walking::refusing("permission denied"));
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
    assert!(refused.is_err_and(
        |problem| problem.code == crate::space::HALTED && problem.meaning.contains("database")
    ));
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
        format!("[{{\"hash\":\"aa\",\"name\":\"{name}\",\"size\":3000,\"uploaded\":0,\"downloaded\":3000}}]");
    let http = Fake::by_path(vec![
        ("/api/v2/auth/login", Answer::reply(200, "Ok.")),
        ("/api/v2/torrents/info", Answer::reply(200, body)),
    ]);
    a_context()
        .settings(Settings {
            env_file: Some(env_at(scratch, &a_password())),
            ..measuring()
        })
        .build()
        .with_http(http)
        .with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_facts(facts(900_000_000_000)),
        ))
        .with_occupancy(Walking::holding(a_tree()))
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
        holding_one("Never.Taken", "space-offered").with_eraser(Arc::clone(&erasing) as Arc<_>);

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
        holding_one("Never.Taken", "space-refused").with_eraser(Arc::clone(&erasing) as Arc<_>);
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
        .with_eraser(Arc::clone(&erasing) as Arc<_>)
        .rehearsing();
    let taken = space(&ctx, true).await;
    assert!(erasing.asked().is_empty(), "a rehearsal removes nothing");
    assert!(taken.is_ok_and(|taken| taken
        .reclaimed
        .is_some_and(|reclaimed| reclaimed.gone.len() == 1 && reclaimed.bytes == 3_000)));
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
                r#"[{"hash":"aa","name":"Never.Taken","size":3000,"uploaded":0,"downloaded":3000}]"#,
            ),
        ),
        ("/queue", Answer::reply(200, STALLED)),
        ("/history", Answer::reply(200, r#"{"records":[]}"#)),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("space-stalled", &a_password())),
            ..measuring()
        })
        .build()
        .with_http(http)
        .with_filesystem(Arc::new(
            SeedFs::keyed(Some(KEYED), None).with_facts(facts(900_000_000_000)),
        ))
        .with_occupancy(Walking::holding(a_tree()));

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
        .with_occupancy(Walking::holding(a_tree()));
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
        .with_occupancy(Walking::holding(a_tree()));
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

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::ports::{Stopped, Waited, STAMPED};
use lemonfiber_fixtures::scratch::Scratch;

use super::kept::Kept;
use super::pacing::Stopwatch;
use super::run::{read, Starting};
use super::{Copied, Copying, Meter as _, Running, State, What, EXISTING_PART};
use crate::backup::{self, Scope};
use crate::config::paths::Paths;
use crate::instant::{Instant, MINUTE};

const MIB: u64 = 1 << 20;
const STALLED: u64 = 5 * MINUTE;

fn paths(scratch: &Scratch) -> Paths {
    Paths::at(scratch.path(), scratch.path())
}

fn whole_stack(paths: &Paths) -> Vec<(String, String)> {
    backup::plan(paths, &Scope::WholeStack, &[]).areas()
}

fn starting(stopwatch: &Arc<Waited>, limit: Option<u64>) -> Starting {
    Starting::on(
        Stopped::at(STAMPED.seconds()),
        Arc::clone(stopwatch) as Arc<dyn Stopwatch>,
        Some("9f2c1a7e04b3d815".to_owned()),
        limit,
    )
}

fn running(paths: &Paths, at: Instant) -> Option<Running> {
    read(&paths.copying(), at, STALLED).running
}

#[test]
fn a_limited_backup_moves_no_faster_than_its_limit_and_no_slower() {
    let stopwatch = Waited::started();
    let limit = MIB;
    let meter = starting(&stopwatch, Some(limit)).begin(What::Backup, Scope::WholeStack, 0, vec![]);
    let chunk = 64 * 1024;
    let mut moved = 0_u64;
    for _ in 0..160 {
        stopwatch.pass(Duration::from_millis(1));
        meter.wrote(chunk);
        moved += chunk;
        let elapsed = stopwatch.elapsed().as_nanos();
        assert!(
            u128::from(moved) * 1_000_000_000 <= u128::from(limit) * elapsed,
            "{moved} bytes moved in {elapsed} ns, faster than {limit} a second"
        );
    }
    let least = u128::from(moved) * 1_000_000_000 / u128::from(limit);
    assert!(
        stopwatch.elapsed().as_nanos() <= least + 1_000_000,
        "held back further than the limit asks"
    );
}

#[test]
fn a_restore_is_never_slowed_whatever_the_limit() {
    let stopwatch = Waited::started();
    let meter = starting(&stopwatch, Some(MIB)).begin(What::Restore, Scope::WholeStack, 0, vec![]);
    for _ in 0..100 {
        meter.wrote(MIB);
    }
    assert_eq!(stopwatch.elapsed().as_nanos(), 0, "a restore waited");
}

#[test]
fn a_backup_with_no_limit_never_waits_and_reads_working() {
    let scratch = Scratch::named("copying-unlimited");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let meter = starting(&stopwatch, None).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        10 * MIB,
        whole_stack(&paths),
    );
    meter.wrote(10 * MIB);
    assert_eq!(stopwatch.elapsed().as_nanos(), 0);
    assert_eq!(
        running(&paths, STAMPED).map(|running| running.state),
        Some(State::Working)
    );
}

#[test]
fn a_running_copy_is_read_from_its_reading_and_none_once_it_ends() {
    let scratch = Scratch::named("copying-read");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let meter = starting(&stopwatch, Some(50 * MIB))
        .kept_under(&paths)
        .begin(
            What::Backup,
            Scope::WholeStack,
            2 * MIB,
            whole_stack(&paths),
        );
    meter.writing("services/sonarr/config.xml");
    stopwatch.pass(Duration::from_secs(1));
    meter.wrote(MIB);

    assert_eq!(
        running(&paths, STAMPED),
        Some(Running {
            what: What::Backup,
            scope: Copied::WholeStack,
            job: Some("9f2c1a7e04b3d815".to_owned()),
            started: STAMPED,
            written: MIB,
            whole: 2 * MIB,
            part: "sonarr configuration".to_owned(),
            state: State::Paced { limit: 50 * MIB },
        })
    );

    drop(meter);
    assert_eq!(read(&paths.copying(), STAMPED, STALLED), Copying::default());
}

#[test]
fn a_restore_learns_its_whole_from_the_writer() {
    let scratch = Scratch::named("copying-restore");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let meter = starting(&stopwatch, Some(MIB)).kept_under(&paths).begin(
        What::Restore,
        Scope::WholeStack,
        0,
        whole_stack(&paths),
    );
    meter.whole(3 * MIB);
    meter.writing("config/.env");
    let read = running(&paths, STAMPED);
    assert_eq!(read.as_ref().map(|running| running.whole), Some(3 * MIB));
    assert_eq!(
        read.as_ref().map(|running| running.part.as_str()),
        Some("lemonfiber configuration")
    );
    assert_eq!(read.map(|running| running.state), Some(State::Working));
}

#[test]
fn a_copy_that_writes_nothing_for_the_minutes_set_reads_stalled_even_when_paced() {
    let scratch = Scratch::named("copying-stalled");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let _meter = starting(&stopwatch, Some(MIB)).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        MIB,
        whole_stack(&paths),
    );

    assert_eq!(
        running(&paths, STAMPED.later(STALLED - 1)).map(|running| running.state),
        Some(State::Paced { limit: MIB })
    );
    assert_eq!(
        running(&paths, STAMPED.later(STALLED)).map(|running| running.state),
        Some(State::Stalled { wrote: STAMPED })
    );
}

#[test]
fn a_reading_left_by_a_copy_that_is_gone_reads_as_none() {
    let scratch = Scratch::named("copying-gone");
    let paths = paths(&scratch);
    let dir = paths.copying();
    let left = Kept {
        what: What::Backup,
        scope: Scope::WholeStack,
        job: None,
        started: STAMPED,
        written: 1,
        whole: 2,
        part: String::new(),
        wrote: STAMPED,
        limit: None,
    };
    assert!(crate::config::store::write(
        &super::kept::reading(&dir),
        &serde_json::to_string(&left).unwrap_or_default()
    )
    .is_ok());
    assert!(std::fs::write(dir.join("reading.lock"), "").is_ok());

    assert_eq!(read(&dir, STAMPED, STALLED), Copying::default());
}

#[cfg(unix)]
#[test]
fn a_reading_anybody_else_could_have_written_is_not_believed() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::named("copying-loosened");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let _meter = starting(&stopwatch, None).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        MIB,
        whole_stack(&paths),
    );
    let at = super::kept::reading(&paths.copying());
    assert!(running(&paths, STAMPED).is_some());

    assert!(std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).is_ok());
    assert_eq!(running(&paths, STAMPED), None);

    let elsewhere = scratch.path().join("planted.json");
    assert!(std::fs::rename(&at, &elsewhere).is_ok());
    assert!(std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o600)).is_ok());
    assert!(std::os::unix::fs::symlink(&elsewhere, &at).is_ok());
    assert_eq!(running(&paths, STAMPED), None);
}

#[test]
fn a_key_is_told_no_job_and_of_an_existing_setup_neither_its_project_its_trees_nor_a_host_path() {
    let scratch = Scratch::named("copying-for-a-key");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let scope = Scope::existing("media", &["/srv/media/sonarr".to_owned()]);
    let areas = backup::plan(&paths, &scope, &[]).areas();
    let meter =
        starting(&stopwatch, None)
            .kept_under(&paths)
            .begin(What::Backup, scope, MIB, areas);
    meter.writing("existing/0/config.xml");

    let Some(operators) = running(&paths, STAMPED) else {
        unreachable!("the copy begun above keeps a reading");
    };
    assert_eq!(operators.job.as_deref(), Some("9f2c1a7e04b3d815"));
    assert_eq!(operators.part, "/srv/media/sonarr");
    assert!(matches!(
        &operators.scope,
        Copied::Existing { project: Some(project), trees: Some(trees) }
            if project == "media" && trees.len() == 1
    ));

    let keys = operators.clone().for_a_key();
    assert_eq!(
        keys,
        Running {
            job: None,
            scope: Copied::Existing {
                project: None,
                trees: None,
            },
            part: EXISTING_PART.to_owned(),
            ..operators.clone()
        }
    );
    let said = serde_json::to_string(&keys).unwrap_or_default();
    for withheld in [
        "media",
        "/srv",
        "9f2c1a7e04b3d815",
        "project",
        "trees",
        "job",
    ] {
        assert!(!said.contains(withheld), "{withheld} in {said}");
    }

    let whole = Running {
        scope: Copied::WholeStack,
        part: "sonarr configuration".to_owned(),
        ..operators
    };
    let told = crate::app::Outcome::Copying(Copying {
        running: Some(whole.clone()),
    })
    .for_a_key();
    assert_eq!(
        told,
        crate::app::Outcome::Copying(Copying {
            running: Some(Running { job: None, ..whole }),
        })
    );
    assert_eq!(Copying::default().for_a_key(), Copying::default());
}

/// Where the copy [`dies_holding_a_reading_where_another_test_asks`] begins keeps its reading.
const HOLDING: &str = "LEMONFIBER_TEST_DYING_COPY";

/// What that copy says once it holds its reading.
const HELD: &str = "a dying copy holds its reading";

#[test]
fn dies_holding_a_reading_where_another_test_asks() {
    let Some(root) = std::env::var_os(HOLDING) else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let paths = Paths::at(&root, &root);
    let meter = starting(&Waited::started(), None).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        MIB,
        whole_stack(&paths),
    );
    std::mem::forget(meter);
    println!("{HELD}");
    let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut Vec::new());
    std::process::exit(0);
}

#[test]
fn a_reading_kept_by_a_process_of_this_user_that_has_died_reads_as_none() {
    use std::io::BufRead as _;
    use std::process::{Command, Stdio};
    let scratch = Scratch::named("copying-died");
    let paths = paths(&scratch);
    let Ok(mut dying) = std::env::current_exe().and_then(|exe| {
        Command::new(exe)
            .args([
                "--exact",
                "copying::tests::dies_holding_a_reading_where_another_test_asks",
                "--nocapture",
            ])
            .env(HOLDING, scratch.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
    }) else {
        unreachable!("the test binary runs one of its own tests");
    };
    let held = dying.stdout.take().map(|said| {
        std::io::BufReader::new(said)
            .lines()
            .map_while(Result::ok)
            .any(|line| line.contains(HELD))
    });
    assert_eq!(held, Some(true));
    assert!(running(&paths, STAMPED).is_some());

    drop(dying.stdin.take());
    assert!(dying.wait().is_ok_and(|ended| ended.success()));
    assert!(super::kept::reading(&paths.copying()).exists());
    assert_eq!(running(&paths, STAMPED), None);
}

#[test]
fn an_entry_no_area_holds_leaves_the_part_being_written() {
    let scratch = Scratch::named("copying-unheld");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let meter = starting(&stopwatch, None).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        MIB,
        whole_stack(&paths),
    );
    meter.writing("services/sonarr/config.xml");
    meter.writing("manifest.json");
    assert_eq!(
        running(&paths, STAMPED).map(|running| running.part),
        Some("sonarr configuration".to_owned())
    );
}

#[test]
fn the_real_stopwatch_waits_as_long_as_it_is_asked() {
    let stopwatch = super::pacing::Monotonic::started();
    stopwatch.wait(Duration::from_millis(2));
    assert!(stopwatch.elapsed() >= Duration::from_millis(2));
}

#[test]
fn a_second_copy_while_one_runs_leaves_the_first_ones_reading() {
    let scratch = Scratch::named("copying-second");
    let paths = paths(&scratch);
    let stopwatch = Waited::started();
    let first = starting(&stopwatch, None).kept_under(&paths).begin(
        What::Backup,
        Scope::WholeStack,
        MIB,
        whole_stack(&paths),
    );
    stopwatch.pass(Duration::from_secs(1));
    first.wrote(1);
    let second = starting(&stopwatch, None).kept_under(&paths).begin(
        What::Restore,
        Scope::WholeStack,
        9 * MIB,
        whole_stack(&paths),
    );
    second.wrote(7);
    drop(second);

    let read = running(&paths, STAMPED);
    assert_eq!(
        read.as_ref().map(|running| running.what),
        Some(What::Backup)
    );
    assert_eq!(read.map(|running| running.written), Some(1));
}

#[test]
fn an_entry_is_named_by_its_area_and_by_its_service_inside_the_services_area() {
    let scratch = Scratch::named("copying-parts");
    let paths = paths(&scratch);
    let areas = whole_stack(&paths);
    let part = |entry: &str| backup::part(entry, &areas);
    assert_eq!(
        part("services/sonarr/sonarr.db").as_deref(),
        Some("sonarr configuration")
    );
    assert_eq!(
        part("./services/radarr").as_deref(),
        Some("radarr configuration")
    );
    assert_eq!(part("services").as_deref(), Some("service configuration"));
    assert_eq!(
        part("config/.env").as_deref(),
        Some("lemonfiber configuration")
    );
    assert_eq!(
        part("stack/compose.yaml").as_deref(),
        Some("materialised stack")
    );
    assert_eq!(part("manifest.json"), None);
    assert_eq!(part("configured/x"), None);

    let one = backup::plan(
        &paths,
        &Scope::Service {
            name: "sonarr".to_owned(),
        },
        &[],
    )
    .areas();
    assert_eq!(
        backup::part("services/sonarr/sonarr.db", &one).as_deref(),
        Some("sonarr configuration")
    );
}

#[test]
fn the_backup_limit_is_whole_mebibytes_a_second_or_off() {
    use crate::settings::own::{backup_limit, code_count, COPY_STALLED};
    assert_eq!(backup_limit("off"), Ok(None));
    assert_eq!(backup_limit("1"), Ok(Some(1)));
    assert_eq!(backup_limit("100000"), Ok(Some(100_000)));
    for refused in ["0", "100001", "fast", "", "OFF", "-5"] {
        assert!(backup_limit(refused).is_err(), "{refused} was taken");
    }
    assert_eq!(code_count(&COPY_STALLED, "1"), Ok(1));
    assert_eq!(code_count(&COPY_STALLED, "1440"), Ok(1440));
    for refused in ["0", "1441", "soon"] {
        assert!(
            code_count(&COPY_STALLED, refused).is_err(),
            "{refused} was taken"
        );
    }
}

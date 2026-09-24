use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::findings::writable;
use super::{
    Check, Crowded, Environment, Finding, StorageCheck, Verdict, COPY_ONLY, DEGRADED, ROOT_ABSENT,
    ROOT_UNWRITABLE, SERVICE_DENIED, SPACE_LOW,
};
use crate::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};

/// Room to spare, so a test that is not about space never trips the floor.
const AMPLE: u64 = 500 * 1024 * 1024 * 1024;

/// A one-terabyte volume, the size the ample figure is free space on.
const CAPACITY: u64 = 1024 * 1024 * 1024 * 1024;

/// Facts for a filesystem, with a capacity a test does not otherwise care
/// about filled in generously.
fn facts(kind: FsKind, removable: bool) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from("/"),
        kind,
        removable,
        available: AMPLE,
        total: CAPACITY,
    }
}

/// A filesystem whose every answer the test scripts. Identity is asked twice
/// — of the original and of the link — and told apart by the name asked
/// about, so a test can make the two disagree.
struct Bench {
    resolves: Result<PathBuf, Fault>,
    writes: Result<(), Fault>,
    original: Result<Identity, Fault>,
    links: Result<(), Fault>,
    confirmed: Result<Identity, Fault>,
    facts: StorageFacts,
    remembered: Option<String>,
    owner: Option<Ownership>,
    /// What the check wrote to the state file, so a test can assert whether —
    /// and with what — the baseline was recorded.
    recorded: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Bench {
    /// A healthy local filesystem: resolves, writes, links, and reports the
    /// two names as one file on a filesystem that links, with room to spare
    /// and no capability remembered from before.
    fn healthy() -> Self {
        Self {
            resolves: Ok(PathBuf::from("/data")),
            writes: Ok(()),
            original: Ok(Identity { file: 7, links: 1 }),
            links: Ok(()),
            confirmed: Ok(Identity { file: 7, links: 2 }),
            facts: facts(FsKind::Linking("apfs".to_owned()), false),
            remembered: None,
            owner: None,
            recorded: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl FileSystem for Bench {
    async fn canonicalize(&self, _path: &Path) -> Result<PathBuf, Fault> {
        self.resolves.clone()
    }
    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        self.writes.clone()
    }
    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        self.links.clone()
    }
    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        if path.to_string_lossy().ends_with(".link") {
            self.confirmed.clone()
        } else {
            self.original.clone()
        }
    }
    async fn remove(&self, _path: &Path) {}
    async fn read(&self, _path: &Path) -> Option<String> {
        self.remembered.clone()
    }
    async fn write(&self, _path: &Path, contents: &str) {
        if let Ok(mut log) = self.recorded.lock() {
            log.push(contents.to_owned());
        }
    }
    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        self.owner
    }
}

#[async_trait]
impl Storage for Bench {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        self.facts.clone()
    }
}

/// Off native Linux, so the permission finding skips and the tests that are
/// not about it are undisturbed by it.
async fn run(bench: Bench, root: Option<&str>) -> Vec<Finding> {
    StorageCheck::new(
        Arc::new(bench),
        root.map(PathBuf::from),
        None,
        Environment::MacOs,
        None,
        None,
        Vec::new(),
    )
    .run()
    .await
}

/// A run carrying a committed-bytes figure, so the space finding projects
/// exhaustion from it rather than guarding the raw free space.
async fn run_committed(bench: Bench, root: &str, committed: Option<u64>) -> Vec<Finding> {
    StorageCheck::new(
        Arc::new(bench),
        Some(PathBuf::from(root)),
        None,
        Environment::MacOs,
        None,
        committed,
        Vec::new(),
    )
    .run()
    .await
}

/// The same run, but with a place to remember what was seen, so a regression
/// can be noticed between one call and the next.
async fn run_remembering(bench: Bench, root: &str) -> Vec<Finding> {
    StorageCheck::new(
        Arc::new(bench),
        Some(PathBuf::from(root)),
        Some(PathBuf::from("/state/storage-state.json")),
        Environment::MacOs,
        None,
        None,
        Vec::new(),
    )
    .run()
    .await
}

/// A run on native Linux, where host ownership gates the services, with the
/// service user configured.
async fn run_on_native_linux(bench: Bench, service_user: (u32, u32)) -> Vec<Finding> {
    StorageCheck::new(
        Arc::new(bench),
        Some(PathBuf::from("/data")),
        None,
        Environment::LinuxNative,
        Some(service_user),
        None,
        Vec::new(),
    )
    .run()
    .await
}

/// A run whose stack splits the data location between two mounts, so the half of
/// the answer the probe cannot see arrives beside the half it can.
async fn run_with_split_mounts(bench: Bench) -> Vec<Finding> {
    StorageCheck::new(
        Arc::new(bench),
        Some(PathBuf::from("/data")),
        None,
        Environment::MacOs,
        None,
        None,
        vec![Crowded {
            service: "sonarr".to_owned(),
            mounts: vec![
                "${DATA_ROOT}/downloads:/downloads".to_owned(),
                "${DATA_ROOT}/media:/media".to_owned(),
            ],
        }],
    )
    .run()
    .await
}

fn verdict<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Verdict> {
    findings
        .iter()
        .find(|finding| finding.check == check)
        .map(|finding| &finding.verdict)
}

#[tokio::test]
async fn a_root_that_links_passes_and_derives_the_local_mode() {
    let findings = run(Bench::healthy(), Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("apfs")
    ));
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("local")
    ));
}

#[tokio::test]
async fn a_working_link_on_removable_media_is_the_external_mode() {
    let bench = Bench {
        facts: facts(FsKind::Linking("apfs".to_owned()), true),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("external")
    ));
}

#[tokio::test]
async fn exfat_is_named_specifically_as_the_reason_it_cannot_link() {
    let bench = Bench {
        links: Err(Fault::new("operation not permitted")),
        facts: facts(FsKind::ExFat, true),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Warn(problem))
            if problem.code == COPY_ONLY && problem.summary.contains("exFAT")
    ));
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("copy")
    ));
}

#[tokio::test]
async fn a_network_share_that_cannot_link_derives_the_nas_mode() {
    let bench = Bench {
        links: Err(Fault::new("not supported")),
        facts: facts(FsKind::Nfs, false),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("nas")
    ));
}

#[tokio::test]
async fn a_filesystem_that_does_not_link_but_names_nothing_still_warns() {
    let bench = Bench {
        links: Err(Fault::new("nope")),
        facts: facts(FsKind::Unknown("weirdfs".to_owned()), false),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
    ));
}

#[tokio::test]
async fn a_root_that_cannot_be_written_to_fails_and_derives_no_mode() {
    let bench = Bench {
        writes: Err(Fault::new("permission denied")),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Fail(problem)) if problem.code == ROOT_UNWRITABLE
    ));
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_root_that_cannot_be_reached_is_reported_as_absent() {
    let bench = Bench {
        resolves: Err(Fault::new("no such file or directory")),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Fail(problem)) if problem.code == ROOT_ABSENT
    ));
}

#[tokio::test]
async fn a_link_that_cannot_be_confirmed_is_unverified_not_passed() {
    let bench = Bench {
        confirmed: Ok(Identity {
            file: 999,
            links: 1,
        }),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn an_unconfigured_machine_is_told_to_run_setup_rather_than_shown_an_error() {
    let findings = run(Bench::healthy(), None).await;
    assert!(matches!(
        verdict(&findings, "storage"),
        Some(Verdict::Skipped { reason }) if reason.contains("setup")
    ));
}

#[tokio::test]
async fn a_volume_with_room_reports_its_free_space() {
    let findings = run(Bench::healthy(), Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("free of")
    ));
}

#[tokio::test]
async fn a_nearly_full_volume_warns_about_space() {
    let mut low = facts(FsKind::Linking("ext4".to_owned()), false);
    low.available = 2 * 1024 * 1024 * 1024;
    let findings = run(
        Bench {
            facts: low,
            ..Bench::healthy()
        },
        Some("/data"),
    )
    .await;
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Warn(problem)) if problem.code == SPACE_LOW
    ));
}

#[tokio::test]
async fn committed_downloads_that_will_not_fit_project_exhaustion() {
    // Room now (the healthy bench reports ample free space), but the download
    // clients still have to write nearly all of it — so what is left once the
    // queue lands is below the floor, and the warning arrives before the disk
    // actually fills.
    let committed = 495 * 1024 * 1024 * 1024;
    let findings = run_committed(Bench::healthy(), "/data", Some(committed)).await;
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Warn(problem))
            if problem.code == SPACE_LOW && problem.summary.contains("projected to run out")
    ));
}

#[tokio::test]
async fn committed_downloads_that_fit_pass_and_still_name_what_is_coming() {
    // The queue fits with room to spare: a pass, but the note still states what
    // is on its way so a comfortable pass that is only comfortable because the
    // queue is small still says so.
    let committed = 100 * 1024 * 1024 * 1024;
    let findings = run_committed(Bench::healthy(), "/data", Some(committed)).await;
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("still to land")
    ));
}

#[tokio::test]
async fn free_space_is_reported_even_when_the_root_cannot_be_written_to() {
    let bench = Bench {
        writes: Err(Fault::new("permission denied")),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    // A full disk and an unwritable one are different problems; the operator
    // gets both rather than the write failure hiding the space figure.
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Pass { .. })
    ));
}

#[tokio::test]
async fn a_volume_whose_size_cannot_be_read_is_unverified_not_reported_full() {
    let mut unreadable = facts(FsKind::Linking("ext4".to_owned()), false);
    unreadable.total = 0;
    unreadable.available = 0;
    let findings = run(
        Bench {
            facts: unreadable,
            ..Bench::healthy()
        },
        Some("/data"),
    )
    .await;
    assert!(matches!(
        verdict(&findings, "storage.space"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_location_that_used_to_link_and_stopped_is_reported_as_a_regression() {
    let bench = Bench {
        links: Err(Fault::new("not supported")),
        remembered: Some(r#"{"hardlinks":true}"#.to_owned()),
        ..Bench::healthy()
    };
    let recorded = bench.recorded.clone();
    let findings = run_remembering(bench, "/data").await;
    assert!(
        matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Fail(problem)) if problem.code == DEGRADED
        ),
        "a lost capability is louder than one that was never there"
    );
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Warn(problem)) if problem.code == DEGRADED
    ));
    // The failure is not written over the good baseline, so the next run
    // still sees it and keeps shouting rather than downgrading to a warning.
    assert!(
        recorded.lock().is_ok_and(|log| log.is_empty()),
        "a regression must not overwrite the known-good baseline"
    );
}

#[tokio::test]
async fn a_location_that_never_linked_is_copy_mode_not_a_regression() {
    let bench = Bench {
        links: Err(Fault::new("not supported")),
        remembered: Some(r#"{"hardlinks":false}"#.to_owned()),
        ..Bench::healthy()
    };
    let findings = run_remembering(bench, "/data").await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
    ));
}

#[tokio::test]
async fn an_unreadable_baseline_is_ignored_rather_than_trusted() {
    let bench = Bench {
        links: Err(Fault::new("not supported")),
        remembered: Some("not json at all".to_owned()),
        ..Bench::healthy()
    };
    let findings = run_remembering(bench, "/data").await;
    // A corrupt baseline cannot say the capability was ever there, so it is a
    // plain copy-mode location rather than a regression against nonsense.
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
    ));
}

#[tokio::test]
async fn a_healthy_run_records_what_it_saw_for_next_time() {
    let bench = Bench::healthy();
    let recorded = bench.recorded.clone();
    let findings = run_remembering(bench, "/data").await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Pass { .. })
    ));
    // A working link is written down, so a later run can notice if it stops.
    assert!(
        recorded
            .lock()
            .is_ok_and(|log| log.iter().any(|written| written.contains("true"))),
        "the working link is recorded"
    );
}

#[tokio::test]
async fn a_link_whose_identity_reads_as_zero_is_not_taken_as_proof() {
    // A filesystem that reports no usable file identity leaves nothing to
    // compare; two zeroes are equal but prove nothing, so the link is
    // unverified rather than passed.
    let bench = Bench {
        original: Ok(Identity { file: 0, links: 1 }),
        confirmed: Ok(Identity { file: 0, links: 2 }),
        ..Bench::healthy()
    };
    let findings = run(bench, Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_run_that_could_not_confirm_the_link_records_nothing() {
    // An unconfirmed result must not overwrite a known-good baseline, so the
    // recording step is skipped rather than writing an uncertain answer.
    let bench = Bench {
        confirmed: Ok(Identity {
            file: 999,
            links: 1,
        }),
        ..Bench::healthy()
    };
    let findings = run_remembering(bench, "/data").await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn where_docker_maps_ownership_the_permission_check_does_not_apply() {
    let findings = run(Bench::healthy(), Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.permissions"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn an_unconfigured_service_user_skips_the_permission_check_on_native_linux() {
    let findings = StorageCheck::new(
        Arc::new(Bench::healthy()),
        Some(PathBuf::from("/data")),
        None,
        Environment::LinuxNative,
        None,
        None,
        Vec::new(),
    )
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "storage.permissions"),
        Some(Verdict::Skipped { reason }) if reason.contains("not configured")
    ));
}

#[tokio::test]
async fn ownership_that_cannot_be_read_leaves_the_permission_unverified() {
    // The healthy bench reports no ownership; on native Linux with a service
    // user that is the "could not read it" case.
    let findings = run_on_native_linux(Bench::healthy(), (1000, 1000)).await;
    assert!(matches!(
        verdict(&findings, "storage.permissions"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_data_root_the_services_can_write_passes() {
    let bench = Bench {
        owner: Some(Ownership {
            uid: 1000,
            gid: 1000,
            mode: 0o755,
        }),
        ..Bench::healthy()
    };
    let findings = run_on_native_linux(bench, (1000, 1000)).await;
    assert!(matches!(
        verdict(&findings, "storage.permissions"),
        Some(Verdict::Pass { .. })
    ));
}

#[tokio::test]
async fn a_root_the_operator_owns_but_the_services_cannot_write_is_its_own_failure() {
    // Owned by root, world-unwritable: the operator running the check may
    // well own it, yet the containers running as 1000:1000 cannot write, and
    // that is a different problem from the operator being unable to.
    let bench = Bench {
        owner: Some(Ownership {
            uid: 0,
            gid: 0,
            mode: 0o755,
        }),
        ..Bench::healthy()
    };
    let findings = run_on_native_linux(bench, (1000, 1000)).await;
    assert!(matches!(
        verdict(&findings, "storage.permissions"),
        Some(Verdict::Fail(problem)) if problem.code == SERVICE_DENIED
    ));
    // The hardlink finding, which speaks for the operator, is untroubled —
    // the two permission problems are reported apart.
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Pass { .. })
    ));
}

#[test]
fn writing_is_judged_by_the_class_that_owns_the_file() {
    let owns = |mode| Ownership {
        uid: 5,
        gid: 5,
        mode,
    };
    // Owner class, even when the group or other bits would be kinder.
    assert!(writable(owns(0o200), 5, 9));
    assert!(!writable(owns(0o077), 5, 5));
    // Group class, when the user is not the owner but shares the group.
    assert!(writable(
        Ownership {
            uid: 0,
            gid: 5,
            mode: 0o020
        },
        5,
        5
    ));
    // Other class, when neither the user nor the group owns it.
    assert!(writable(
        Ownership {
            uid: 0,
            gid: 0,
            mode: 0o002
        },
        5,
        5
    ));
    assert!(!writable(
        Ownership {
            uid: 0,
            gid: 0,
            mode: 0o750
        },
        5,
        5
    ));
    // Root is bound by no bits: a container running as uid 0 writes a
    // directory it does not own and has no mode share in.
    assert!(writable(
        Ownership {
            uid: 5,
            gid: 5,
            mode: 0o700
        },
        0,
        0
    ));
}

#[tokio::test]
async fn a_leftover_probe_link_does_not_read_as_an_inability_to_hardlink() {
    // A previous run interrupted between the link and its cleanup leaves the
    // link behind. Against a real filesystem that hardlinks fine, the check
    // must clear it and still pass, not conclude the volume cannot link.
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-storage-{}-leftover",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(".lemonfiber-hardlink-probe.link"), "stale");

    let findings = StorageCheck::new(
        Arc::new(lemonfiber_adapters::Disk),
        Some(dir.clone()),
        None,
        Environment::MacOs,
        None,
        None,
        Vec::new(),
    )
    .run()
    .await;
    // A stale probe link must not be mistaken for a filesystem that cannot
    // link: the check clears it first and still passes.
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Pass { .. })
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_location_that_links_still_says_the_containers_cannot() {
    // The trap this exists for. On this machine the data location is one
    // filesystem and the probe passes; inside the containers the downloads and the
    // library are on opposite sides of a boundary, and every import copies. An
    // operator shown only the first half has been told the wrong thing.
    let findings = run_with_split_mounts(Bench::healthy()).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Pass { .. })
    ));
    assert!(matches!(
        verdict(&findings, "storage.single-mount"),
        Some(Verdict::Warn(problem)) if problem.summary.contains("sonarr")
    ));
}

#[tokio::test]
async fn the_layout_is_reported_even_where_the_location_cannot_be_reached() {
    // A drive that is not plugged in says nothing about how the stack is written,
    // and the operator can act on the layout from anywhere. Withholding it until
    // the disk comes back would hold one answer hostage to another.
    let unreachable = Bench {
        resolves: Err(Fault::new("no such file or directory")),
        ..Bench::healthy()
    };
    let findings = run_with_split_mounts(unreachable).await;
    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Fail(_))
    ));
    assert!(matches!(
        verdict(&findings, "storage.single-mount"),
        Some(Verdict::Warn(_))
    ));
}

#[tokio::test]
async fn a_stack_that_mounts_the_location_once_says_so_beside_the_probe() {
    let findings = run(Bench::healthy(), Some("/data")).await;
    assert!(matches!(
        verdict(&findings, "storage.single-mount"),
        Some(Verdict::Pass { .. })
    ));
}

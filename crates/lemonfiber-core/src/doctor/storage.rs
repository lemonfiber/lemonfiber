//! Proving the data root can hardlink, rather than trusting that it can.
//!
//! The whole media pipeline rests on one property: downloads and the library
//! share a filesystem, so importing a file links it rather than copying it.
//! When that breaks nothing announces it — imports still succeed, the library
//! still fills — and the only symptoms are a disk consuming twice what it should
//! and torrents that cannot seed from the library copy, both found late.
//!
//! So capability is measured, never inferred: create a file under the data root,
//! link it, and check the two names point at one underlying file. A filesystem's
//! type only names *why* a link failed once the link has been tried — exFAT
//! cannot link at all, a network share usually cannot — and that name is added
//! to the finding rather than standing in for the test.
//!
//! The mode the stack should run in follows from the result. It is derived here,
//! never chosen: the operator picks a location, and this determines what the
//! location can do.
//!
//! See `.docs/architecture/module-layout.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::filesystem::{FileSystem, Identity, StorageFacts};

/// Raised when the data root cannot hardlink, so imports must copy.
pub const COPY_ONLY: Code = Code::new("STORAGE-1");

/// Raised when the data root exists but cannot be written to.
pub const ROOT_UNWRITABLE: Code = Code::new("STORAGE-2");

/// Raised when the data root is not there to test.
pub const ROOT_ABSENT: Code = Code::new("STORAGE-3");

/// Raised when the volume holding the data root is nearly full.
pub const SPACE_LOW: Code = Code::new("STORAGE-4");

/// The free space below which an import is at risk of failing.
///
/// A coarse floor rather than the projection the spec ultimately wants: computing
/// exhaustion from what the download queue holds needs the service client, which
/// is not built yet, so this catches a volume that is nearly full before that
/// arrives. A single large import can be tens of gigabytes, so the floor sits
/// well above one file.
const LOW_SPACE_FLOOR: u64 = 10 * 1024 * 1024 * 1024;

/// The name of the file the probe creates, and the second name it links it to.
///
/// Fixed and unmistakable so that a probe interrupted mid-run leaves something a
/// person recognises as lemonfiber's rather than a mystery file in their media.
const PROBE: &str = ".lemonfiber-hardlink-probe";

/// The second name the probe file is linked to.
const LINKED: &str = ".lemonfiber-hardlink-probe.link";

/// The one consequence sentence a lost link means, stated the same way wherever
/// it is reported — because "hardlinks unsupported" means nothing, and this is
/// what it means.
const CONSEQUENCE: &str = "Imports will copy instead of link. Each takes minutes rather than \
    being instant, uses twice the disk while it runs, and torrents cannot seed from the library \
    copy.";

/// Whether the data root can hardlink, and what mode that puts the stack in.
pub struct StorageCheck {
    filesystem: Arc<dyn FileSystem>,
    root: Option<PathBuf>,
}

impl StorageCheck {
    /// A storage check over the given filesystem and configured data root.
    ///
    /// The root is optional because a machine that has not been set up has not
    /// chosen one yet, and an operator in that state is told to run setup rather
    /// than shown an error about a path they never picked.
    #[must_use]
    pub fn new(filesystem: Arc<dyn FileSystem>, root: Option<PathBuf>) -> Self {
        Self { filesystem, root }
    }

    /// Run the probe against a resolved data root and read what it proved.
    ///
    /// The volume is described first, so its free space is reported alongside
    /// even a root that cannot be written to — a full disk and an unwritable one
    /// are different problems, and the operator is owed both answers at once.
    async fn probe(&self, real: &Path) -> Vec<Finding> {
        let facts = self.filesystem.describe(real).await;
        let space = space(&facts);

        let probe = real.join(PROBE);
        let linked = real.join(LINKED);

        if let Err(fault) = self.filesystem.touch(&probe).await {
            let mut findings = unwritable(&fault.message);
            findings.push(space);
            return findings;
        }

        let original = self.filesystem.identify(&probe).await.ok();
        let link = self.filesystem.link(&probe, &linked).await;

        let mut findings = if link.is_err() {
            copying(&facts)
        } else {
            let confirmed = self.filesystem.identify(&linked).await.ok();
            linked_result(original, confirmed, &facts)
        };

        self.filesystem.remove(&linked).await;
        self.filesystem.remove(&probe).await;
        findings.push(space);
        findings
    }
}

#[async_trait]
impl Check for StorageCheck {
    fn category(&self) -> Category {
        Category::Storage
    }

    async fn run(&self) -> Vec<Finding> {
        let Some(root) = &self.root else {
            return vec![skipped(
                "no data location is configured yet — run setup to choose one",
            )];
        };

        // Resolved first so the probe runs against the filesystem the data
        // actually lives on: a symlinked root would otherwise be tested on the
        // filesystem holding the link, which is not the one that matters.
        match self.filesystem.canonicalize(root).await {
            Err(fault) => absent(root, &fault.message),
            Ok(real) => self.probe(&real).await,
        }
    }
}

/// The findings when the link was made: a pass if the two names are one file,
/// and the mode the working link puts the stack in.
fn linked_result(
    original: Option<Identity>,
    confirmed: Option<Identity>,
    facts: &StorageFacts,
) -> Vec<Finding> {
    match (original, confirmed) {
        (Some(one), Some(two)) if one.file == two.file => {
            let note = format!("{}, {} names to one file", facts.kind.label(), two.links);
            pair(Verdict::Pass { note: Some(note) }, working_mode(facts))
        }
        // The link call succeeded yet the names do not agree on a file, or one
        // could not be read back: the capability is not disproven, but it is not
        // proven either, and an unproven guarantee is never reported as met.
        _ => pair(
            Verdict::Unverified {
                reason: "the link was made but could not be confirmed to point at the same file"
                    .to_owned(),
                remedy: Remedy::new("Run the storage check again"),
            },
            Verdict::Skipped {
                reason: "the link could not be confirmed, so no mode was derived".to_owned(),
            },
        ),
    }
}

/// The findings when the link could not be made: the copy-mode warning, named
/// with the filesystem's specific limitation where there is one.
fn copying(facts: &StorageFacts) -> Vec<Finding> {
    let summary = match facts.kind.limitation() {
        Some(cause) => format!("This location cannot hardlink — {cause}"),
        None => "This location cannot hardlink".to_owned(),
    };
    let problem = Problem::new(
        COPY_ONLY,
        Severity::Warning,
        summary,
        CONSEQUENCE,
        Remedy::new("Choose a location that hardlinks, or continue in copy mode")
            .with_detail("The services are configured to copy so imports still work"),
    )
    .in_state(State::Guided);

    pair(Verdict::Warn(problem), copy_mode(facts))
}

/// The mode a working link puts the stack in: local, or external on removable
/// media, both of which link.
fn working_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.removable {
        "external — hardlinks on removable media"
    } else {
        "local — imports hardlink instantly"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// The mode a failed link puts the stack in: copy, or a network share's copy.
fn copy_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.kind.is_network() {
        "nas — imports copy across a network share"
    } else {
        "copy — imports copy; this location cannot hardlink"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// The free-space finding for the volume the data root sits on.
///
/// A volume that reports no size at all could not be measured rather than being
/// empty, so it is unverified rather than reported as full. This is the raw
/// figure, not the projection from queued content the spec ultimately wants —
/// that needs the download client — so it warns on a floor rather than on an
/// exhaustion date.
fn space(facts: &StorageFacts) -> Finding {
    let verdict = if facts.total == 0 {
        Verdict::Unverified {
            reason: "the volume's free space could not be read".to_owned(),
            remedy: Remedy::new("Run the storage check again once the location is reachable"),
        }
    } else if facts.available < LOW_SPACE_FLOOR {
        Verdict::Warn(
            Problem::new(
                SPACE_LOW,
                Severity::Warning,
                format!("Free space is low — {} left", humanize(facts.available)),
                "A disk that fills partway through an import leaves half a file behind and \
                 stalls the queue, so what is left has to cover what is still to come.",
                Remedy::new("Free space on the data location, or move it to a larger volume"),
            )
            .in_state(State::Guided),
        )
    } else {
        Verdict::Pass {
            note: Some(format!(
                "{} free of {}",
                humanize(facts.available),
                humanize(facts.total)
            )),
        }
    };
    finding("storage.space", "Free space", verdict)
}

/// A byte count as a person reads it, to one decimal place.
///
/// Binary units, because that is what the tools an operator will cross-check
/// against report, and one decimal because a library measured to the byte is
/// noise around a figure whose point is "roughly how much room is left".
fn humanize(bytes: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("TiB", 1 << 40),
        ("GiB", 1 << 30),
        ("MiB", 1 << 20),
        ("KiB", 1 << 10),
    ];
    for (label, size) in UNITS {
        if bytes >= size {
            let whole = bytes / size;
            let tenths = (bytes % size) * 10 / size;
            return format!("{whole}.{tenths} {label}");
        }
    }
    format!("{bytes} B")
}

/// The findings when the data root could not be reached at all.
fn absent(root: &Path, detail: &str) -> Vec<Finding> {
    let problem = Problem::new(
        ROOT_ABSENT,
        Severity::Error,
        format!("The data location {} could not be reached", root.display()),
        "Nothing can be stored where there is no reachable directory, and a stack that \
         wrote into a missing mount point would build a phantom library on the system disk.",
        Remedy::new("Check the location exists and any drive holding it is connected"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned());

    pair(
        Verdict::Fail(problem),
        Verdict::Skipped {
            reason: "the data location could not be reached, so no mode was derived".to_owned(),
        },
    )
}

/// The findings when the data root is present but cannot be written to.
fn unwritable(detail: &str) -> Vec<Finding> {
    let problem = Problem::new(
        ROOT_UNWRITABLE,
        Severity::Error,
        "The data location cannot be written to",
        "The services run as a user that has to own what they import, so a data root they \
         cannot write to fails every import far from where the cause shows.",
        Remedy::new("Give the account that runs the services write access to the data location"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned());

    pair(
        Verdict::Fail(problem),
        Verdict::Skipped {
            reason: "the data location could not be written to, so no mode was derived".to_owned(),
        },
    )
}

/// The two findings a storage run reports: the hardlink capability, and the mode
/// derived from it.
fn pair(hardlinks: Verdict, mode: Verdict) -> Vec<Finding> {
    vec![
        finding("storage.hardlinks", "Hardlinks", hardlinks),
        finding("storage.mode", "Storage mode", mode),
    ]
}

/// A finding in the storage category.
fn finding(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding {
        check: check.to_owned(),
        category: Category::Storage,
        title: title.to_owned(),
        verdict,
    }
}

/// A single finding for a check that does not apply.
fn skipped(reason: &str) -> Finding {
    finding(
        "storage",
        "Storage",
        Verdict::Skipped {
            reason: reason.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        humanize, Check, Finding, StorageCheck, Verdict, COPY_ONLY, ROOT_ABSENT, ROOT_UNWRITABLE,
        SPACE_LOW,
    };
    use crate::ports::filesystem::{Fault, FileSystem, FsKind, Identity, StorageFacts};

    /// Room to spare, so a test that is not about space never trips the floor.
    const AMPLE: u64 = 500 * 1024 * 1024 * 1024;

    /// A one-terabyte volume, the size the ample figure is free space on.
    const CAPACITY: u64 = 1024 * 1024 * 1024 * 1024;

    /// Facts for a filesystem, with a capacity a test does not otherwise care
    /// about filled in generously.
    fn facts(kind: FsKind, removable: bool) -> StorageFacts {
        StorageFacts {
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
    }

    impl Bench {
        /// A healthy local filesystem: resolves, writes, links, and reports the
        /// two names as one file on a filesystem that links, with room to spare.
        fn healthy() -> Self {
            Self {
                resolves: Ok(PathBuf::from("/data")),
                writes: Ok(()),
                original: Ok(Identity { file: 7, links: 1 }),
                links: Ok(()),
                confirmed: Ok(Identity { file: 7, links: 2 }),
                facts: facts(FsKind::Linking("apfs".to_owned()), false),
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
        async fn describe(&self, _path: &Path) -> StorageFacts {
            self.facts.clone()
        }
    }

    async fn run(bench: Bench, root: Option<&str>) -> Vec<Finding> {
        StorageCheck::new(Arc::new(bench), root.map(PathBuf::from))
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

    #[test]
    fn a_byte_count_reads_in_the_unit_a_person_would_use() {
        assert_eq!(humanize(0), "0 B");
        assert_eq!(humanize(512), "512 B");
        assert_eq!(humanize(1536), "1.5 KiB");
        assert_eq!(humanize(3 * 1024 * 1024), "3.0 MiB");
        assert_eq!(
            humanize(10 * 1024 * 1024 * 1024 + 512 * 1024 * 1024),
            "10.5 GiB"
        );
        assert_eq!(humanize(1024_u64.pow(4) * 2), "2.0 TiB");
    }
}

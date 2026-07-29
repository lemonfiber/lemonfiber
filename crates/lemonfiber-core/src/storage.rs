//! Proving a directory can hardlink, in the one place that asks it.
//!
//! The whole media pipeline rests on one property: downloads and the library
//! share a filesystem, so importing a file links it rather than copying it. A
//! filesystem's *type* only hints at whether that works — exFAT never links, a
//! network share usually cannot — so capability is measured, never inferred:
//! create a file, link it, and check the two names point at one underlying file.
//!
//! The [`ports::filesystem`](crate::ports::filesystem) port is deliberately small
//! operations rather than one "test hardlinks" call, because this sequence — the
//! part that decides — belongs above the port where a fake can drive every branch
//! of it without a real disk. Both the storage diagnostic and the setup wizard's
//! data-location step ask the same question, so they ask it through here rather
//! than each keeping a probe of its own to drift.

use std::path::Path;

use crate::ports::filesystem::{FileSystem, Identity};

/// The name of the file the probe creates, and the second name it links it to.
///
/// Fixed and unmistakable so that a probe interrupted mid-run leaves something a
/// person recognises as lemonfiber's rather than a mystery file in their media.
const PROBE: &str = ".lemonfiber-hardlink-probe";

/// The second name the probe file is linked to.
const LINKED: &str = ".lemonfiber-hardlink-probe.link";

/// What creating and inspecting a hardlink under a directory established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Linked {
    /// The link was made and the two names resolve to one file: it hardlinks,
    /// and this many names point at that file now.
    Yes {
        /// How many names point at the linked file — the evidence the link took.
        links: u64,
    },
    /// The link could not be made: the directory cannot hardlink.
    No,
    /// A probe file could not even be created — the directory is not writable —
    /// carrying the platform's own words for why.
    Unwritable {
        /// The operating system's description of why the file could not be made.
        message: String,
    },
    /// The link was made but could not be confirmed to point at one file: not
    /// disproven, but an unproven guarantee is never reported as met.
    Unconfirmed,
}

/// Create a file under `dir`, link it, and inspect whether the two names are one
/// file — the empirical test, never inferred from the filesystem's name.
///
/// `dir` must be a directory that already exists; a caller testing a location not
/// yet created resolves it to a real ancestor first. Both probe names are cleared
/// before and after, so a run interrupted between the link and its cleanup does
/// not read as an inability to link on the next.
pub async fn test_link(filesystem: &dyn FileSystem, dir: &Path) -> Linked {
    let probe = dir.join(PROBE);
    let linked = dir.join(LINKED);

    // A run killed between the link and the cleanup below leaves the link behind,
    // and a link to a name that already exists fails — which would read as "cannot
    // hardlink" on a filesystem that hardlinks fine. Clearing both names first
    // makes the probe robust to its own interrupted past.
    filesystem.remove(&linked).await;
    filesystem.remove(&probe).await;

    if let Err(fault) = filesystem.touch(&probe).await {
        return Linked::Unwritable {
            message: fault.message,
        };
    }

    let original = filesystem.identify(&probe).await.ok();
    let made = filesystem.link(&probe, &linked).await;
    let result = if made.is_err() {
        Linked::No
    } else {
        let confirmed = filesystem.identify(&linked).await.ok();
        if same_file(original, confirmed) {
            Linked::Yes {
                links: confirmed.map_or(0, |identity| identity.links),
            }
        } else {
            Linked::Unconfirmed
        }
    };

    filesystem.remove(&linked).await;
    filesystem.remove(&probe).await;
    result
}

/// Whether two entries name the same underlying file.
///
/// A zero identity is no identity — a real file's number is never zero, and a
/// platform that reports no file index leaves nothing to compare — so it never
/// counts as a match, however equal two zeroes look.
fn same_file(original: Option<Identity>, confirmed: Option<Identity>) -> bool {
    matches!((original, confirmed), (Some(a), Some(b)) if a.file == b.file && a.file != 0)
}

//! This machine's disk, as every world reads and writes it, with what would differ from
//! one machine to the next held still: the filesystem every path is on and who owns it.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_adapters::Disk;
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_fixtures::steadied::Steadied;

/// The filesystem every path is on, as the platform names it.
const FILESYSTEM: &str = "ext4";

/// How many bytes that filesystem holds.
const TOTAL: u64 = 4 << 40;

/// How many of them are free.
const AVAILABLE: u64 = 3 << 40;

/// The user and the group every file is owned by.
const OWNER: u32 = 1000;

/// The disk, every file owned by [`OWNER`] and every path on one roomy local filesystem.
pub(crate) fn kept() -> Steadied {
    let facts = StorageFacts {
        point: PathBuf::from("/"),
        kind: FsKind::Linking(FILESYSTEM.to_owned()),
        removable: false,
        available: AVAILABLE,
        total: TOTAL,
    };
    Steadied::over(Arc::new(Disk), OWNER, facts)
}

//! What this machine says about the copy of lemonfiber that is running.
//!
//! Three reads and a probe, each of which only a real machine can answer, kept apart
//! from the deciding they feed so that every answer they can produce is reachable from
//! one laptop. What is decided from them is [`crate::update::Installed`]'s, which sees
//! the facts and never the filesystem.
//!
//! The probe writes beside the binary rather than on it. Replacing a file means
//! putting a new one in the same directory and moving it into place, so the directory
//! is what has to be writable — and asking that way means the running binary is never
//! touched by the asking.

use std::path::{Path, PathBuf};

use crate::ports::FileSystem;
use crate::update::{cargo_record_above, probe_beside, receipt_under, Installed, Signs};

/// Where the running binary is, with any link followed.
///
/// A link followed is what makes a package manager tellable at all: Homebrew installs
/// into a cellar and links the name into its own `bin`, so the unresolved path says
/// only that something is on the path. A machine that will not resolve it falls back
/// to the path as given, which still answers the question an operator asks of it.
pub(super) async fn at(files: &dyn FileSystem, program: Option<&PathBuf>) -> Option<PathBuf> {
    let program = program?;
    Some(
        files
            .canonicalize(program)
            .await
            .unwrap_or_else(|_| program.clone()),
    )
}

/// Which tool owns this copy, from the records the tools leave.
///
/// Both records are read as presence rather than as content, with one exception:
/// cargo keeps one record for everything it has installed, so its file being there
/// says nothing until it is asked whether this program is named in it.
pub(super) async fn installed(
    files: &dyn FileSystem,
    at: Option<&Path>,
    home: Option<&PathBuf>,
) -> Installed {
    let receipt = match home {
        Some(home) => files.read(&receipt_under(home)).await.is_some(),
        None => false,
    };
    let recorded_by_cargo = match at.and_then(cargo_record_above) {
        Some(record) => files
            .read(&record)
            .await
            .is_some_and(|text| text.contains(crate::PRODUCT)),
        None => false,
    };
    Installed::read(&Signs {
        at,
        receipt,
        recorded_by_cargo,
    })
}

/// Whether the directory holding the running binary would take a new file.
///
/// Asked by trying, because permission bits are not the whole answer: an immutable
/// flag, a read-only mount and a container's own layer each refuse a write that the
/// bits say is allowed. Whatever it comes to, the probe is taken away again — a run
/// interrupted between the two leaves a file whose name says what it was for.
pub(super) async fn replaceable(files: &dyn FileSystem, at: Option<&Path>) -> Option<bool> {
    let probe = at.and_then(probe_beside)?;
    let written = files.touch(&probe).await.is_ok();
    files.remove(&probe).await;
    Some(written)
}

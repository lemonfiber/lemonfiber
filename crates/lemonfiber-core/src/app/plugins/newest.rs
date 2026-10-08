//! The newest catalogue index this machine has verified, and the refusal of any older.
//!
//! A release the catalogue replaced is still signed by the key that signed it, so a
//! signature alone cannot tell it from the release that replaced it. Each index names
//! the release it is, and an index older than the newest one this machine has verified
//! is refused before a name is resolved through it.
//!
//! **What cannot be read is not read as nothing.** A record of the newest index that
//! is there and cannot be read, and a machine with nowhere to keep one, refuse every
//! install by name: read as none, either would let a replaced release through.
//!
//! **Only a run that acts remembers.** A reading and a rehearsal write nothing, so they
//! compare against what is remembered and leave it as it was.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::CATALOGUE;
use crate::error::codes::plugin::{CATALOGUE_REPLACED, NEWEST_UNKEPT};
use crate::error::{Problem, Remedy, State};

/// What is remembered about the catalogue.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Newest {
    /// The newest index this machine verified, by the release it names.
    serial: u64,
}

/// Hold a verified index to the newest one this machine verified, and remember it
/// where this run acts.
///
/// # Errors
///
/// Where the index is older than the newest one verified, and where the record of that
/// cannot be read, has nowhere to be kept, or cannot be written.
pub(super) fn held(ctx: &Ctx, serial: u64, acting: bool) -> Result<(), Box<Problem>> {
    let at = super::super::targets::beside_env(ctx, CATALOGUE)
        .ok_or_else(|| Box::new(unkept("this machine has no configuration to keep it in")))?;
    let newest = remembered(&at)?;
    if serial < newest {
        return Err(Box::new(replaced(serial, newest)));
    }
    if acting && serial > newest {
        super::super::record::keep(Some(&at), &Newest { serial })
            .map_err(|why| Box::new(unkept(&why.summary)))?;
    }
    Ok(())
}

/// The newest serial remembered at `at`, nought where nothing has been verified yet.
fn remembered(at: &Path) -> Result<u64, Box<Problem>> {
    match std::fs::read_to_string(at) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(why) => Err(Box::new(unreadable(at, &why.to_string()))),
        Ok(text) => serde_json::from_str::<Newest>(&text)
            .map(|newest| newest.serial)
            .map_err(|why| Box::new(unreadable(at, &why.to_string()))),
    }
}

/// Said where the index is older than the newest one this machine verified.
fn replaced(serial: u64, newest: u64) -> Problem {
    Problem::new(
        CATALOGUE_REPLACED,
        "The catalogue's index is older than one this machine has already verified",
        format!(
            "Nothing was resolved through it and nothing was installed. It is release {serial}, \
             and this machine has verified release {newest}: a release the catalogue has \
             replaced is still signed, and what it reviewed may since have been withdrawn."
        ),
        Remedy::new(
            "Try again later, when the catalogue serves its newest release, or install it from \
             its git source",
        ),
    )
    .in_state(State::Guided)
}

/// Said where the record of the newest index cannot be read.
fn unreadable(at: &Path, why: &str) -> Problem {
    unkept(&format!("{} could not be read: {why}", at.display()))
}

/// Said where the newest index verified cannot be known or kept.
fn unkept(why: &str) -> Problem {
    Problem::new(
        NEWEST_UNKEPT,
        "This machine cannot tell whether the catalogue's index is its newest",
        "Nothing was resolved through it and nothing was installed. Without the record of the \
         newest index this machine verified, a release the catalogue has replaced would verify \
         as though it were current.",
        Remedy::new(format!(
            "Check {CATALOGUE} in the configuration directory, or install it from its git source"
        )),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

//! The one file that is nothing like the others.
//!
//! A ninety-gigabyte remux in a library of four-gigabyte encodes is almost never a
//! decision somebody made. It is a quality profile that let one release through, or
//! a single grab nobody looked at, and it costs as much as twenty ordinary ones. It
//! is also invisible in every figure that sums a tree, which is exactly why it goes
//! unnoticed until the disk is full.
//!
//! Large is a comparison rather than a number. A library of remuxes is not
//! misconfigured because its files are large, so the test is against what the rest
//! of this operator's own files look like — several times the middle one — with an
//! absolute floor beneath it so that a small collection of small files does not
//! report its largest as a problem.

use serde::Serialize;

use crate::ports::occupancy::Occupant;

/// How many times the middle file a file must be before it is worth pointing at.
///
/// High enough that the ordinary spread of a library — a long film against a short
/// one — never trips it, and low enough to catch the case this exists for, where
/// one release is an order of magnitude out.
const TIMES_TYPICAL: u64 = 8;

/// The size below which nothing is worth pointing at, however far out it is.
///
/// Without it, a tidy collection of small files reports its largest as an anomaly
/// every run, which teaches the operator that this line means nothing.
pub(crate) const FLOOR: u64 = 20 * 1024 * 1024 * 1024;

/// How many are named, at most.
///
/// A handful is a highlight; a hundred is a directory listing, which is the thing
/// this exists instead of.
pub(crate) const MOST: usize = 5;

/// One file far larger than the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Outsized {
    /// Where it is.
    pub path: String,
    /// What it occupies.
    pub bytes: u64,
    /// How many times the middle file of this walk it is.
    pub times_typical: u64,
}

/// The files far enough out of line with the rest to be worth pointing at.
///
/// `typical` is the size of the walk's middle file and `largest` its largest files
/// at or above [`FLOOR`], largest first, which is all of the walk this reads — see
/// [`crate::space::survey::Survey`]. Largest first, and at most a handful of them.
#[must_use]
pub fn outsized(typical: Option<u64>, largest: &[Occupant]) -> Vec<Outsized> {
    let Some(typical) = typical else {
        return Vec::new();
    };
    largest
        .iter()
        .filter(|occupant| occupant.bytes >= FLOOR)
        .filter(|occupant| occupant.bytes / typical >= TIMES_TYPICAL)
        .take(MOST)
        .map(|occupant| Outsized {
            path: occupant.path.display().to_string(),
            bytes: occupant.bytes,
            times_typical: occupant.bytes / typical,
        })
        .collect()
}

#[cfg(test)]
mod tests;

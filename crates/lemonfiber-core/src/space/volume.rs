//! One volume, measured — and what the measurement is worth.
//!
//! Two of them are watched rather than one. The media lives under the data root
//! and the services' own databases live beside lemonfiber's configuration, and on
//! many machines those are different drives: a stack whose data volume has room to
//! spare still stops dead when the volume holding the databases fills, and the
//! operator looking at a healthy free-space figure has no idea why.
//!
//! A reading over the network is not the same kind of fact as a reading off a
//! local disk. A share reports what its server last told the client, and a client
//! that has been holding that figure for a minute will report it as confidently as
//! one that read it just now. Nothing here can make that figure fresher, so what
//! it does instead is say when it was taken and let the operator weigh it — a
//! stale number presented as live is worse than the same number presented as what
//! it is.

use serde::Serialize;

use super::level::Level;
use crate::ports::filesystem::StorageFacts;

/// Which of the two volumes a reading is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Where the media and the downloads live.
    Data,
    /// Where the services keep their own configuration and databases.
    Services,
}

impl Role {
    /// What this volume is for, as a person reads it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Data => "the data location",
            Self::Services => "the services' own files",
        }
    }

    /// What its filling costs, which differs enough to be worth saying apart.
    #[must_use]
    pub const fn costs(self) -> &'static str {
        match self {
            Self::Data => "downloads stall part-way and imports leave half a file behind",
            Self::Services => {
                "the services cannot write their databases, which is how one \
                               is corrupted rather than merely stopped"
            }
        }
    }
}

/// How much a reading can be relied on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "as", content = "at")]
pub enum Freshness {
    /// Read off a local disk, so it is true as of now.
    Live,
    /// Read across a network share, which answers with what it was last told —
    /// carrying the moment it was taken, in seconds since the epoch, so a figure
    /// nobody can refresh is at least dated.
    AsOf(u64),
}

impl Freshness {
    /// How a reading of this kind is taken from a volume of this type.
    #[must_use]
    pub const fn of(network: bool, taken: u64) -> Self {
        if network {
            Self::AsOf(taken)
        } else {
            Self::Live
        }
    }

    /// Whether the figure may already have moved on without anybody being told.
    #[must_use]
    pub const fn goes_stale(self) -> bool {
        matches!(self, Self::AsOf(_))
    }
}

/// One volume, as one run measured it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Volume {
    /// Which of the two this is.
    pub role: Role,
    /// The path that was measured.
    pub at: String,
    /// Where the volume holding it is mounted, which is what the limit belongs to.
    pub point: String,
    /// Bytes free, or nothing where the volume could not be read.
    pub free: Option<u64>,
    /// The effective limit — the mount's own size, which on a dataset given a
    /// quota is the quota rather than the device beneath it.
    pub limit: Option<u64>,
    /// The bytes already committed to landing here.
    pub committed: u64,
    /// What would be free once the committed content has landed.
    pub projected: Option<u64>,
    /// Where it stands.
    pub level: Level,
    /// What the reading is worth.
    pub reading: Freshness,
}

impl Volume {
    /// One volume, measured from what the platform reports about the path.
    ///
    /// A volume that could not be attributed to any mount reports a total of zero,
    /// and its free space is then unknown rather than zero: "the volume could not
    /// be read" and "the disk is full" are opposite things to an operator, and a
    /// report that renders them alike sends somebody to delete files off a drive
    /// that is merely unplugged.
    #[must_use]
    pub fn measured(
        role: Role,
        at: &std::path::Path,
        facts: &StorageFacts,
        committed: u64,
        taken: u64,
    ) -> Self {
        let measured = facts.total > 0;
        let free = measured.then_some(facts.available);
        let limit = measured.then_some(facts.total);
        Self {
            role,
            at: at.display().to_string(),
            point: facts.point.display().to_string(),
            free,
            limit,
            committed,
            projected: free.map(|free| free.saturating_sub(committed)),
            level: Level::reached(free, limit, committed),
            reading: Freshness::of(facts.kind.is_network(), taken),
        }
    }

    /// Whether this volume and another are one and the same.
    ///
    /// Answered by the mount rather than by the paths, since the data root and the
    /// service configuration are different directories however many volumes they
    /// are spread over. Two paths under no reported mount are not evidence of one
    /// volume, however equal two empty strings look.
    #[must_use]
    pub fn shares_with(&self, other: &Self) -> bool {
        !self.point.is_empty() && self.point == other.point
    }
}

#[cfg(test)]
mod tests;

//! Projecting the disk a chosen quality would want, so an implausible one is caught.
//!
//! A quality preset is an appetite: 4K HDR eats disk many times faster than a
//! space-saving encode. An operator can pick the highest preset on a disk that
//! could never hold a library at that quality, and only discover it once the disk
//! fills partway through. This projects the room the chosen quality wants against
//! the free space there is, and warns before the choice bites — nothing already on
//! disk is touched, and a lighter preset or a larger volume resolves it.
//!
//! The projection is forward-looking, as a preset change is: it does not assume the
//! existing library is re-acquired, only that new content arrives at the chosen
//! rate. It reads "how many hours the free space holds at this preset" against a
//! plausible-library floor — a heuristic, stated as such, not a promise about any
//! particular library.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use super::storage::LOW_SPACE_FLOOR;
use super::{Category, Check, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::ports::filesystem::FileSystem;
use crate::quality::Preset;

pub(crate) use crate::error::codes::qual::HEADROOM_LOW;

/// The hours of content a projection takes as a starter-library floor — roughly a
/// dozen films and a couple of seasons. A heuristic, deliberately modest: the check
/// warns only when the disk cannot hold even this much at the chosen quality, so it
/// speaks to a disk that is genuinely small for the choice rather than to one that
/// would simply hold a smaller library than a larger disk would.
const STARTER_LIBRARY_HOURS: u64 = 30;

/// Projects the disk a chosen preset would want against the free space there is.
pub struct HeadroomCheck {
    filesystem: Arc<dyn FileSystem>,
    data_root: Option<PathBuf>,
    preset: Preset,
}

impl HeadroomCheck {
    /// A check that projects `preset`'s appetite against the free space at
    /// `data_root`, where one is configured.
    #[must_use]
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        data_root: Option<PathBuf>,
        preset: Preset,
    ) -> Self {
        Self {
            filesystem,
            data_root,
            preset,
        }
    }

    /// Read the free space and project the chosen quality against it.
    async fn project(&self, root: &Path) -> Verdict {
        let facts = self.filesystem.describe(root).await;
        // A volume that reports no size at all could not be measured, rather than
        // being full — so the room is unknown, never falsely reported as ample.
        if facts.total == 0 {
            return Verdict::Unverified {
                reason: "the data location's free space could not be read, so the room for the \
                         chosen quality is unknown"
                    .to_owned(),
                remedy: Remedy::new("Check the data location is reachable, then check again"),
            };
        }
        // A disk this low is a free-space problem the space check already reports; this
        // check speaks only to a disk with room in absolute terms that is nonetheless
        // small for the chosen quality, so it defers rather than warning twice.
        if facts.available < LOW_SPACE_FLOOR {
            return Verdict::Skipped {
                reason: "the free-space check already reports a disk this low".to_owned(),
            };
        }
        // The rate is a fixed non-zero figure per preset, so this cannot divide by
        // zero.
        let hours = facts.available / self.preset.bytes_per_hour();
        if hours >= STARTER_LIBRARY_HOURS {
            Verdict::Pass {
                note: Some(format!(
                    "the free space holds about {hours} hours of content at the {} preset",
                    self.preset.label()
                )),
            }
        } else {
            Verdict::Warn(low(self.preset, hours))
        }
    }
}

#[async_trait]
impl Check for HeadroomCheck {
    fn category(&self) -> Category {
        Category::Storage
    }

    /// Longer than a check bounded by a container command. This one waits on the
    /// operator's own storage, which may be a network share or a drive that has
    /// spun down, and calling merely slow hardware unreadable sends them to
    /// diagnose a disk that is working.
    fn budget(&self) -> std::time::Duration {
        super::FILESYSTEM_BUDGET
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Whether the free space holds enough at the quality in force.
async fn ran(check: &HeadroomCheck) -> Vec<Finding> {
    let verdict = match &check.data_root {
        None => Verdict::Skipped {
            reason: "no data location is configured, so there is nothing to project against"
                .to_owned(),
        },
        Some(root) => check.project(root).await,
    };
    vec![Finding::in_category(
        Category::Storage,
        "storage.quality-headroom",
        "Room for the chosen quality",
        verdict,
    )]
}

/// The free space holds too little at the chosen quality: a risk to warn on, not a
/// fault — nothing is broken, and the existing library is untouched. The `preset` is
/// the most demanding one in force, which may be a single media type's exception, so
/// the wording is about content kept at that preset rather than the whole library.
fn low(preset: Preset, hours: u64) -> Problem {
    Problem::new(
        HEADROOM_LOW,
        Severity::Warning,
        format!("The disk is small for {} quality", preset.label()),
        format!(
            "Content kept at the {} preset takes {}, and the free space here holds only about \
             {hours} hours of it — thin for a library at that quality. Nothing is broken and \
             nothing already downloaded is affected; new acquisitions at this preset will simply \
             fill the disk quickly.",
            preset.label(),
            preset.consequence().size_per_hour,
        ),
        Remedy::new(
            "Free space on the data location, move it to a larger volume, or choose a lighter \
             quality preset for the media that does not need it",
        ),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;

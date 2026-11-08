//! The record of the queue, kept beside the configuration: each unhealthy item's strikes,
//! what the latest pass found, and the newest runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::HEALING as RECORD;
use crate::config::Healing;
use crate::error::Problem;
use crate::model::{HealRun, HealingSwitch, QueueHealth, Unreached, Wedged};
use crate::queue::healing::Struck;

use super::gathering::Gathered;
use super::judging::Judged;

/// How many runs are kept, the newest first.
pub(crate) const RUNS_KEPT: usize = 20;

/// What the latest pass found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Latest {
    /// When, in seconds since the Unix epoch.
    pub(crate) at: u64,
    /// Every unhealthy item.
    #[serde(default)]
    pub(crate) items: Vec<Wedged>,
    /// Every curator or client it could not reach or verify.
    #[serde(default)]
    pub(crate) unreachable: Vec<Unreached>,
    /// Where a client files a curator's downloads outside the curator's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mismatch: Option<String>,
    /// Whether the list may be short.
    #[serde(default)]
    pub(crate) incomplete: bool,
}

/// The record of the queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each unhealthy item's strikes, by what they are kept under.
    #[serde(default)]
    pub(crate) strikes: BTreeMap<String, Struck>,
    /// What the latest pass found, where one has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) latest: Option<Latest>,
    /// The newest runs first.
    #[serde(default)]
    pub(crate) runs: Vec<HealRun>,
}

impl Kept {
    /// Keep what a pass at `now` found, and the strikes it left where it was not a
    /// rehearsal; an item acted on starts again from none.
    pub(super) fn passed(
        &mut self,
        judged: &Judged,
        gathered: &Gathered,
        healing: &Healing,
        now: u64,
        acted: &[String],
        rehearsed: bool,
    ) {
        if !rehearsed {
            self.strikes = judged
                .strikes
                .iter()
                .filter(|(key, _)| !acted.contains(key))
                .map(|(key, struck)| (key.clone(), *struck))
                .collect();
        }
        self.latest = Some(Latest {
            at: now,
            items: judged.wedged(gathered, healing, now),
            unreachable: gathered.unreachable.clone(),
            mismatch: gathered.mismatch(),
            incomplete: gathered.incomplete,
        });
    }

    /// Keep `run` as the newest, letting the oldest go past [`RUNS_KEPT`].
    pub(crate) fn ran(&mut self, run: HealRun) {
        self.runs.insert(0, run);
        self.runs.truncate(RUNS_KEPT);
    }

    /// The queue as this record and `healing` say it.
    pub(crate) fn read(self, healing: &Healing) -> QueueHealth {
        let latest = self.latest.unwrap_or_default();
        let classified = latest.at;
        QueueHealth {
            healing: if healing.on {
                HealingSwitch::On
            } else {
                HealingSwitch::Off
            },
            strikes_needed: healing.strikes,
            grace_s: healing.grace.as_secs(),
            items: latest.items,
            unreachable: latest.unreachable,
            mismatch: latest.mismatch,
            incomplete: latest.incomplete,
            runs: self.runs,
            classified_at: (classified > 0).then_some(classified),
            rehearsed: false,
        }
    }
}

/// The record as kept, the default where there is none or it cannot be read.
pub(crate) fn held(ctx: &Ctx) -> Kept {
    crate::app::record::beside(ctx, RECORD)
}

/// The record as kept, the default where there is none.
///
/// # Errors
///
/// Where the record is there and cannot be read.
pub(crate) fn read(ctx: &Ctx) -> Result<Kept, Box<Problem>> {
    crate::app::record::read(crate::app::targets::beside_env(ctx, RECORD).as_deref())
}

/// Write the record back.
pub(crate) fn keep(ctx: &Ctx, kept: &Kept) {
    crate::app::record::keep_beside(ctx, RECORD, kept);
}

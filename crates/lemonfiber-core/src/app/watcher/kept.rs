//! The record of the monitors lemonfiber provisioned, kept beside the configuration: the
//! id the watcher answered for each tag, what it was asked, and what it watches instead
//! where it could not watch that.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::WATCHER as RECORD;
use crate::error::Problem;
use crate::model::{Unconfigured, WatchPath};
use crate::ports::service::{Provision, Provisioned};

use super::covering::Wanted;

/// Every monitor lemonfiber provisioned, by its tag.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each monitor, by the tag it was provisioned under.
    #[serde(default)]
    pub(crate) made: BTreeMap<String, Made>,
}

/// One monitor lemonfiber provisioned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Made {
    /// The watcher's id for it, which pairs it with what it covers.
    pub(crate) id: String,
    /// What it was provisioned as.
    pub(crate) asked: Provision,
    /// Which path an entry point's monitor watches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<WatchPath>,
    /// What it was asked to watch and what it watches instead, where it does not watch
    /// what it should.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) unconfigured: Option<Unconfigured>,
}

impl Made {
    /// The monitor a provision of `wanted` made, as the watcher answered it.
    ///
    /// Where the watcher watches something other than what it was asked to, that is what
    /// it watches and why, beside what the core itself could not ask for.
    pub(crate) fn provisioned(wanted: &Wanted, provisioned: Provisioned) -> Self {
        let unconfigured = match (provisioned.instead, &wanted.short) {
            (None, short) => short.clone(),
            (Some(instead), short) => Some(Unconfigured {
                wanted: short.as_ref().map_or_else(
                    || wanted.provision.target.clone(),
                    |short| short.wanted.clone(),
                ),
                watches: instead.watches,
                why: instead.why,
            }),
        };
        Self {
            id: provisioned.id,
            asked: wanted.provision.clone(),
            path: wanted.path,
            unconfigured,
        }
    }
}

impl Kept {
    /// The tag of the monitor the watcher holds as `id`, where lemonfiber made it.
    pub(crate) fn tagged(&self, id: &str) -> Option<(&str, &Made)> {
        self.made
            .iter()
            .find(|(_, made)| made.id == id)
            .map(|(tag, made)| (tag.as_str(), made))
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
/// Where the record is there and cannot be read, which is never answered as nothing
/// provisioned.
pub(crate) fn read(ctx: &Ctx) -> Result<Kept, Box<Problem>> {
    crate::app::record::read(crate::app::targets::beside_env(ctx, RECORD).as_deref())
}

/// Write the record back; nothing on a rehearsal.
pub(crate) fn keep(ctx: &Ctx, kept: &Kept) {
    if ctx.dry_run {
        return;
    }
    crate::app::record::keep_beside(ctx, RECORD, kept);
}

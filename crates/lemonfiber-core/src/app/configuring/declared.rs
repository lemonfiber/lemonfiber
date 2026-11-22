//! The areas the operator declared unmanaged, as the `config` read lists them.
//!
//! The setting carries the line as it was written, and a line says what was written
//! rather than what it stops: a misspelt area covers nothing while the operator believes
//! it does. So each area is listed with what it covers now, worked out by
//! [`crate::unmanaged`]'s one rule over everything here a declaration can stop.

use std::collections::HashSet;

use super::Ctx;
use crate::config::env::EnvFile;
use crate::model::{CoveredReport, UnmanagedReport};
use crate::plugin::Installed;
use crate::text::shown::{bounded, shown};
use crate::unmanaged::CoveredKind;

/// The most areas the read lists; how many it leaves out is counted beside them. The
/// setting is one line written by hand, and a client draws each area as a row of its own.
const MOST_DECLARED: usize = 64;

/// The most characters, as written, an area or a reason holds: one past it is cut and
/// says so. A reason is a sentence somebody reads back, not a document.
const LONGEST: usize = 512;

/// The most things one area lists as covered; how many it leaves out is counted beside
/// them. A broad area such as `config` covers every file beneath it.
const MOST_COVERED: usize = 64;

/// The areas declared in `file`, as listed, and how many are left out.
pub(super) struct Declared {
    /// The areas listed, in the order written.
    pub(super) listed: Vec<UnmanagedReport>,
    /// How many are left out.
    pub(super) more: usize,
}

/// The areas `file` declares unmanaged, each with what it covers among `things`.
pub(super) fn declared(file: &EnvFile, things: &[(CoveredKind, String)]) -> Declared {
    let all = crate::config::unmanaged_from_env(file);
    Declared {
        more: all.len().saturating_sub(MOST_DECLARED),
        listed: all
            .iter()
            .take(MOST_DECLARED)
            .map(|(area, why)| reported(area, why, things))
            .collect(),
    }
}

/// One area and its reason, bounded, with what it covers among `things`.
fn reported(area: &str, why: &str, things: &[(CoveredKind, String)]) -> UnmanagedReport {
    let covered: Vec<&(CoveredKind, String)> = crate::unmanaged::covered(area, things).collect();
    let (area_shown, why_shown) = (
        bounded(&shown(area), LONGEST),
        bounded(&shown(why), LONGEST),
    );
    UnmanagedReport {
        area: area_shown.text,
        area_cut: area_shown.cut,
        why: why_shown.text,
        why_cut: why_shown.cut,
        covers_more: covered.len().saturating_sub(MOST_COVERED),
        covers: covered
            .into_iter()
            .take(MOST_COVERED)
            .map(|(kind, name)| CoveredReport {
                kind: *kind,
                name: name.clone(),
            })
            .collect(),
    }
}

/// Everything here a declaration can stop, each as what it is and named as the rule
/// matches it: the services the stack runs, its manifest's and those `installed` plugins
/// brought; the files lemonfiber writes, a carried stack's by its place beneath the stack
/// and the project's own by name; and the settings `file` holds or a decision names.
pub(super) fn things(
    ctx: &Ctx,
    file: &EnvFile,
    installed: Option<&[Installed]>,
) -> Vec<(CoveredKind, String)> {
    let services = ctx
        .stack
        .checked_manifest(ctx.today())
        .map(|manifest| manifest.services.into_iter().map(|service| service.id))
        .into_iter()
        .flatten()
        .chain(
            installed
                .unwrap_or_default()
                .iter()
                .flat_map(|one| one.services.iter().map(|placed| placed.service.clone())),
        )
        .map(|id| (CoveredKind::Service, id));
    let paths = ctx
        .stack
        .files()
        .into_iter()
        .map(|(relative, _)| relative.to_string_lossy().into_owned())
        .chain(
            crate::app::project::WRITTEN
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .map(|path| (CoveredKind::Path, path));
    let settings = crate::config::store::shown(file)
        .into_iter()
        .map(|setting| setting.key)
        .chain(
            crate::reconfigure::DECISIONS
                .iter()
                .map(|decision| decision.key.as_str().to_owned()),
        )
        .map(|key| (CoveredKind::Setting, key));
    let mut seen = HashSet::new();
    services
        .chain(paths)
        .chain(settings)
        .filter(|thing| seen.insert(thing.clone()))
        .collect()
}

#[cfg(test)]
mod tests;

//! What the catalogue offers to install, read from its verified index as the operator
//! browses it.
//!
//! **Asked only where reaching it is allowed.** Switched off, nothing is fetched, and
//! the answer names the setting that holds it off and the value that lets it be asked.
//!
//! **What was not read is said, never listed as nothing.** An index that cannot be
//! fetched, verified or read answers `failed` with the code an install by name would be
//! refused with, and no entry from any earlier reading.
//!
//! **A reading remembers nothing.** The index is held to the newest one this machine
//! verified, and the newest is left as it was.

use crate::app::Ctx;
use crate::config::REACH_CATALOGUE_KEY;
use crate::error::Problem;
use crate::migration::version::{among_versions, Standing};
use crate::model::{
    CatalogueFailure, CataloguedPlugin, InstalledVersion, PluginCatalogueReport,
    PluginCatalogueState, ReachSetting, SettingWrite,
};
use crate::plugin::catalogue::{Entry, Index};
use crate::plugin::Installed;

/// What the catalogue offers, or why it was not read.
///
/// # Errors
///
/// Where the record of what is installed cannot be read. Every way the catalogue
/// itself is not read is an answer.
pub(crate) async fn offered(ctx: &Ctx) -> Result<PluginCatalogueReport, Box<Problem>> {
    if let Some((key, value)) = ctx.settings.reaching.held_by(REACH_CATALOGUE_KEY) {
        return Ok(unread(
            PluginCatalogueState::Off,
            Some(ReachSetting {
                key: key.to_owned(),
                value: value.to_owned(),
                action: SettingWrite::ConfigSet,
            }),
            None,
        ));
    }
    let held = super::read(ctx)?;
    Ok(match super::cataloguing::current(ctx, false).await {
        Ok(index) => listed(&index, held.installed()).unwrap_or_else(|| {
            failed(&super::cataloguing::unreadable(
                "it names no date it was published",
            ))
        }),
        Err(problem) => failed(&problem),
    })
}

/// The entries of a verified index, each with what is installed of it, or nothing where
/// the index names no date it was published.
fn listed(index: &Index, installed: &[Installed]) -> Option<PluginCatalogueReport> {
    let published = index.published()?;
    let entries: Vec<CataloguedPlugin> = index
        .plugins()
        .iter()
        .filter_map(|entry| catalogued(entry, installed))
        .collect();
    Some(PluginCatalogueReport {
        state: PluginCatalogueState::Listed,
        published: Some(published),
        serial: Some(index.serial()),
        unreadable: Some(index.plugins().len().saturating_sub(entries.len())),
        entries,
        setting: None,
        failure: None,
    })
}

/// One entry as it is listed, where it carries every fact an entry is listed with.
fn catalogued(entry: &Entry, installed: &[Installed]) -> Option<CataloguedPlugin> {
    let version = entry.version.clone()?;
    Some(CataloguedPlugin {
        id: entry.id.clone(),
        name: entry.name.clone()?,
        description: entry.description.clone()?,
        without_it: entry.without_it.clone()?,
        installed: installed
            .iter()
            .find(|one| one.plugin == entry.id)
            .map(|one| InstalledVersion {
                newer: among_versions(&version, &one.version) == Standing::Later,
                version: one.version.clone(),
            }),
        version,
    })
}

/// The answer where the index was not read.
fn failed(problem: &Problem) -> PluginCatalogueReport {
    unread(
        PluginCatalogueState::Failed,
        None,
        Some(CatalogueFailure {
            code: problem.code.as_str().to_owned(),
            summary: problem.summary.clone(),
        }),
    )
}

/// An answer listing nothing, for a catalogue that was not read.
const fn unread(
    state: PluginCatalogueState,
    setting: Option<ReachSetting>,
    failure: Option<CatalogueFailure>,
) -> PluginCatalogueReport {
    PluginCatalogueReport {
        state,
        published: None,
        serial: None,
        entries: Vec::new(),
        unreadable: None,
        setting,
        failure,
    }
}

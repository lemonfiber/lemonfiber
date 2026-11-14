//! Whether a plugin's adapters answer the contracts they speak, as the core reports it:
//! [`lemonfiber_conformance`]'s verdict on every case, beside the refusals the manifest
//! draws, which no recording can make up for.

use std::path::Path;

use lemonfiber_conformance::Speaker;
pub use lemonfiber_conformance::{Judged, Verdict as CaseVerdict};
pub(crate) use lemonfiber_conformance::{ASKED_WITH, RECORDINGS};
use lemonfiber_plugin::{Manifest, Violation};

use super::claimed::{sourced, Unreadable};
use crate::doctor::BUNDLED_CHECKS;

/// What a plugin's recordings say about the contracts its adapters speak.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Conformed {
    /// The plugin's id.
    pub id: String,
    /// Everything the manifest is refused for, which no recording can make up for.
    pub refusals: Vec<Violation>,
    /// Every case of every contract an adapter speaks, as its recording answered it.
    pub cases: Vec<Judged>,
    /// Whether nothing is refused and every case passed.
    pub conforms: bool,
}

impl Conformed {
    /// Every case that did not pass.
    pub fn not_passed(&self) -> impl Iterator<Item = &Judged> {
        self.cases
            .iter()
            .filter(|one| one.verdict != CaseVerdict::Passed)
    }
}

/// What the plugin whose source is at this path conforms to, judged from its recordings.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, or this build cannot read it.
pub async fn conformed(path: &Path) -> Result<Conformed, Unreadable> {
    conformed_to(path, &lemonfiber_contract::capabilities::all()).await
}

/// What the plugin whose source is at this path conforms to of the contracts `published`.
async fn conformed_to(
    path: &Path,
    published: &[lemonfiber_contract::Capability],
) -> Result<Conformed, Unreadable> {
    let (root, manifest) = sourced(path)?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    let cases = lemonfiber_conformance::judged(&root, &speakers(&manifest), published).await;
    let conforms =
        refusals.is_empty() && cases.iter().all(|one| one.verdict == CaseVerdict::Passed);
    Ok(Conformed {
        id: manifest.plugin.id.clone(),
        refusals,
        cases,
        conforms,
    })
}

/// Each service of `manifest` that speaks a contract, as the judge asks of it: what it
/// speaks, and what the upstream it answers for is pinned at and files.
pub(crate) fn speakers(manifest: &Manifest) -> Vec<Speaker> {
    manifest
        .services
        .iter()
        .filter(|service| !service.speaks.is_empty())
        .map(|service| {
            let upstream = manifest.upstream_of(service);
            Speaker {
                service: service.id.clone(),
                speaks: service.speaks.clone(),
                pinned: upstream.map(|one| format!("{}@{}", one.image, one.digest)),
                files: upstream
                    .map(|one| one.media_types.clone())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;

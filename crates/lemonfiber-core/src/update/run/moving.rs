//! The moves in place a confirmed update takes: each part the bundle installs that the
//! stack still runs as its own service, moved through the plugin install path with the
//! confirmed update as its yes.
//!
//! The update's yes answers each move's own offer and nothing beside it. A move whose
//! install would carry a value somewhere else or take a privileged shape is refused by
//! the install for want of that approval, and stays on offer with the reason said.

use lemonfiber_manifest::Manifest;
use schemars::JsonSchema;
use serde::Serialize;

use crate::app::plugins::{Asked, Consent, MoveOffered};
use crate::app::Ctx;
use crate::plugin::Source;

/// A move in place this run took, or tried to take and could not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(rename = "StackMoveTaken")]
pub struct Moved {
    /// The plugin the bundle installs.
    pub plugin: String,
    /// The stack's own service it took, or would have taken, the place of.
    pub service: String,
    /// Whether the plugin now runs in the service's place.
    pub taken: bool,
    /// Why it was not taken, where it was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// One move on offer, with the name its install's reading goes by or why it could not be
/// read.
#[derive(Debug, Clone)]
pub(crate) struct Read {
    /// The move.
    pub(crate) offered: MoveOffered,
    /// The name of its install's offer, or why there is none.
    pub(crate) agreement: Result<String, String>,
}

/// Every move on offer against `manifest`, narrowed to `service` where one is named, each
/// with its install read.
pub(super) async fn read(ctx: &Ctx, manifest: &Manifest, service: Option<&str>) -> Vec<Read> {
    reading(
        ctx,
        crate::app::plugins::moving_in::on_offer(ctx, manifest)
            .into_iter()
            .filter(|one| service.is_none_or(|named| named == one.service))
            .collect(),
    )
    .await
}

/// Each of `offered`, with its install read.
pub(crate) async fn reading(ctx: &Ctx, offered: Vec<MoveOffered>) -> Vec<Read> {
    let mut read = Vec::new();
    for offered in offered {
        let agreement = Box::pin(installing(ctx, &offered.plugin, Consent::default()))
            .await
            .map(|installs| installs.agreement.unwrap_or_default());
        read.push(Read { offered, agreement });
    }
    read
}

/// The words each move adds to the update's offer: the plugin and its install's offer.
pub(super) fn words(read: &[Read]) -> Vec<String> {
    read.iter()
        .flat_map(|one| {
            [
                one.offered.plugin.clone(),
                one.agreement.clone().unwrap_or_default(),
            ]
        })
        .collect()
}

/// Take every move `read` holds, each answering its own install's offer.
pub(crate) async fn taken(ctx: &Ctx, read: Vec<Read>) -> Vec<Moved> {
    let mut moved = Vec::new();
    for one in read {
        let outcome = match one.agreement {
            Ok(agreement) => Box::pin(installing(
                ctx,
                &one.offered.plugin,
                Consent {
                    agreement: Some(agreement),
                    ..Consent::default()
                },
            ))
            .await
            .and_then(|installs| held(installs.install)),
            Err(why) => Err(why),
        };
        moved.push(Moved {
            plugin: one.offered.plugin,
            service: one.offered.service,
            taken: outcome.is_ok(),
            why: outcome.err(),
        });
    }
    moved
}

/// Why an install that answered with nothing to record was not taken.
const UNRECORDED: &str = "its install recorded nothing";

/// Whether `install` was recorded, or why not: each of its proofs that did not hold and
/// each check of the stack's it broke.
fn held(install: Option<Box<crate::plugin::Install>>) -> Result<(), String> {
    let install = match install {
        Some(install) if install.recorded => return Ok(()),
        Some(install) => *install,
        None => return Err(UNRECORDED.to_owned()),
    };
    let unheld: Vec<String> = install
        .proofs
        .into_iter()
        .filter(|proving| proving.came_to != Some(crate::plugin::Verdict::Passed))
        .map(|proving| proving.proof)
        .chain(
            install
                .verified
                .into_iter()
                .flat_map(|verified| verified.broke)
                .map(|broke| broke.now.check),
        )
        .collect();
    Err(format!(
        "its install was taken back over {}",
        unheld.join(", ")
    ))
}

/// Install `plugin` from the tree this build carries, answering `consent`.
async fn installing(
    ctx: &Ctx,
    plugin: &str,
    consent: Consent,
) -> Result<crate::plugin::Installs, String> {
    crate::app::plugins::unguarded(
        ctx,
        &Asked::Install {
            source: Source::Name(plugin.to_owned()),
            consent,
        },
    )
    .await
    .map_err(|problem| problem.summary.clone())
}

#[cfg(test)]
mod tests;

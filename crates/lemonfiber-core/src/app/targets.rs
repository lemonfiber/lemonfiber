//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the curator-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.
//!
//! Five questions, one per file: where things sit, which services speak the curator shape,
//! which download clients the stack has, what lemonfiber recorded for itself, and how to
//! open a client for any of them. Re-exported as one, so callers see the module they
//! always did.
//!
//! And one answer assembled from two of them: what in this stack a feature that has to
//! know what a service *is* cannot cover. Three reports carry it — the status reading,
//! a seed pass and a queue-health reading — and assembling it here is what keeps those
//! three from each deciding it differently.

mod downloads;
mod filled;
mod layout;
pub(crate) use layout::services_config_dir;
mod curators;
mod media;
mod opening;
mod secrets;

/// Everything in this stack that a feature needing to know what a service is cannot
/// cover, each with why, in one settled order.
///
/// Two sources and no third: a shape this build does not speak at all, and a curator
/// declaration this build cannot reach through. A service declaring no API is in
/// neither — the manifest saying nothing is the stack's own statement that there is
/// nothing to integrate with, and repeating it back is not information.
///
/// Sorted by the service it names, so a report reads the same twice and two reports
/// carrying it agree line for line. The two sources cannot name one service between
/// them — a shape is either spoken or not — so nothing is reported twice.
pub(crate) fn unsupported_here(
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Vec<crate::model::UnsupportedReport> {
    let mut found = crate::unsupported::deferred(services);
    found.extend(curators::unreachable_targets(services, project));
    found.sort_by(|one, two| one.what.cmp(&two.what));
    found
}

pub(crate) use curators::*;
pub(crate) use downloads::*;
pub(crate) use filled::{identity, media, served_by, serving, spoken, Media, Spoken};
pub(crate) use layout::*;
pub(crate) use media::*;
pub(crate) use opening::*;
pub(crate) use secrets::*;

#[cfg(test)]
mod tests;

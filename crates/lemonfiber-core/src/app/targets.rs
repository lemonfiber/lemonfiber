//! Resolving stack services to what a read of them starts from — the project root a
//! file is read under, the curators and the download clients, each asked over its
//! contract. Seeding and diagnosis both begin here, so the resolution lives in one place
//! they can share.
//!
//! Five questions, one per file: where things sit, which curators there are, which
//! download clients the stack has, what lemonfiber recorded for itself, and how to ask
//! any of them. Re-exported as one, so callers see the module they always did.
//!
//! And one answer assembled from them: what in this stack a feature that has to know
//! what a service *is* cannot cover. Three reports carry it — the status reading,
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

/// Everything among `fillers` that a feature needing to know what a service is cannot
/// cover, each with why, in one settled order: every stack service declaring a native
/// API, which this build asks only through a plugin's adapter. A service declaring no API
/// is not among them — the manifest saying nothing is the stack's own statement that
/// there is nothing to integrate with, and repeating it back is not information.
///
/// Sorted by the service it names, so a report reads the same twice and two reports
/// carrying it agree line for line.
pub(crate) fn unsupported_here(
    fillers: &crate::wiring::Fillers,
) -> Vec<crate::model::UnsupportedReport> {
    let mut found = curators::unasked(fillers);
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

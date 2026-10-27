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

pub(crate) use curators::*;
pub(crate) use downloads::*;
pub(crate) use filled::{identity, media, served_by, server_named, serving, spoken, Media, Spoken};
pub(crate) use layout::*;
pub(crate) use media::*;
pub(crate) use opening::*;
pub(crate) use secrets::*;

#[cfg(test)]
mod tests;

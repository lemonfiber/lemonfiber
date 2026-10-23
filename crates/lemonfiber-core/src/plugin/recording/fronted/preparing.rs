//! What both containers of a recording are given before either starts: the throwaway
//! copies of their starting files, the adapter's key and its upstream's credentials.

use std::path::Path;

use lemonfiber_contract::adapter::KEY_FILE;

use super::{refused, Faulted, Fronting, Placed, Recorder, Starting, Throwaway};
use crate::ports::random::Random;

/// What both containers are given before either starts.
pub(super) struct Prepared {
    /// The throwaway copy of the upstream's starting files.
    pub(super) copy: Throwaway,
    /// Its throwaway library, where it takes one.
    pub(super) data: Option<std::path::PathBuf>,
    /// The throwaway copy of the adapter's starting files, which holds its key.
    pub(super) held: Throwaway,
    /// The key lemonfiber minted for the adapter.
    pub(super) key: String,
    /// Every credential minted for the run so far, which every answer is scrubbed of.
    pub(super) made: Vec<String>,
}

/// Each field of the adapter's `upstream`, holding the value its upstream's starting files
/// were written with, or one minted for the run where they name none.
fn handed(
    random: &dyn Random,
    copy: &Throwaway,
    held: &Throwaway,
    adapter: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let mut handed = Vec::new();
    for (field, secret) in &adapter.upstream {
        let value = if let Some(seeded) = copy.minted(secret).or_else(|| held.minted(secret)) {
            seeded.to_owned()
        } else {
            let fresh = crate::plugin::seeding::mint(random)
                .ok_or_else(|| crate::plugin::seeding::UNMINTED.to_owned())?;
            made.push(fresh.clone());
            fresh
        };
        handed.push((field.clone(), value));
    }
    Ok(handed)
}

impl Prepared {
    /// Both copies and the adapter's key, or why one could not be made.
    pub(super) fn of(
        recorder: Recorder<'_>,
        root: &Path,
        upstream: &Placed,
        adapter: Fronting<'_>,
    ) -> Result<Self, Faulted> {
        let copy = Throwaway::of(
            Some(Starting {
                root,
                service: &upstream.service,
            }),
            recorder.random,
        )
        .map_err(refused("starting files"))?;
        let data = if upstream.takes_data {
            Some(copy.emptied("data").map_err(refused("data"))?)
        } else {
            None
        };
        let held = Throwaway::of(
            Some(Starting {
                root,
                service: &adapter.placed.service,
            }),
            recorder.random,
        )
        .map_err(refused("adapter's starting files"))?;
        let key = crate::secret::generate(recorder.random).ok_or_else(|| {
            refused("adapter's key")(crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())
        })?;
        held.hold(KEY_FILE, &key)
            .map_err(refused("adapter's key"))?;
        let mut made: Vec<String> = std::iter::once(key.clone())
            .chain(copy.made().map(str::to_owned))
            .chain(held.made().map(str::to_owned))
            .collect();
        let handed = handed(recorder.random, &copy, &held, adapter.placed, &mut made)
            .map_err(refused("adapter's upstream credentials"))?;
        if !handed.is_empty() {
            super::filling::handed_over(&held, &handed)
                .map_err(refused("adapter's upstream credentials"))?;
        }
        Ok(Self {
            copy,
            data,
            held,
            key,
            made,
        })
    }
}

#[cfg(test)]
mod tests;

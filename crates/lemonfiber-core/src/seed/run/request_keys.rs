//! The request gate's key on each curator's adapter: minted here, written beside the
//! adapter's configuration, where the adapter reads it on every request, and handed to
//! the gate as that curator's route's credential.
//!
//! lemonfiber keeps no other copy: the file is where the key is read back from. Where the
//! gate presents none of the keys written there, none of them is in use, and a new one is
//! written in their place. A rotation writes the new key beside the one the gate presents,
//! so nothing the gate asks fails in between, and takes the old one out once the request
//! service's own test of the route has passed through the gate under the new.

use std::path::PathBuf;

use lemonfiber_contract::adapter::REQUEST_KEY_FILE;

use super::Ctx;
use crate::ports::digest::sha256;
use crate::ports::filesystem::{Beneath, Readable};
use crate::wiring::Filler;

/// The keys written for the gate on one curator's adapter, and where.
pub(crate) struct Keys {
    /// The file.
    at: PathBuf,
    /// The adapter's configuration directory, which the file stays beneath.
    within: PathBuf,
    /// Each key it holds.
    held: Vec<String>,
}

impl Keys {
    /// The keys written on `adapter`; none where nothing is written yet.
    ///
    /// # Errors
    ///
    /// Why they cannot be read: the adapter has no configuration on this machine, or
    /// something other than a plain file of its own is where they are kept.
    pub(crate) fn of(ctx: &Ctx, adapter: &Filler) -> Result<Self, String> {
        let within = adapter.confined_to.clone().ok_or_else(|| {
            format!(
                "{} has no configuration on this machine to hold the request gate's key",
                adapter.name
            )
        })?;
        let at = within.join(REQUEST_KEY_FILE);
        let held = match ctx.seams.confined.read(&at, &within) {
            Beneath::Read(written) => written
                .lines()
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_owned)
                .collect(),
            Beneath::Absent => Vec::new(),
            Beneath::Escaped => {
                return Err(format!(
                    "the request gate's key on {} is not a plain file of its own, so it was \
                     left as it is",
                    adapter.name
                ))
            }
        };
        Ok(Self { at, within, held })
    }

    /// The key held here whose SHA-256 is `presented`, the one the gate presents now.
    pub(crate) fn presented(&self, presented: &str) -> Option<&str> {
        self.held
            .iter()
            .find(|key| sha256(key) == presented)
            .map(String::as_str)
    }

    /// A new key, written in place of every key held here, which the adapter admits from
    /// its next request.
    ///
    /// # Errors
    ///
    /// Why no key could be minted or written.
    pub(super) fn fresh(&mut self, ctx: &Ctx) -> Result<String, String> {
        let key = minted(ctx)?;
        self.write(ctx, vec![key.clone()])?;
        Ok(key)
    }

    /// A new key, written beside `kept`, the one the gate presents, so both are admitted
    /// until the new one is proved.
    ///
    /// # Errors
    ///
    /// Why no key could be minted or written.
    pub(crate) fn beside(&mut self, ctx: &Ctx, kept: &str) -> Result<String, String> {
        let key = minted(ctx)?;
        self.write(ctx, vec![kept.to_owned(), key.clone()])?;
        Ok(key)
    }

    /// Hold `kept` alone.
    ///
    /// # Errors
    ///
    /// Why the file could not be written.
    pub(crate) fn only(&mut self, ctx: &Ctx, kept: &str) -> Result<(), String> {
        self.write(ctx, vec![kept.to_owned()])
    }

    /// Write `held` whole, readable by the adapter's user alone.
    fn write(&mut self, ctx: &Ctx, held: Vec<String>) -> Result<(), String> {
        let mut contents = held.join("\n");
        contents.push('\n');
        ctx.seams
            .confined
            .replaced(
                &self.at,
                &self.within,
                contents.as_bytes(),
                Readable::Private,
            )
            .map_err(|fault| {
                format!(
                    "the request gate's key could not be written: {}",
                    fault.message
                )
            })?;
        self.held = held;
        Ok(())
    }
}

/// A key minted for the gate.
fn minted(ctx: &Ctx) -> Result<String, String> {
    crate::secret::generate(ctx.seams.random.as_ref())
        .ok_or_else(|| crate::secret::NO_RANDOMNESS_FOR_TOKEN.to_owned())
}

#[cfg(test)]
mod tests;

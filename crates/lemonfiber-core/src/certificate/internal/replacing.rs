//! Replacing the internal certificate while every holder of a media server credential
//! keeps reaching the media server: the replacement kept beside the certificate in force
//! until each holder holds its SHA-256, then served in its place, and the one it replaced
//! kept until each holder reaches the media server with the new one.

use std::path::Path;

use super::{kept_at, made_for, Internal, INTERNAL};
use crate::certificate::Unkept;
use crate::ports::filesystem::{Confined, Readable};

/// Where a replacement is kept beneath the media server's configuration directory until it
/// is served.
pub const NEXT: &str = "lemonfiber/internal.next.p12";

/// Where the certificate a replacement took the place of is kept until it is withdrawn.
pub const PREVIOUS: &str = "lemonfiber/internal.previous.p12";

/// The certificates kept beneath the media server's configuration directory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Replacing {
    /// The one in force, which the media server serves.
    pub current: Option<Internal>,
    /// A replacement not yet served.
    pub next: Option<Internal>,
    /// The one a replacement took the place of, not yet withdrawn.
    pub previous: Option<Internal>,
}

impl Replacing {
    /// The SHA-256 of every certificate kept, each once, in order: what every holder of a
    /// media server credential is handed.
    #[must_use]
    pub fn pins(&self) -> Vec<String> {
        let mut pins: Vec<String> = [&self.current, &self.next, &self.previous]
            .into_iter()
            .flatten()
            .map(|kept| kept.fingerprint.clone())
            .collect();
        pins.sort();
        pins.dedup();
        pins
    }

    /// Whether a replacement is under way.
    #[must_use]
    pub const fn under_way(&self) -> bool {
        self.next.is_some() || self.previous.is_some()
    }
}

/// Every certificate kept beneath `config`, read through no link.
///
/// # Errors
///
/// [`Unkept::Unreadable`] where one that is kept is not one, or is reached through a link.
pub fn replacing(confined: &dyn Confined, config: &Path) -> Result<Replacing, Unkept> {
    Ok(Replacing {
        current: kept_at(confined, config, INTERNAL)?,
        next: kept_at(confined, config, NEXT)?,
        previous: kept_at(confined, config, PREVIOUS)?,
    })
}

/// The replacement kept beneath `config`, made out to `service` and written first where
/// none is.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read, or a new one cannot be made or written.
pub fn staged(confined: &dyn Confined, config: &Path, service: &str) -> Result<Internal, Unkept> {
    if let Some(next) = kept_at(confined, config, NEXT)? {
        return Ok(next);
    }
    let made = made_for(service)?;
    put(confined, config, NEXT, made.pkcs12())?;
    Ok(made)
}

/// The replacement put in place of the certificate in force, which is kept beside it
/// until it is withdrawn; nothing where no replacement is kept.
///
/// Taken up again where a run stopped part way: the certificate in force is kept aside
/// only while it is not yet the replacement.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read or written.
pub fn promoted(confined: &dyn Confined, config: &Path) -> Result<Option<Internal>, Unkept> {
    let Some(next) = kept_at(confined, config, NEXT)? else {
        return Ok(None);
    };
    match kept_at(confined, config, INTERNAL)? {
        Some(current) if current.fingerprint == next.fingerprint => {}
        Some(current) => {
            put(confined, config, PREVIOUS, current.pkcs12())?;
            put(confined, config, INTERNAL, next.pkcs12())?;
        }
        None => put(confined, config, INTERNAL, next.pkcs12())?,
    }
    confined
        .removed(&config.join(NEXT), config)
        .map_err(|fault| unmade(NEXT, &fault.message))?;
    Ok(Some(next))
}

/// The certificate a replacement took the place of taken away, so no holder is handed its
/// SHA-256 again.
///
/// # Errors
///
/// [`Unkept::Unmade`] where it cannot be taken away.
pub fn withdrawn(confined: &dyn Confined, config: &Path) -> Result<(), Unkept> {
    confined
        .removed(&config.join(PREVIOUS), config)
        .map_err(|fault| unmade(PREVIOUS, &fault.message))
}

/// `pkcs12` written at `at` beneath `config`, readable by its owner alone.
fn put(confined: &dyn Confined, config: &Path, at: &str, pkcs12: &[u8]) -> Result<(), Unkept> {
    confined
        .replaced(&config.join(at), config, pkcs12, Readable::Owner)
        .map_err(|fault| unmade(at, &fault.message))
}

/// Why `at` could not be written or taken away.
fn unmade(at: &str, why: &str) -> Unkept {
    Unkept::Unmade(format!("{at} could not be written: {why}"))
}

#[cfg(test)]
mod tests;

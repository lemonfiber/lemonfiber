//! The key an adapter presents to its upstream, as lemonfiber writes it into the adapter's
//! configuration directory.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use lemonfiber_contract::adapter::UPSTREAM_FILE;

/// The longest key taken.
pub const KEY_LIMIT: usize = 256;

/// The marks a generated key may hold beside letters and digits: those hex, base64 and a
/// UUID are written in.
const KEY_MARKS: &[u8] = b"-_.+/=";

/// Where the key every call presents comes from.
pub trait Holding: Send + Sync {
    /// The key held now.
    ///
    /// # Errors
    ///
    /// Why no key is held, in words that name no part of one.
    fn key(&self) -> Result<String, String>;
}

impl Holding for String {
    fn key(&self) -> Result<String, String> {
        Ok(self.clone())
    }
}

/// The key lemonfiber wrote into one configuration directory, under one field of
/// `upstream.json`.
#[derive(Debug, Clone)]
pub struct Upstream {
    /// The configuration directory.
    at: PathBuf,
    /// The field of `upstream.json` the key is under.
    field: &'static str,
}

impl Upstream {
    /// The key lemonfiber writes into `configuration` as `field`.
    #[must_use]
    pub fn in_directory(configuration: &Path, field: &'static str) -> Self {
        Self {
            at: configuration.to_path_buf(),
            field,
        }
    }
}

impl Holding for Upstream {
    fn key(&self) -> Result<String, String> {
        let field = self.field;
        let mut parts = crate::upstream_in(&self.at).map_err(|why| {
            if why.kind() == ErrorKind::InvalidData {
                format!("{UPSTREAM_FILE} is not an object of named strings")
            } else {
                format!("{UPSTREAM_FILE} could not be read: {why}")
            }
        })?;
        let key = parts
            .remove(field)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| format!("{UPSTREAM_FILE} names no {field}"))?;
        if spelled_as_a_key(&key) {
            Ok(key)
        } else {
            Err(format!(
                "the {field} in {UPSTREAM_FILE} is not spelled as a key is"
            ))
        }
    }
}

/// Whether `key` is spelled the way a generated key is: letters, digits and the marks
/// hex, base64 and a UUID are written in, and no longer than [`KEY_LIMIT`].
#[must_use]
pub fn spelled_as_a_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= KEY_LIMIT
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || KEY_MARKS.contains(&byte))
}

#[cfg(test)]
mod tests;

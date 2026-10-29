//! The certificate a service declares it serves with: where lemonfiber writes it and its
//! key beneath the service's configuration directory, and the names it is made out to.

use serde::{Deserialize, Serialize};

use super::paths::is_source;
use super::STREAM_GUARD;

/// The most names one certificate is made out to.
pub const MOST_NAMES: usize = 8;

/// The longest name a certificate is made out to, as a host name may be.
const LONGEST_NAME: usize = 253;

/// The certificate lemonfiber makes for a service before it first starts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceCertificate")]
pub struct Certificate {
    /// Where its private key is written, relative to the configuration directory.
    pub key: String,
    /// Where the certificate is written, relative to the configuration directory.
    pub certificate: String,
    /// The host names it is made out to.
    pub names: Vec<String>,
}

impl Certificate {
    /// Every way it is not one lemonfiber writes, for a service that provides `provides`.
    #[must_use]
    pub fn broken(&self, provides: &[String]) -> Vec<String> {
        let mut found = Vec::new();
        for path in [&self.key, &self.certificate] {
            if !is_source(path) {
                found.push(format!(
                    "{path} is not a relative path beneath its configuration directory"
                ));
            }
        }
        if self.key == self.certificate {
            found.push(format!(
                "{} is both the key's path and the certificate's",
                self.key
            ));
        }
        if self.names.is_empty() || self.names.len() > MOST_NAMES {
            found.push(format!(
                "names {} names, where a certificate is made out to one to {MOST_NAMES}",
                self.names.len()
            ));
        }
        found.extend(
            self.names
                .iter()
                .filter(|name| !is_host_name(name))
                .map(|name| {
                    format!(
                        "{name:?} is not a host name of lowercase letters, digits, \"-\" and \
                         \".\", at most {LONGEST_NAME} characters"
                    )
                }),
        );
        if provides.iter().any(|name| name == STREAM_GUARD) {
            found.push(format!(
                "it provides {STREAM_GUARD}, whose certificate the core makes for it"
            ));
        }
        found
    }
}

/// Whether `name` is a host name a certificate is made out to.
fn is_host_name(name: &str) -> bool {
    (1..=LONGEST_NAME).contains(&name.len())
        && name.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|letter| {
                    letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-'
                })
        })
}

#[cfg(test)]
mod tests;

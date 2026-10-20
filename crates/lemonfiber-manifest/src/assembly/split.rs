//! One manifest text written apart again: a root and a file per service.

use std::collections::BTreeMap;

use toml::{Table, Value};

use super::{EXTENSION, INCLUDE, SERVICE, SERVICES};

/// Why a manifest text could not be written apart.
#[derive(Debug, thiserror::Error)]
pub enum Unsplit {
    /// The text is not TOML.
    #[error("the manifest could not be parsed: {0}")]
    Syntax(#[from] toml::de::Error),
    /// A part of it could not be written back as TOML.
    #[error("a part of the manifest could not be written: {0}")]
    Unwritten(#[from] toml::ser::Error),
    /// The services are not a list, so none of them is a `[[service]]`.
    #[error("the services are not a list of [[service]] tables")]
    Unlisted,
    /// A service declares no `id`, so it has no file to be written to.
    #[error("a service declares no id")]
    Unnamed,
    /// Two services declare the same `id`, so they would be written to one file.
    #[error("more than one service declares the id {0}")]
    Repeated(String),
}

/// The manifest `text` as a root and each service's file by name: the root holds every
/// table but the services and an `include` naming each service's file in the order the
/// services are declared, and each file holds its one service. [`super::assemble`]
/// reads them back as the same manifest.
///
/// # Errors
///
/// [`Unsplit`] where `text` is not TOML, its services are not a list, a service
/// declares no `id` or one another declares too, or a part of it cannot be written
/// back.
pub fn split(text: &str) -> Result<(String, BTreeMap<String, String>), Unsplit> {
    let mut root: Table = toml::from_str(text)?;
    let services = match root.remove(SERVICE) {
        Some(Value::Array(services)) => services,
        Some(_) => return Err(Unsplit::Unlisted),
        None => Vec::new(),
    };
    let mut files = BTreeMap::new();
    let mut include = Vec::new();
    for service in services {
        let id = service
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Unsplit::Unnamed)?
            .to_owned();
        let file = format!("{id}{EXTENSION}");
        let mut alone = Table::new();
        alone.insert(SERVICE.into(), Value::Array(vec![service]));
        if files.insert(file.clone(), written(&alone)?).is_some() {
            return Err(Unsplit::Repeated(id));
        }
        include.push(Value::String(format!("{SERVICES}/{file}")));
    }
    root.insert(INCLUDE.into(), Value::Array(include));
    Ok((written(&root)?, files))
}

/// `table` as TOML text.
fn written(table: &Table) -> Result<String, Unsplit> {
    Ok(toml::to_string(table)?)
}

#[cfg(test)]
mod tests;

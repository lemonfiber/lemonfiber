//! The request gate, as the stack declares it and as its files are placed.
//!
//! The gate stands between the request service and the services it fulfils requests
//! through, holding their credentials so the request service holds only tokens
//! lemonfiber minted for the gate. Its files live in its configuration directory, in
//! the shapes [`lemonfiber_sidecar::gate`] defines.

use std::path::{Path, PathBuf};

use lemonfiber_sidecar::gate::File;

/// The request gate's id in the stack manifest.
pub(crate) const SERVICE: &str = "request-gate";

/// The request gate, where the stack runs one.
pub(crate) fn service(
    services: &[lemonfiber_manifest::Service],
) -> Option<&lemonfiber_manifest::Service> {
    services.iter().find(|service| service.id == SERVICE)
}

/// Where one of the gate's files lives: its configuration directory, under the project
/// root the stack's config volumes are mounted from.
pub(crate) fn path(project: &Path, file: File) -> PathBuf {
    project.join("config").join(SERVICE).join(file.name())
}

#[cfg(test)]
mod tests;

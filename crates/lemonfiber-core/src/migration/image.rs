//! Reading what an image is, and what is standing on it.
//!
//! The engine reports a container's project but not its image, and reports an image's
//! tags and the projects standing on it but not which container is which. So an image
//! is matched to one of lemonfiber's services by its repository — the part of a tag
//! before its version — and that one reading answers both questions asked here.

use crate::ports::docker::Image;

use super::Ours;
use crate::model::UnsupportedReport;

/// A name with any digest dropped: `repository:tag@sha256:…` and `repository@sha256:…`
/// are the names a digest-pinned image is started from and pulled under.
fn undigested(name: &str) -> &str {
    name.split_once('@').map_or(name, |(before, _)| before)
}

/// The repository part of an image name, with any version and any digest dropped.
///
/// A registry may itself carry a port, so only a final segment holding no path
/// separator is a version rather than part of the address.
#[must_use]
pub fn repository(name: &str) -> &str {
    let name = undigested(name);
    match name.rsplit_once(':') {
        Some((repository, version)) if !version.contains('/') => repository,
        _ => name,
    }
}

/// The version part of a name, which is what is left once the repository and any
/// digest are dropped. Empty for a name that carries only a digest.
#[must_use]
pub(crate) fn version_of(name: &str) -> &str {
    let named = undigested(name);
    named
        .get(repository(named).len()..)
        .and_then(|rest| rest.strip_prefix(':'))
        .unwrap_or_default()
}

/// The first tag of this image whose repository is one lemonfiber runs.
fn ours_among<'a>(image: &'a Image, ours: &[Ours]) -> Option<&'a String> {
    image
        .tags
        .iter()
        .find(|tag| ours.iter().any(|one| one.image == repository(tag)))
}

/// The version of a named service standing on a given project, where it is.
#[must_use]
pub(crate) fn standing_on(images: &[Image], project: &str, image: &str) -> Option<String> {
    images
        .iter()
        .filter(|pulled| pulled.projects.iter().any(|held| held == project))
        .find_map(|pulled| {
            pulled
                .tags
                .iter()
                .filter(|tag| repository(tag) == image)
                .map(|tag| version_of(tag))
                .find(|version| !version.is_empty())
                .map(str::to_owned)
        })
}

/// Every service lemonfiber knows that is running outside Compose.
///
/// A container started by hand carries no project for the engine to report, so it
/// cannot be listed the way a project can, and it is named from the image beneath it
/// instead. Narrowed to images lemonfiber runs: a machine has databases and build tools
/// standing on it that have nothing to do with a media stack, and naming those under a
/// migration survey would bury the one line that matters — a Jellyfin nobody can adopt
/// because there is no project description to adopt it from.
#[must_use]
pub(crate) fn outside_compose(images: &[Image], ours: &[Ours]) -> Vec<UnsupportedReport> {
    let mut named: Vec<UnsupportedReport> = images
        .iter()
        .filter(|image| image.projects.iter().any(String::is_empty))
        .filter_map(|image| ours_among(image, ours).cloned())
        .map(|what| UnsupportedReport {
            what,
            because: "it was started outside Compose, so there is no project description \
                      to take it over from"
                .to_owned(),
        })
        .collect();
    named.sort_by(|one, two| one.what.cmp(&two.what));
    named
}

#[cfg(test)]
mod tests;

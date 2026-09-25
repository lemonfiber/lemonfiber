//! Reading what an image is, and what is standing on it.
//!
//! The engine reports a container's project but not its image, and reports an image's
//! tags and the projects standing on it but not which container is which. So an image
//! is matched to one of lemonfiber's services by its repository — the part of a tag
//! before its version — and that one reading answers both questions asked here.

use crate::ports::docker::Image;

use super::Ours;
use crate::model::UnsupportedReport;

/// The repository part of an image tag, with any version dropped.
///
/// A registry may itself carry a port, so only a final segment holding no path
/// separator is a version rather than part of the address.
#[must_use]
pub fn repository(tag: &str) -> &str {
    match tag.rsplit_once(':') {
        Some((repository, version)) if !version.contains('/') => repository,
        _ => tag,
    }
}

/// The version part of a tag, which is what is left once the repository is dropped.
#[must_use]
pub(crate) fn version_of(tag: &str) -> &str {
    let repository = repository(tag);
    tag.get(repository.len()..)
        .map_or(tag, |rest| rest.strip_prefix(':').unwrap_or(tag))
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
                .find(|tag| repository(tag) == image)
                .map(|tag| version_of(tag).to_owned())
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

//! Correlating pulled images with the containers standing on them.
//!
//! The engine answers the two halves separately — `/images/json` says what is on
//! disk and how big it is, `/containers/json` says what is built on what — and
//! neither answers the question removal asks, which is whether taking an image away
//! would take something outside this stack with it.
//!
//! Correlation is by image id and falls back to the name a container was started
//! from. Both are needed: a container started from `linuxserver/sonarr:4.0.15`
//! reports that name and the id it resolved to, and an image re-tagged since keeps
//! the id while losing the name.
//!
//! **An image pulled by digest carries no tag.** A stack that names
//! `linuxserver/sonarr:4.0.20@sha256:…` is pulled by the digest, and the engine files
//! the image under `linuxserver/sonarr@sha256:…` and nothing else — the version lives
//! only in the name the container was started from. So an image is reported under its
//! tags, its digests, and every name a container was started from it by.
//!
//! Pure, so every case of it is driven without a daemon.

use std::collections::HashMap;

use bollard::models::{ContainerSummary, ImageSummary};

use lemonfiber_ports::docker::Image;

use super::PROJECT_LABEL;

/// The Compose projects standing on each image, keyed by image id and by every name
/// a container was started from.
///
/// Both keys point at the same list, so a lookup by either finds it. A container
/// under no Compose project contributes an empty entry rather than none at all,
/// because an image something outside Compose is built on is exactly as unsafe to
/// remove as one another project holds.
fn standing(containers: &[ContainerSummary]) -> HashMap<String, Vec<String>> {
    let mut held: HashMap<String, Vec<String>> = HashMap::new();
    for container in containers {
        let project = container
            .labels
            .as_ref()
            .and_then(|labels| labels.get(PROJECT_LABEL))
            .cloned()
            .unwrap_or_default();
        for key in [container.image_id.as_ref(), container.image.as_ref()]
            .into_iter()
            .flatten()
        {
            held.entry(key.clone()).or_default().push(project.clone());
        }
    }
    held
}

/// Every image, with the projects standing on it.
///
/// A negative size is the engine saying it has not calculated one, which becomes
/// zero rather than a wrapped figure: an unknown size understates a total, and a
/// total nobody could believe is worse than one that is short.
pub(super) fn correlate(images: Vec<ImageSummary>, containers: &[ContainerSummary]) -> Vec<Image> {
    let held = standing(containers);
    images
        .into_iter()
        .map(|image| {
            let mut projects: Vec<String> = std::iter::once(&image.id)
                .chain(image.repo_tags.iter())
                .chain(image.repo_digests.iter())
                .filter_map(|key| held.get(key))
                .flatten()
                .cloned()
                .collect();
            projects.sort_unstable();
            projects.dedup();
            let mut names: Vec<String> = image
                .repo_tags
                .into_iter()
                .chain(image.repo_digests)
                .chain(started_as(containers, &image.id))
                .collect();
            names.sort_unstable();
            names.dedup();
            Image {
                tags: names,
                bytes: u64::try_from(image.size).unwrap_or_default(),
                projects,
            }
        })
        .collect()
}

/// Every name a container was started from this image by.
fn started_as<'a>(
    containers: &'a [ContainerSummary],
    id: &'a str,
) -> impl Iterator<Item = String> + 'a {
    containers
        .iter()
        .filter(move |container| container.image_id.as_deref() == Some(id))
        .filter_map(|container| container.image.clone())
}

#[cfg(test)]
mod tests;

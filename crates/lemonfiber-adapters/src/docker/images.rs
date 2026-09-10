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
                .filter_map(|key| held.get(key))
                .flatten()
                .cloned()
                .collect();
            projects.sort_unstable();
            projects.dedup();
            Image {
                tags: image.repo_tags,
                bytes: u64::try_from(image.size).unwrap_or_default(),
                projects,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bollard::models::{ContainerSummary, ImageSummary};

    use super::correlate;
    use crate::docker::PROJECT_LABEL;

    /// An image summary with an id, its names and its size, which is all this reads.
    fn image(id: &str, tags: &[&str], size: i64) -> ImageSummary {
        ImageSummary {
            id: id.to_owned(),
            repo_tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            size,
            ..Default::default()
        }
    }

    /// A container built on an image, under a Compose project or under none.
    fn container(image_id: &str, from: &str, project: Option<&str>) -> ContainerSummary {
        ContainerSummary {
            image: Some(from.to_owned()),
            image_id: Some(image_id.to_owned()),
            labels: project.map(|name| {
                let mut labels = HashMap::new();
                labels.insert(PROJECT_LABEL.to_owned(), name.to_owned());
                labels
            }),
            ..Default::default()
        }
    }

    #[test]
    fn an_image_is_credited_to_every_project_standing_on_it() {
        let correlated = correlate(
            vec![image("sha256:aa", &["linuxserver/sonarr:4.0.15"], 400)],
            &[
                container("sha256:aa", "linuxserver/sonarr:4.0.15", Some("lemonfiber")),
                container(
                    "sha256:aa",
                    "linuxserver/sonarr:4.0.15",
                    Some("someone-else"),
                ),
            ],
        );

        assert_eq!(correlated.len(), 1);
        assert_eq!(
            correlated.first().map(|image| image.projects.clone()),
            Some(vec!["lemonfiber".to_owned(), "someone-else".to_owned()])
        );
    }

    /// The id is enough on its own, which is what keeps a re-tagged image correlated.
    #[test]
    fn a_container_whose_name_no_longer_matches_is_still_found_by_id() {
        let correlated = correlate(
            vec![image("sha256:bb", &["linuxserver/radarr:5.28"], 300)],
            &[container(
                "sha256:bb",
                "linuxserver/radarr:old",
                Some("lemonfiber"),
            )],
        );

        assert_eq!(
            correlated.first().map(|image| image.projects.clone()),
            Some(vec!["lemonfiber".to_owned()])
        );
    }

    /// A container outside Compose is a holder too, reported as the empty project.
    #[test]
    fn a_container_under_no_compose_project_is_still_standing_on_it() {
        let correlated = correlate(
            vec![image("sha256:cc", &["postgres:16"], 100)],
            &[container("sha256:cc", "postgres:16", None)],
        );

        assert_eq!(
            correlated.first().map(|image| image.projects.clone()),
            Some(vec![String::new()])
        );
    }

    /// Nothing built on it is nothing standing on it, and an untagged image is
    /// still an image occupying room.
    #[test]
    fn an_image_nothing_stands_on_is_reported_with_nobody_and_its_size() {
        let correlated = correlate(vec![image("sha256:dd", &[], 900)], &[]);

        let found = correlated.first().cloned();
        assert_eq!(found.as_ref().map(|image| image.bytes), Some(900));
        assert_eq!(found.map(|image| image.projects), Some(Vec::new()));
    }

    /// A size the engine has not calculated is reported as `-1`, and a total built
    /// from a wrapped figure would be unreadable.
    #[test]
    fn a_size_the_engine_never_calculated_counts_as_nothing() {
        let correlated = correlate(vec![image("sha256:ee", &["odd:1"], -1)], &[]);

        assert_eq!(correlated.first().map(|image| image.bytes), Some(0));
    }

    /// One project twice is one project: an image two of this stack's own services
    /// are built on is not thereby shared with anybody.
    #[test]
    fn one_project_holding_two_containers_is_named_once() {
        let correlated = correlate(
            vec![image("sha256:ff", &["alpine:3"], 10)],
            &[
                container("sha256:ff", "alpine:3", Some("lemonfiber")),
                container("sha256:ff", "alpine:3", Some("lemonfiber")),
            ],
        );

        assert_eq!(
            correlated.first().map(|image| image.projects.clone()),
            Some(vec!["lemonfiber".to_owned()])
        );
    }
}

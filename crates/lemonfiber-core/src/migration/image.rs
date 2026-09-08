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
pub fn version_of(tag: &str) -> &str {
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
pub fn standing_on(images: &[Image], project: &str, image: &str) -> Option<String> {
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
pub fn outside_compose(images: &[Image], ours: &[Ours]) -> Vec<UnsupportedReport> {
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
mod tests {
    use super::{outside_compose, repository, standing_on, version_of};
    use crate::migration::tests::{image, ours};

    #[test]
    fn a_version_is_dropped_to_leave_the_repository() {
        assert_eq!(repository("plex:1.2"), "plex");
        assert_eq!(version_of("plex:1.2"), "1.2");
    }

    #[test]
    fn an_image_named_without_a_version_is_all_repository() {
        assert_eq!(repository("plex"), "plex");
        assert_eq!(version_of("plex"), "plex");
    }

    #[test]
    fn a_registry_carrying_its_own_port_is_not_read_as_a_version() {
        assert_eq!(
            repository("example.test:5000/plex:1.2"),
            "example.test:5000/plex"
        );
        assert_eq!(
            repository("example.test:5000/plex"),
            "example.test:5000/plex"
        );
    }

    #[test]
    fn a_container_started_by_hand_is_named_from_the_image_beneath_it() {
        let images = [image(&["plex:latest"], &[""])];
        let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
        let what = named.first().map(|item| item.what.clone());
        assert_eq!(what, Some("plex:latest".to_owned()), "{named:?}");
    }

    #[test]
    fn an_image_only_projects_stand_on_is_not_named_as_unsupported() {
        let images = [image(&["plex:latest"], &["media"])];
        let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
        assert!(named.is_empty(), "{named:?}");
    }

    #[test]
    fn an_image_we_do_not_run_is_not_a_migration_finding() {
        let images = [image(&["a-database:17"], &[""])];
        let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
        assert!(named.is_empty(), "{named:?}");
    }

    #[test]
    fn what_was_started_outside_compose_reads_in_a_settled_order() {
        let images = [image(&["sonarr:1"], &[""]), image(&["plex:2"], &[""])];
        let ours = [
            ours("sonarr", "sonarr", "1", None),
            ours("plex", "plex", "2", None),
        ];
        let named: Vec<String> = outside_compose(&images, &ours)
            .into_iter()
            .map(|item| item.what)
            .collect();
        assert_eq!(named, vec!["plex:2".to_owned(), "sonarr:1".to_owned()]);
    }

    #[test]
    fn a_service_standing_on_a_project_answers_with_its_version() {
        let images = [image(&["linuxserver/sonarr:4.0.1"], &["media"])];
        assert_eq!(
            standing_on(&images, "media", "linuxserver/sonarr"),
            Some("4.0.1".to_owned())
        );
    }

    #[test]
    fn a_service_on_another_project_is_not_standing_on_this_one() {
        let images = [image(&["linuxserver/sonarr:4.0.1"], &["other"])];
        assert_eq!(standing_on(&images, "media", "linuxserver/sonarr"), None);
    }
}

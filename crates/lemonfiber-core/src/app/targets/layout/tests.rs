/// A plugin's service owns the directory named for it, and only where its id is one
/// plain name.
#[test]
fn a_plugins_service_owns_the_directory_named_for_it() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        super::plugin_config_dir(
            project,
            &crate::test_support::a_placed("nzbget", &[], Some(8080))
        ),
        Some(project.join("config/nzbget"))
    );
    assert_eq!(
        super::plugin_config_dir(
            project,
            &crate::test_support::a_placed("../etc", &[], Some(8080))
        ),
        None
    );
}

/// A stack file inside a service's configuration directory is held beneath that
/// directory, which its container owns; anything else beneath the project root.
#[test]
fn a_stack_file_is_held_beneath_its_services_directory_or_the_project() {
    use std::path::Path;
    let project = Path::new("/srv/stack");

    assert_eq!(
        super::held_beneath(project, Path::new("config/recyclarr/includes/a.yml")),
        project.join("config").join("recyclarr")
    );
    assert_eq!(
        super::held_beneath(project, Path::new("config/caddy/Caddyfile")),
        project.join("config").join("caddy")
    );
    assert_eq!(
        super::held_beneath(project, Path::new("compose/media.yml")),
        project
    );
    assert_eq!(super::held_beneath(project, Path::new("config/x")), project);
}

/// What [`super::read_owned`] reads of a service's settings file beneath the directory its
/// container owns, through `files`.
async fn read_through(files: crate::test_support::SeedFs) -> Option<String> {
    let owned = std::path::Path::new("/srv/stack/config/kept");
    super::read_owned(&files, &owned.join("settings.json"), owned).await
}

/// A file a service wrote is read only where it lies beneath the directory its container
/// owns: one that is not there, and one whose path leads away, read as nothing.
#[tokio::test]
async fn a_file_is_read_only_where_it_lies_beneath_its_owners_directory() {
    use crate::test_support::SeedFs;
    assert_eq!(
        read_through(SeedFs::keyed(Some("held"))).await.as_deref(),
        Some("held")
    );
    assert_eq!(
        read_through(SeedFs::keyed(Some("held")).missing(vec!["kept"])).await,
        None
    );
    assert_eq!(
        read_through(SeedFs::keyed(Some("held")).leading_away(vec!["settings.json"])).await,
        None
    );
}

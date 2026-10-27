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

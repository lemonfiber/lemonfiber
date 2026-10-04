use super::config_path;

fn a_service(id: &str) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: id.to_owned(),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        digest: None,
        port: None,
        bind: None,
        health: None,
        api: None,
        listens: None,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// A path under the usual mount reads from the service's own config directory.
///
/// The nested case is the subtitle finder's, whose file sits a directory deeper —
/// only the mount is stripped, not every `config` in the path.
#[test]
fn a_path_under_the_usual_mount_reads_from_the_services_own_directory() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        config_path(project, &a_service("sonarr"), Some("/config/config.xml")),
        Some(project.join("config/sonarr/config.xml"))
    );
    assert_eq!(
        config_path(
            project,
            &a_service("bazarr"),
            Some("/config/config/config.yaml")
        ),
        Some(project.join("config/bazarr/config/config.yaml"))
    );
}

/// The request service mounts its config beneath its application directory.
///
/// Not every service uses the same mount, and a path naming one this does not know
/// resolves to nothing rather than to a wrong file — which is why the list of
/// mounts is the first thing to check when a credential cannot be found.
#[test]
fn a_path_under_the_application_mount_reads_from_the_same_place() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        config_path(
            project,
            &a_service("seerr"),
            Some("/app/config/settings.json")
        ),
        Some(project.join("config/seerr/settings.json"))
    );
}

/// A path naming no mount this knows, and no path at all, both resolve to nothing.
#[test]
fn a_path_outside_every_known_mount_resolves_to_nothing() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        config_path(project, &a_service("odd"), Some("/etc/odd/settings.json")),
        None
    );
    assert_eq!(config_path(project, &a_service("odd"), None), None);
}

/// A plugin's service whose adapter reads its key from `path`.
fn keyed_at(service: &str, path: &str) -> crate::plugin::Placed {
    crate::test_support::a_placed(
        service,
        &[],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some(path.to_owned()),
            version: None,
        }),
        Some(8080),
    )
}

/// A plugin's credential is read from beneath its own configuration directory, at the
/// mount its record names.
#[test]
fn a_plugins_credential_is_read_beneath_its_own_directory() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        super::plugin_config_path(project, &keyed_at("nzbget", "/config/nzbget.conf")),
        Some(project.join("config/nzbget/nzbget.conf"))
    );
    assert_eq!(
        super::plugin_config_path(
            project,
            &crate::test_support::a_placed("nzbget", &[], None, None)
        ),
        None,
        "an adapter naming no file, or none at all, has nothing to read"
    );
}

/// Nothing a record holds can have the credential read from outside the service's own
/// directory: not a path that climbs out, not one beside the mount, not the directory
/// itself, and not a service id that is anything but one plain name.
#[test]
fn a_plugins_credential_is_never_read_from_outside_its_directory() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    for (service, path) in [
        ("nzbget", "/config/../../etc/shadow"),
        ("nzbget", "/etc/shadow"),
        ("nzbget", "/configuration/key"),
        ("nzbget", "/config"),
        ("nzbget", "/config/"),
        ("nzbget", "/config//etc/shadow"),
        ("../../etc", "/config/shadow"),
        ("nested/name", "/config/key"),
        ("", "/config/key"),
    ] {
        assert_eq!(
            super::plugin_config_path(project, &keyed_at(service, path)),
            None,
            "{service} reading {path}"
        );
    }
}

/// A plugin's service owns the directory named for it, and only where its id is one
/// plain name.
#[test]
fn a_plugins_service_owns_the_directory_named_for_it() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    assert_eq!(
        super::plugin_config_dir(project, &keyed_at("nzbget", "/config/key")),
        Some(project.join("config/nzbget"))
    );
    assert_eq!(
        super::plugin_config_dir(project, &keyed_at("../etc", "/config/key")),
        None
    );
}

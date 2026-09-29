use super::{Environment, HostOs, Manager, HOST_OS};

#[test]
fn linux_splits_on_whether_the_daemon_is_desktop() {
    assert_eq!(
        Environment::resolve(HostOs::Linux, false),
        Environment::LinuxNative
    );
    assert_eq!(
        Environment::resolve(HostOs::Linux, true),
        Environment::LinuxDesktop
    );
}

#[test]
fn the_other_platforms_do_not_depend_on_the_daemon() {
    for desktop in [true, false] {
        assert_eq!(
            Environment::resolve(HostOs::MacOs, desktop),
            Environment::MacOs
        );
        assert_eq!(
            Environment::resolve(HostOs::Windows, desktop),
            Environment::Windows
        );
        assert_eq!(
            Environment::resolve(HostOs::Other, desktop),
            Environment::Unsupported
        );
    }
}

#[test]
fn ownership_is_real_only_where_docker_runs_without_a_virtual_machine() {
    assert!(Environment::LinuxNative.ownership_is_real());
    for environment in [
        Environment::MacOs,
        Environment::LinuxDesktop,
        Environment::Windows,
        Environment::Unsupported,
    ] {
        assert!(!environment.ownership_is_real());
    }
}

#[test]
fn transcoding_in_docker_is_a_linux_capability() {
    assert!(Environment::LinuxNative.can_transcode_in_docker());
    assert!(Environment::LinuxDesktop.can_transcode_in_docker());
    for environment in [
        Environment::MacOs,
        Environment::Windows,
        Environment::Unsupported,
    ] {
        assert!(!environment.can_transcode_in_docker());
    }
}

#[test]
fn puid_pgid_is_asked_only_where_ownership_is_visible() {
    // The wizard asks for PUID/PGID only where it changes something the
    // operator can see — native Linux Docker, and nowhere else.
    assert!(Environment::LinuxNative.ownership_is_real());
    for environment in [
        Environment::MacOs,
        Environment::LinuxDesktop,
        Environment::Windows,
        Environment::Unsupported,
    ] {
        assert!(
            !environment.ownership_is_real(),
            "{environment:?} maps ownership away, so PUID/PGID must not be asked"
        );
    }
}

#[test]
fn native_jellyfin_is_offered_only_where_docker_cannot_transcode() {
    // Offered on macOS and Windows, where the Docker VM cannot reach the
    // encoder; never on Linux, where the container can.
    assert!(Environment::MacOs.offers_native_jellyfin());
    assert!(Environment::Windows.offers_native_jellyfin());
    for environment in [
        Environment::LinuxNative,
        Environment::LinuxDesktop,
        Environment::Unsupported,
    ] {
        assert!(
            !environment.offers_native_jellyfin(),
            "{environment:?} either transcodes in Docker or is unsupported"
        );
    }
}

#[test]
fn only_native_linux_has_to_be_told_about_the_host_gateway() {
    assert!(!Environment::LinuxNative.resolves_host_gateway());
    assert!(Environment::MacOs.resolves_host_gateway());
    assert!(Environment::LinuxDesktop.resolves_host_gateway());
    assert!(Environment::Windows.resolves_host_gateway());
    assert!(!Environment::Unsupported.resolves_host_gateway());
}

#[test]
fn each_platform_names_the_service_manager_it_actually_has() {
    assert_eq!(HostOs::MacOs.manager(), Manager::Launchd);
    assert_eq!(HostOs::Linux.manager(), Manager::Systemd);
    for host in [HostOs::Windows, HostOs::Other] {
        assert!(
            !host.manager().configurable(),
            "{host:?} has nothing lemonfiber configures"
        );
    }
}

#[test]
fn this_build_targets_a_platform_lemonfiber_supports() {
    assert_ne!(HOST_OS, HostOs::Other);
}

/// Docker Desktop on Linux is told apart by the context it installs and the socket it
/// listens on, and anything else Linux is pointed at is Docker Engine.
#[test]
fn docker_desktop_is_told_apart_by_where_its_engine_was_found() {
    use crate::ports::docker::{Origin, Target};

    let desktop_context = Target::at(
        "unix:///home/ana/.docker/desktop/docker.sock",
        Origin::Context("desktop-linux".to_owned()),
    );
    let desktop_socket = Target::socket("/home/ana/.docker/desktop/docker.sock");
    let named_only = Target {
        origin: Origin::Context("desktop-linux".to_owned()),
        ..Target::local()
    };
    for desktop in [&desktop_context, &desktop_socket, &named_only] {
        assert!(Environment::engine_is_desktop(desktop), "{desktop:?}");
        assert_eq!(
            Environment::resolve(HostOs::Linux, Environment::engine_is_desktop(desktop)),
            Environment::LinuxDesktop
        );
    }

    let engine = [
        Target::local(),
        Target::socket("/var/run/docker.sock"),
        Target::at("ssh://ana@server", Origin::Context("server".to_owned())),
        Target::at("tcp://10.0.0.2:2375", Origin::Variable),
    ];
    for native in &engine {
        assert!(!Environment::engine_is_desktop(native), "{native:?}");
    }
    assert_eq!(
        Environment::resolve(
            HostOs::Linux,
            Environment::engine_is_desktop(&Target::local())
        ),
        Environment::LinuxNative
    );
}

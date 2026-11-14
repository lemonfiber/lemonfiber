use super::{engine_is_desktop, engine_is_orbstack, manager, Environment, HostOs, Manager};

#[test]
fn each_platform_names_the_service_manager_it_actually_has() {
    assert_eq!(manager(HostOs::MacOs), Manager::Launchd);
    assert_eq!(manager(HostOs::Linux), Manager::Systemd);
    for host in [HostOs::Windows, HostOs::Other] {
        assert!(
            !manager(host).configurable(),
            "{host:?} has nothing lemonfiber configures"
        );
    }
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
        assert!(engine_is_desktop(desktop), "{desktop:?}");
        assert_eq!(
            Environment::resolve(HostOs::Linux, engine_is_desktop(desktop)),
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
        assert!(!engine_is_desktop(native), "{native:?}");
    }
    assert_eq!(
        Environment::resolve(HostOs::Linux, engine_is_desktop(&Target::local())),
        Environment::LinuxNative
    );
}

/// `OrbStack` is told apart by the context it installs and the socket it listens on.
#[test]
fn orbstack_is_told_apart_by_where_its_engine_was_found() {
    use crate::ports::docker::{Origin, Target};

    let orbstack = [
        Target::at(
            "unix:///Users/ana/.orbstack/run/docker.sock",
            Origin::Context("orbstack".to_owned()),
        ),
        Target::socket("/Users/ana/.orbstack/run/docker.sock"),
        Target {
            origin: Origin::Context("orbstack".to_owned()),
            ..Target::local()
        },
    ];
    for one in &orbstack {
        assert!(engine_is_orbstack(one), "{one:?}");
    }
    let other = [
        Target::local(),
        Target::socket("/var/run/docker.sock"),
        Target::socket("/Users/ana/.docker/run/docker.sock"),
        Target::at(
            "unix:///Users/ana/.docker/run/docker.sock",
            Origin::Context("desktop-linux".to_owned()),
        ),
    ];
    for one in &other {
        assert!(!engine_is_orbstack(one), "{one:?}");
    }
}

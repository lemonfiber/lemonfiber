use bollard::models::{MountPoint, PortSummary};

use super::{mounted, published};

/// One port as the engine reports it.
fn reported(ip: Option<&str>, public: Option<u16>) -> PortSummary {
    PortSummary {
        ip: ip.map(str::to_owned),
        private_port: 8989,
        public_port: public,
        typ: None,
    }
}

/// What the engine says it published is what comes back, address by address.
#[test]
fn every_address_the_engine_reports_is_read_back() {
    let read = published(vec![
        reported(Some("127.0.0.1"), Some(8989)),
        reported(Some("::"), Some(8989)),
    ]);
    assert_eq!(read.len(), 2);
    assert!(read
        .first()
        .is_some_and(|first| first.address.is_loopback()));
    assert!(read
        .get(1)
        .is_some_and(|second| second.address.is_unspecified()));
    assert!(read.iter().all(|entry| entry.port == 8989));
}

/// A port with no host address and one with no host port are the same thing: a
/// port the container exposes and nothing published, which has no binding for
/// anybody to have a policy about.
#[test]
fn a_port_nothing_published_is_not_a_binding() {
    assert!(published(vec![reported(None, Some(8989))]).is_empty());
    assert!(published(vec![reported(Some("0.0.0.0"), None)]).is_empty());
}

/// A word the engine gives that is not an address is left out rather than
/// guessed at, because what this list is for is holding real addresses to a rule.
#[test]
fn a_word_that_is_not_an_address_is_left_out() {
    assert!(published(vec![reported(Some("somewhere"), Some(8989))]).is_empty());
}

/// One mount as the engine reports it, with the host path it came from.
fn point(source: Option<&str>) -> MountPoint {
    MountPoint {
        source: source.map(std::borrow::ToOwned::to_owned),
        ..MountPoint::default()
    }
}

/// Where a container's data sits on this machine is what the hardlink question is
/// asked about, so the host path is what comes back.
#[test]
fn the_host_path_of_each_mount_is_what_comes_back() {
    let read = mounted(vec![
        point(Some("/srv/media")),
        point(Some("/mnt/downloads")),
    ]);
    assert_eq!(
        read,
        vec![
            std::path::PathBuf::from("/srv/media"),
            std::path::PathBuf::from("/mnt/downloads")
        ]
    );
}

/// A named or anonymous volume is the engine's own storage rather than somewhere in
/// the operator's tree, so it has no path to ask a hardlink question about.
#[test]
fn a_mount_with_no_host_path_is_not_somewhere_on_this_machine() {
    assert!(mounted(vec![point(None), point(Some(""))]).is_empty());
}

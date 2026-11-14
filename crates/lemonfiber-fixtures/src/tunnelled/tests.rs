use super::*;

/// A container the fake does not hold refuses an exec, and its stats end at once.
#[tokio::test]
async fn an_unheld_container_refuses_an_exec_and_its_stats_end_at_once() {
    let fake = Fake::new(vec![Behavior::up("vpn", Some("10.8.0.2"))]);

    assert!(matches!(
        fake.exec("id-elsewhere", &[]).await,
        Err(Failure::NoSuchContainer { name }) if name == "id-elsewhere"
    ));
    let stats = fake.stats("stack").await;
    assert!(stats.is_ok_and(|mut receiver| receiver.try_recv().is_err()));
}

/// A dropped device answers the route read with no default route, and names its device
/// again once it is back up.
#[test]
fn a_dropped_device_carries_no_default_route_until_it_is_back_up() {
    let fake = Fake::linked(vec![Behavior::up("vpn", Some("10.8.0.2"))], Link::holding());
    let ip =
        |args: &[&str]| fake.answer_ip(&args.iter().map(ToString::to_string).collect::<Vec<_>>());

    assert_eq!(ip(&["ip", "link", "set", "tun0", "down"]).status, Some(0));
    assert!(fake.is_dropped());
    assert_eq!(ip(&["ip", "route"]).stdout, "");

    assert_eq!(ip(&["ip", "link", "set", "tun0", "up"]).status, Some(0));
    assert_eq!(
        ip(&["ip", "route"]).stdout,
        "default via 10.8.0.1 dev tun0 proto static"
    );
}

/// An image with no `ip` answers as a missing program, a device that will not move refuses
/// to go down, and a restore that fails refuses to come back up.
#[test]
fn a_missing_ip_a_fixed_device_and_a_failing_restore_each_refuse() {
    let argv = |args: &[&str]| args.iter().map(ToString::to_string).collect::<Vec<_>>();
    let vpn = || vec![Behavior::up("vpn", Some("10.8.0.2"))];

    let unlinked = Fake::new(vpn());
    assert_eq!(
        unlinked.answer_ip(&argv(&["ip", "route"])).status,
        Some(127)
    );

    let fixed = Fake::linked(
        vpn(),
        Link {
            movable: false,
            ..Link::holding()
        },
    );
    assert_eq!(
        fixed
            .answer_ip(&argv(&["ip", "link", "set", "tun0", "down"]))
            .status,
        Some(2)
    );
    assert!(!fixed.was_dropped());

    let stuck = Fake::linked(
        vpn(),
        Link {
            restore: Restore::Fails,
            ..Link::holding()
        },
    );
    assert_eq!(
        stuck
            .answer_ip(&argv(&["ip", "link", "set", "tun0", "down"]))
            .status,
        Some(0)
    );
    assert_eq!(
        stuck
            .answer_ip(&argv(&["ip", "link", "set", "tun0", "up"]))
            .status,
        Some(2)
    );
    assert!(stuck.is_dropped() && stuck.was_dropped());
}

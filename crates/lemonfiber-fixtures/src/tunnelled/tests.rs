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

use super::is_ours;

#[test]
fn the_conditions_nothing_else_observes_are_ours() {
    for kind in [
        "vpn.egress.leaking",
        "vpn.egress.unverified",
        "service.stopped",
        "service.crash-looping",
        "storage.hardlinks-degraded",
        "queue.stalled",
        "notify.channel.refused",
    ] {
        assert!(is_ours(kind), "{kind}");
    }
}

#[test]
fn the_request_lifecycle_belongs_to_the_service_that_already_sends_it() {
    // Seerr tells the requester itself. A second message from lemonfiber is not
    // an extra courtesy — it is what teaches an operator to mute the channel.
    for kind in [
        "request.approved",
        "request.denied",
        "request.available",
        "household.member-added",
        "watchlist.synced",
    ] {
        assert!(!is_ours(kind), "{kind}");
    }
}

#[test]
fn a_domain_is_settled_by_its_leading_segment_and_not_by_a_list_of_events() {
    // So an event added later to a domain that is not ours is refused without
    // anyone having to remember to classify it.
    assert!(!is_ours("request.something-invented-later"));
    assert!(!is_ours("request"));
    // And a kind that merely mentions one of those words is unaffected.
    assert!(is_ours("queue.request-timeout"));
    assert!(is_ours(""));
}

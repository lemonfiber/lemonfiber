//! Which events exist, and which of them are lemonfiber's to raise.
//!
//! The division of labour is the whole point. Seerr already tells a household
//! member their request was approved and that it has arrived; the \*arrs have their
//! own connection systems. lemonfiber's unique contribution is the conditions
//! **nothing else observes** — a VPN leak, hardlinks quietly degrading to copies,
//! an item downloaded but never imported, a disk that will fill before the queue
//! drains.
//!
//! Duplicating what a service already sends is not a neutral extra. An operator
//! whose channel carries forty "download complete" messages a day mutes it within
//! a week, and takes "your VPN is leaking" down with it. So the boundary is
//! enforced here rather than merely written down: an event in a domain that
//! belongs to a service is refused, whatever raises it.

/// A namespace an event's kind can begin with, and who owns telling people about
/// it.
///
/// Matched on the leading segment, so a whole domain is settled by one rule rather
/// than by remembering to classify each new event as it is added.
const THEIRS: [&str; 3] = ["request", "household", "watchlist"];

/// Whether this kind of event is lemonfiber's to raise at all.
///
/// False for everything in a service's own domain — the request lifecycle Seerr
/// owns end to end, and anything addressed to a household member rather than to
/// the operator. lemonfiber does not chase a household member who has no
/// notification target either; that is Seerr's to handle.
#[must_use]
pub fn is_ours(kind: &str) -> bool {
    let domain = kind.split('.').next().unwrap_or(kind);
    !THEIRS.contains(&domain)
}

#[cfg(test)]
mod tests {
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
}

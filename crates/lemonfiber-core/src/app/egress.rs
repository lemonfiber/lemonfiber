//! Re-checking that traffic is still behind the tunnel, while it is moving.
//!
//! A leak found by a diagnosis is a leak that has already been happening for as
//! long as nobody ran one. The consequence of this particular fault reaches
//! outside the machine and cannot be undone afterwards, which makes it the one
//! check worth repeating rather than waiting to be asked for.
//!
//! Repeated only while there is something to leak. A stack with nothing
//! downloading has no torrent traffic to escape, so re-checking it spends an exec
//! into two containers to establish something that cannot currently be false —
//! and a check that runs constantly for no reason is one an operator disables.
//!
//! What counts as a leak is not decided here. The same [`crate::health::observed`]
//! translation the dashboard uses turns the tunnel reading into a fault, so a
//! watch and a panel cannot come to different conclusions about the same tunnel.

use crate::condition::Conditions;
use crate::dashboard::Panel;
use crate::doctor::vpn::{read_vpn, VpnReading};
use crate::health::{observed, Egress};

use super::Ctx;

/// What one re-check came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechecked {
    /// Nothing is downloading, so there is nothing that could be leaking.
    Idle,
    /// The client's traffic was proven to leave through the tunnel.
    Behind,
    /// It was proven to leave somewhere else.
    Leaking,
    /// It could not be established either way.
    Unverified,
}

impl Rechecked {
    /// Whether this is the moment an operator has to be interrupted.
    ///
    /// Only a proven leak. The consequence is outside the machine and every
    /// further second of it is more of the same, so it does not wait for a digest
    /// window or a quiet period — which the alert layer already grants anything
    /// critical.
    #[must_use]
    pub const fn is_urgent(self) -> bool {
        matches!(self, Self::Leaking)
    }
}

/// Re-read the tunnel and record what it says, where there is traffic to protect.
///
/// Recorded through the condition store, so a leak that starts is raised once and
/// a leak that stops is cleared — the notification layer then decides whether the
/// operator has already been told.
pub async fn recheck(ctx: &Ctx, conditions: &mut Conditions, active: bool, now: &str) -> Rechecked {
    if !active {
        return Rechecked::Idle;
    }
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Rechecked::Unverified;
    };
    let reading = read_vpn(
        ctx.engine.as_ref(),
        &ctx.settings.project,
        &manifest,
        ctx.settings.protocols,
        ctx.settings.ip_echo.clone(),
        ctx.settings.port_forward.enabled,
    )
    .await;
    let egress = match reading {
        // No VPN-contained client is not something to watch: there is no tunnel
        // this traffic was ever meant to be inside.
        VpnReading::NotApplicable => return Rechecked::Idle,
        VpnReading::Unavailable(_) => Egress::Unreadable,
        VpnReading::Ready { egress_matches, .. } if egress_matches => Egress::Behind,
        VpnReading::Ready { .. } => Egress::Leaking,
    };

    // The same translation the dashboard uses, so a watch and a panel cannot
    // disagree about one tunnel.
    for (check, fault) in observed(&[], egress) {
        conditions.observe(&check, fault.as_ref(), now);
    }

    match egress {
        Egress::Behind => Rechecked::Behind,
        Egress::Leaking => Rechecked::Leaking,
        Egress::Unreadable | Egress::NotApplicable => Rechecked::Unverified,
    }
}

/// Whether anything is downloading right now, from a transfers panel.
///
/// A source that could not be read counts as active: not knowing whether traffic
/// is moving is a reason to check the tunnel, not a reason to skip it.
#[must_use]
pub fn is_active(transfers: &Panel<Vec<crate::dashboard::Transfer>>) -> bool {
    match transfers {
        Panel::Ready(active) => !active.is_empty(),
        Panel::Unavailable { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_active, recheck, Rechecked};
    use crate::condition::Conditions;
    use crate::dashboard::{Panel, Protocol, Reading, Transfer};

    /// One download, so the stack counts as having traffic to protect.
    fn a_transfer() -> Transfer {
        Transfer {
            name: "Some.Release".to_owned(),
            protocol: Protocol::Torrent,
            progress: 42,
            speed: Reading::Known(1024),
            eta: None,
        }
    }

    #[test]
    fn a_stack_with_nothing_downloading_has_nothing_to_leak() {
        // Re-checking it would spend an exec into two containers to establish
        // something that cannot currently be false.
        assert!(!is_active(&Panel::Ready(Vec::new())));
        assert!(is_active(&Panel::Ready(vec![a_transfer()])));
    }

    #[test]
    fn transfers_that_could_not_be_read_count_as_traffic() {
        // Not knowing whether anything is moving is a reason to check the tunnel,
        // not a reason to skip it.
        let unread: Panel<Vec<Transfer>> = Panel::unavailable("the client did not answer");
        assert!(is_active(&unread));
    }

    #[test]
    fn only_a_proven_leak_interrupts_anybody() {
        // The consequence is outside the machine and every further second is more
        // of the same. Everything else can wait to be read.
        assert!(Rechecked::Leaking.is_urgent());
        for quiet in [Rechecked::Idle, Rechecked::Behind, Rechecked::Unverified] {
            assert!(!quiet.is_urgent(), "{quiet:?}");
        }
    }

    /// A context with no engine behind it — enough to reach the decisions that
    /// come before one is asked.
    fn ctx() -> crate::app::Ctx {
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            crate::config::Settings {
                // A stack with a torrent client, since a stack without one has no
                // tunnel this traffic was ever meant to be inside.
                protocols: crate::config::Protocols::both(),
                ..crate::config::Settings::default()
            },
            crate::platform::Environment::MacOs,
        )
    }

    #[tokio::test]
    async fn a_stack_with_no_torrent_client_is_never_watched() {
        // There is no tunnel this traffic was meant to be inside, so there is
        // nothing to be outside of.
        let usenet_only = crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            crate::config::Settings {
                protocols: crate::config::Protocols {
                    usenet: true,
                    torrent: false,
                },
                ..crate::config::Settings::default()
            },
            crate::platform::Environment::MacOs,
        );
        let mut conditions = Conditions::new();
        assert_eq!(
            recheck(&usenet_only, &mut conditions, true, "1000").await,
            Rechecked::Idle
        );
    }

    #[tokio::test]
    async fn an_idle_stack_is_not_asked_and_records_nothing() {
        let mut conditions = Conditions::new();
        assert_eq!(
            recheck(&ctx(), &mut conditions, false, "1000").await,
            Rechecked::Idle
        );
        assert!(conditions.is_empty(), "nothing was established either way");
    }

    #[tokio::test]
    async fn a_tunnel_that_cannot_be_read_is_recorded_as_unverified_not_as_safe() {
        // The engine answers nothing here, which is exactly the case where
        // reporting silence as "behind the tunnel" would be the comfortable lie.
        let mut conditions = Conditions::new();
        let rechecked = recheck(&ctx(), &mut conditions, true, "1000").await;
        assert_eq!(rechecked, Rechecked::Unverified);
        assert!(!rechecked.is_urgent(), "unknown is not an emergency");
    }

    /// A context whose engine holds the VPN pair and answers the probe with the
    /// given client address — matching the gateway's means behind the tunnel.
    fn ctx_with_tunnel(client_ip: Option<&'static str>) -> crate::app::Ctx {
        let engine = crate::test_support::Reporting::holding(
            &["gluetun", "qbittorrent"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::None,
        )
        .with_tunnel(crate::test_support::Tunnel {
            gateway: "gluetun",
            gateway_ip: Some("203.0.113.7"),
            client_ip,
            country: Some("nl"),
            port: None,
            second_opinion: None,
        });
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(engine),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            crate::config::Settings {
                protocols: crate::config::Protocols::both(),
                ip_echo: vec!["https://echo".to_owned()],
                ..crate::config::Settings::default()
            },
            crate::platform::Environment::MacOs,
        )
    }

    #[tokio::test]
    async fn traffic_still_behind_the_tunnel_is_recorded_and_says_nothing() {
        let mut conditions = Conditions::new();
        let rechecked = recheck(
            &ctx_with_tunnel(Some("203.0.113.7")),
            &mut conditions,
            true,
            "1000",
        )
        .await;
        assert_eq!(rechecked, Rechecked::Behind);
        assert!(!rechecked.is_urgent());
        assert!(conditions.raised().is_empty(), "nothing is wrong");
    }

    #[tokio::test]
    async fn traffic_leaving_outside_the_tunnel_is_urgent_and_raised_as_critical() {
        // The case the whole watch exists for: it is happening now, and every
        // further second is more of the same.
        let mut conditions = Conditions::new();
        let rechecked = recheck(
            &ctx_with_tunnel(Some("198.51.100.9")),
            &mut conditions,
            true,
            "1000",
        )
        .await;
        assert_eq!(rechecked, Rechecked::Leaking);
        assert!(rechecked.is_urgent());
        let severities: Vec<crate::error::Severity> = conditions
            .raised()
            .iter()
            .map(|condition| condition.severity)
            .collect();
        assert_eq!(severities, vec![crate::error::Severity::Critical]);
    }

    #[tokio::test]
    async fn a_leak_that_stops_clears_rather_than_standing() {
        // The notification layer decides whether the operator has already been
        // told; this only has to stop claiming it is still happening.
        let mut conditions = Conditions::new();
        recheck(
            &ctx_with_tunnel(Some("198.51.100.9")),
            &mut conditions,
            true,
            "1000",
        )
        .await;
        recheck(
            &ctx_with_tunnel(Some("203.0.113.7")),
            &mut conditions,
            true,
            "2000",
        )
        .await;
        assert!(
            conditions.raised().is_empty(),
            "it is behind the tunnel again"
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_establishes_nothing() {
        let nowhere = crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            crate::config::Settings {
                protocols: crate::config::Protocols::both(),
                ..crate::config::Settings::default()
            },
            crate::platform::Environment::MacOs,
        );
        let mut conditions = Conditions::new();
        assert_eq!(
            recheck(&nowhere, &mut conditions, true, "1000").await,
            Rechecked::Unverified
        );
    }
}

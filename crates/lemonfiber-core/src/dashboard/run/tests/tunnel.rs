//! The VPN panel, and what the summary reads across from it.

use super::*;

#[tokio::test]
async fn the_vpn_panel_shows_the_tunnel_when_it_answers() {
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(healthy_tunnel())),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(
        matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v))
            if v.exit_ip == "203.0.113.7"
                && v.country == "NL"
                && v.forwarded_port == Some(51413)
                && v.egress_matches),
        "the tunnel's exit, country, forwarded port, and a matching egress"
    );
}

#[tokio::test]
async fn a_panel_whose_address_services_contradict_each_other_says_so() {
    // The panel compares the same number the check does, so it has to refuse
    // the same way: an address chosen from among contradictory ones would show
    // an exit IP the operator could not rely on, beside a tick.
    let tunnel = Tunnel {
        second_opinion: Some("198.51.100.9"),
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec![
            "https://first.example".to_owned(),
            "https://second.example".to_owned(),
        ],
        forwarding(),
        Protocols::both(),
    );
    assert!(
        matches!(vpn_panel(&ctx).await, Some(Panel::Unavailable { reason })
            if reason.contains("disagree")),
        "the panel states it rather than picking one"
    );
}

#[tokio::test]
async fn a_client_whose_egress_differs_from_the_tunnel_is_flagged() {
    let tunnel = Tunnel {
        client_ip: Some("198.51.100.9"),
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if !v.egress_matches));
}

#[tokio::test]
async fn a_tunnel_that_is_not_running_leaves_the_panel_unavailable() {
    let ctx = vpn_ctx(
        tunnel_engine(false, Some(healthy_tunnel())),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_tunnel_that_returns_no_address_is_unavailable() {
    let tunnel = Tunnel {
        gateway_ip: None,
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_tunnel_the_engine_cannot_exec_is_unavailable() {
    // The gateway is up but the engine has nothing scripted, so the exec fails.
    let ctx = vpn_ctx(
        tunnel_engine(true, None),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_engine_leaves_the_vpn_panel_unavailable() {
    let ctx = vpn_ctx(
        Reporting::absent(),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn leak_detection_switched_off_leaves_the_egress_unreadable() {
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(healthy_tunnel())),
        Vec::new(),
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_stack_without_a_torrent_client_has_no_vpn_panel() {
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(healthy_tunnel())),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols {
            torrent: false,
            usenet: true,
        },
    );
    assert!(vpn_panel(&ctx).await.is_none());
}

#[tokio::test]
async fn port_forwarding_off_reads_no_forwarded_port() {
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(healthy_tunnel())),
        vec!["https://echo".to_owned()],
        PortForward::default(),
        Protocols::both(),
    );
    assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.forwarded_port.is_none()));
}

#[tokio::test]
async fn a_provider_that_granted_no_port_reads_none() {
    let tunnel = Tunnel {
        port: None,
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.forwarded_port.is_none()));
}

#[tokio::test]
async fn a_tunnel_without_a_country_still_reads() {
    let tunnel = Tunnel {
        country: None,
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.country.is_empty()));
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_leaves_the_vpn_panel_unavailable() {
    // An unreadable stack: the manifest resolves to a reason, and the driver
    // carries it into the panel rather than omitting it.
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo: vec!["https://echo".to_owned()],
        port_forward: forwarding(),
        ..Settings::default()
    };
    let ctx = a_context()
        .engine(Arc::new(tunnel_engine(true, Some(healthy_tunnel()))))
        .over(nowhere())
        .settings(settings)
        .build();
    assert!(matches!(
        vpn_panel(&ctx).await,
        Some(Panel::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_torrent_stack_with_no_vpn_pair_does_not_apply() {
    use crate::doctor::vpn::{read_vpn, VpnReading};
    let manifest = lemonfiber_manifest::Manifest {
        schema_version: 1,
        stack_version: String::new(),
        min_cli_version: String::new(),
        profiles: Vec::new(),
        forms: Vec::new(),
        services: Vec::new(),
        removed: Vec::new(),
        wirings: Vec::new(),
    };
    let reading = read_vpn(
        &Reporting::absent(),
        "lemonfiber",
        &manifest,
        Protocols::both(),
        vec!["https://echo".to_owned()],
        true,
    )
    .await;
    assert!(matches!(reading, VpnReading::NotApplicable));
}

#[test]
fn the_tunnel_panel_reads_across_to_the_summary_without_losing_the_middle_case() {
    use crate::health::Egress;
    // No panel is no VPN to be wrong about; a panel that could not be filled is
    // unverified rather than fine — the distinction the summary rests on.
    assert_eq!(super::super::egress(None), Egress::NotApplicable);
    assert_eq!(
        super::super::egress(Some(&Panel::unavailable("the gateway did not answer"))),
        Egress::Unreadable
    );
    assert_eq!(
        super::super::egress(Some(&vpn_panel_with(true))),
        Egress::Behind
    );
    assert_eq!(
        super::super::egress(Some(&vpn_panel_with(false))),
        Egress::Leaking
    );
}

#[tokio::test]
async fn a_healthy_stack_leaking_outside_the_tunnel_is_summarised_as_critical() {
    // The case the summary exists for, end to end: every container the engine
    // reports is healthy, and the download client's traffic is not behind the
    // tunnel. A count of running containers would call this fine.
    let tunnel = Tunnel {
        client_ip: Some("198.51.100.9"),
        ..healthy_tunnel()
    };
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(tunnel)),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    let snapshot = gather(&ctx, None).await;
    assert_eq!(snapshot.health.standing, Standing::Critical);
    assert!(snapshot.health.standing.wants_attention());
    // And the screen itself is fine, which is a separate matter entirely.
    assert_eq!(snapshot.telemetry, Telemetry::Live);
}

#[tokio::test]
async fn a_stack_behind_its_tunnel_with_nothing_wrong_is_healthy() {
    let ctx = vpn_ctx(
        tunnel_engine(true, Some(healthy_tunnel())),
        vec!["https://echo".to_owned()],
        forwarding(),
        Protocols::both(),
    );
    let snapshot = gather(&ctx, None).await;
    assert_eq!(snapshot.health.standing, Standing::Healthy);
    assert_eq!(snapshot.health.said(), "healthy");
}

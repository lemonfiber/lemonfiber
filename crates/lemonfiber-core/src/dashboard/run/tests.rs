use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::{gather, vpn};
use crate::app::Ctx;
use crate::config::{PortForward, Protocols, Settings};
use crate::dashboard::{Hardlink, Panel, Protocol, Reading, Telemetry, Transfer, Vpn};
use crate::health::Standing;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::filesystem::{FsKind, StorageFacts};
use crate::ports::http::Http;
use crate::stack::Source;
use crate::test_support::{
    a_context, env_without_password, guard, nowhere, torrent, usenet, Reporting, SeedFs, Tunnel,
};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Renamed;

/// Storage facts a volume with `total` bytes, `available` free, would report.
fn facts(available: u64, total: u64) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from("/"),
        kind: FsKind::Linking("ext4".to_owned()),
        removable: false,
        available,
        total,
    }
}

/// A context whose engine reports whatever the test put in it, configured with
/// a data root so it is not read as an unconfigured machine.
fn ctx(engine: Reporting) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(std::path::PathBuf::from("/srv/media")),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        .with_patience(Duration::ZERO)
}

/// Every service the `library` form declares.
const LIBRARY: [&str; 8] = [
    "jellyfin",
    "door",
    "seerr",
    "request-gate",
    "decline",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// The door a gather filled, or nothing where its panel could not be filled.
const fn filled(
    door: &Panel<crate::model::FrontDoorReport>,
) -> Option<&crate::model::FrontDoorReport> {
    match door {
        Panel::Ready(door) => Some(door),
        Panel::Unavailable { .. } | Panel::Pending => None,
    }
}

// ── VPN panel ─────────────────────────────────────────────────

/// A healthy tunnel scripted onto the tunnel and torrent client pair the stack
/// declares: matching egress and a country.
fn healthy_tunnel() -> Tunnel {
    Tunnel {
        gateway: "gluetun",
        gateway_ip: Some("203.0.113.7"),
        client_ip: Some("203.0.113.7"),
        country: Some("nl"),
        second_opinion: None,
    }
}

/// An engine holding the VPN pair in one lifecycle, optionally scripted to
/// answer the probe.
fn tunnel_engine(running: bool, tunnel: Option<Tunnel>) -> Reporting {
    let engine = Reporting::holding(
        &["gluetun", "qbittorrent"],
        if running {
            Lifecycle::Running
        } else {
            Lifecycle::Exited
        },
        Health::None,
    );
    match tunnel {
        Some(tunnel) => engine.with_tunnel(tunnel),
        None => engine,
    }
}

/// Port forwarding, enabled for a provider.
fn forwarding() -> PortForward {
    PortForward {
        enabled: true,
        provider: Some("proton".to_owned()),
    }
}

/// A context that reads the VPN through the given engine, with leak detection
/// (the IP-echo) and port forwarding as configured, and the egress guard answering that
/// its provider forwards 51413.
fn vpn_ctx(
    engine: Reporting,
    ip_echo: Vec<String>,
    port_forward: PortForward,
    protocols: Protocols,
) -> Ctx {
    guarded_vpn_ctx(engine, (ip_echo, port_forward, protocols), Some(51413))
}

/// As [`vpn_ctx`], the egress guard answering `forwarded`.
fn guarded_vpn_ctx(
    engine: Reporting,
    (ip_echo, port_forward, protocols): (Vec<String>, PortForward, Protocols),
    forwarded: Option<u16>,
) -> Ctx {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let env = env_without_password(&format!(
        "vpn-panel-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let settings = Settings {
        protocols,
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo,
        port_forward,
        stack_dir: env.parent().map(|config| config.join("data").join("stack")),
        env_file: Some(env),
        ..Settings::default()
    };
    guard::keyed(
        a_context()
            .engine(Arc::new(engine.alongside(guard::running())))
            .settings(settings)
            .build()
            .with_patience(Duration::ZERO)
            .with_http(Fake::by_path(vec![guard::forwarding(forwarded)])),
    )
}

/// The VPN panel the way `gather` reads it — the manifest resolved from the
/// stack, handed to the driver.
async fn vpn_panel(ctx: &Ctx) -> Option<Panel<Vpn>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| crate::error::Diagnose::problem(&err).summary);
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    vpn(ctx, manifest.as_ref(), project.as_deref()).await
}

// ── Storage: hardlink & exhaustion ────────────────────────────

/// A transfer moving at the given speed — the only field the rate reads.
fn a_transfer(speed: Reading<u64>) -> Transfer {
    Transfer {
        name: "download".to_owned(),
        protocol: Protocol::Torrent,
        progress: 0,
        speed,
        eta: None,
    }
}

// ── What the tunnel panel means to the summary ────────────────

/// A filled VPN panel whose client egress does or does not match the tunnel's.
fn vpn_panel_with(egress_matches: bool) -> Panel<Vpn> {
    Panel::Ready(Vpn {
        exit_ip: "203.0.113.7".to_owned(),
        country: "NL".to_owned(),
        forwarded_port: None,
        egress_matches,
    })
}

mod pacing;
mod panels;
mod refreshing;
mod tunnel;
mod waves;

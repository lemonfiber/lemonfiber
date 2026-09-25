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
use crate::test_support::{a_context, a_password, env_at, nowhere, Reporting, SeedFs, Tunnel};
use lemonfiber_fixtures::downloads::{
    downloads, QBIT_TORRENTS, SAB_KEY_INI, SAB_NO_KEY_INI, SAB_QUEUE,
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
const LIBRARY: [&str; 5] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// The door a gather filled, or nothing where its panel could not be filled.
const fn filled(door: &Panel<super::FrontDoorReport>) -> Option<&super::FrontDoorReport> {
    match door {
        Panel::Ready(door) => Some(door),
        Panel::Unavailable { .. } => None,
    }
}

// ── VPN panel ─────────────────────────────────────────────────

/// A healthy tunnel scripted onto the gluetun/qBittorrent pair the stack
/// declares: matching egress, a country, and a forwarded port.
fn healthy_tunnel() -> Tunnel {
    Tunnel {
        gateway: "gluetun",
        gateway_ip: Some("203.0.113.7"),
        client_ip: Some("203.0.113.7"),
        country: Some("nl"),
        port: Some("51413"),
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
/// (the IP-echo) and port forwarding as configured.
fn vpn_ctx(
    engine: Reporting,
    ip_echo: Vec<String>,
    port_forward: PortForward,
    protocols: Protocols,
) -> Ctx {
    let settings = Settings {
        protocols,
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo,
        port_forward,
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        .with_patience(Duration::ZERO)
}

/// The VPN panel the way `gather` reads it — the manifest resolved from the
/// stack, handed to the driver.
async fn vpn_panel(ctx: &Ctx) -> Option<Panel<Vpn>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| crate::error::Diagnose::problem(&err).summary);
    vpn(ctx, manifest.as_ref()).await
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

mod panels;
mod refreshing;
mod tunnel;

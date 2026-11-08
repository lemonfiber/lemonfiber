use lemonfiber_core::model::{
    Heartbeat, Unconfigured, WatchPath, WatchState, WatchedTarget, WatcherReport,
};
use lemonfiber_core::ports::service::{Covers, MonitorKind};

use super::watcher;

/// A monitor of `kind` seeing `state`, covering `covers`.
fn target(covers: Option<Covers>, kind: MonitorKind, state: WatchState) -> WatchedTarget {
    WatchedTarget {
        covers,
        name: format!("{kind:?}"),
        kind,
        state,
        since: 1_759_400_000,
        last_reading: Some(1_759_400_000),
        heartbeat: None,
        active: true,
        path: None,
        unconfigured: None,
    }
}

#[test]
fn each_monitor_is_said_beside_what_it_covers_and_what_nothing_watches_is_named() {
    let mut entry = target(
        Some(Covers::EntryPoint("seerr".to_owned())),
        MonitorKind::Http,
        WatchState::NotReporting,
    );
    entry.path = Some(WatchPath::Internal);
    entry.unconfigured = Some(Unconfigured {
        wanted: "the address the household is handed".to_owned(),
        watches: "http://seerr:5055".to_owned(),
        why: "no address".to_owned(),
    });
    let mut household = target(
        Some(Covers::EntryPoint("jellyfin".to_owned())),
        MonitorKind::Keyword,
        WatchState::Up,
    );
    household.path = Some(WatchPath::Household);
    let mut beat = target(
        Some(Covers::Job("guard".to_owned())),
        MonitorKind::Heartbeat,
        WatchState::Pending,
    );
    beat.heartbeat = Some(Heartbeat {
        every_s: 60,
        grace_s: 60,
        last_check_in: Some(1_759_399_000),
    });
    let mut never = target(
        Some(Covers::Job("expiring".to_owned())),
        MonitorKind::Heartbeat,
        WatchState::Down,
    );
    never.heartbeat = Some(Heartbeat {
        every_s: 3_600,
        grace_s: 3_600,
        last_check_in: None,
    });
    let mut paused = target(None, MonitorKind::Ping, WatchState::Paused);
    paused.active = false;
    let said = watcher(&WatcherReport {
        watcher: Some("uptime-kuma".to_owned()),
        reachable: true,
        read_at: 1_759_400_000,
        unreachable_because: None,
        monitors: vec![
            target(
                Some(Covers::Service("sonarr".to_owned())),
                MonitorKind::Port,
                WatchState::ExpectedDown,
            ),
            entry,
            household,
            beat,
            never,
            paused,
            target(None, MonitorKind::Dns, WatchState::Up),
            target(None, MonitorKind::Docker, WatchState::Up),
        ],
        uncovered: vec![Covers::Service("lidarr".to_owned())],
    })
    .text();

    for line in [
        "uptime-kuma watches this stack, as of 2025-10-02T10:13:20 UTC.",
        "Port (port), the service sonarr: down, as the stack was stopped on purpose since",
        "Http (http), the household's way in at seerr: not reporting since",
        "  over the stack's own network, not the household's address",
        "  asked to watch the address the household is handed, watches http://seerr:5055: no address",
        "Keyword (keyword), the household's way in at jellyfin: up since",
        "  over the address the household is handed",
        "Heartbeat (heartbeat), the job guard: failing, not yet alarmed since",
        "  expects a check-in every 60 s, 60 s grace; last checked in 2025-10-02T09:56:40 UTC",
        "the job expiring: down since",
        "  expects a check-in every 3600 s, 3600 s grace; it has never checked in",
        "Ping (ping), not something the stack runs: paused since",
        "  defined, and not checking",
        "Dns (dns)",
        "Docker (docker)",
        "Nothing actively watches:",
        "  the service lidarr",
    ] {
        assert!(said.contains(line), "{line}\n{said}");
    }
}

#[test]
fn a_watcher_that_does_not_answer_or_is_not_there_is_said_as_blind() {
    let blind = watcher(&WatcherReport {
        watcher: Some("uptime-kuma".to_owned()),
        reachable: false,
        read_at: 1_759_400_000,
        unreachable_because: Some("its database is locked".to_owned()),
        monitors: Vec::new(),
        uncovered: Vec::new(),
    })
    .text();
    assert!(
        blind.contains("uptime-kuma does not answer, so nobody can see what it watches"),
        "{blind}"
    );
    assert!(blind.contains("  its database is locked"), "{blind}");
    assert!(!blind.contains("Nothing actively watches"), "{blind}");

    let absent = watcher(&WatcherReport::default()).text();
    assert!(
        absent.contains("Nothing watches this stack from outside it"),
        "{absent}"
    );
}

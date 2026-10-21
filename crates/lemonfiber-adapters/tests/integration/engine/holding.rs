//! What the engine holds for a project beside its containers: the networks it made, and
//! the volumes a service kept.

use super::fake;
use lemonfiber_adapters::Daemon;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::docker::{EngineVolumes as _, Failure, Networks as _};

/// Two networks the project made, and one Compose did not name.
#[cfg(unix)]
const NETWORKS: &str = concat!(
    r#"[{"Name":"lemonfiber_own__sonarr","Labels":{"com.docker.compose.network":"own__sonarr"}},"#,
    r#"{"Name":"lemonfiber_link__a__b","Labels":{"com.docker.compose.network":"link__a__b"}},"#,
    r#"{"Name":"unlabelled","Labels":{}}]"#
);

/// A network one container is still on.
#[cfg(unix)]
const HELD: &str = r#"{"Name":"lemonfiber_old","Containers":{"c1":{"Name":"sonarr"}}}"#;

#[cfg(unix)]
#[tokio::test]
async fn the_networks_a_project_made_are_listed_by_the_name_its_file_declares() {
    let engine = fake::engine(
        "networks",
        vec![("networks", fake::Reply::Body(200, NETWORKS.to_owned()))],
    );

    let listed = Daemon::at(&engine.socket).networks("lemonfiber").await;

    assert_eq!(
        listed.ok().map(|networks| networks
            .into_iter()
            .map(|network| (network.name, network.declared))
            .collect::<Vec<_>>()),
        Some(vec![
            (
                "lemonfiber_own__sonarr".to_owned(),
                "own__sonarr".to_owned()
            ),
            ("lemonfiber_link__a__b".to_owned(), "link__a__b".to_owned()),
        ])
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_network_is_taken_away_after_every_container_on_it_is_disconnected() {
    let mut engine = fake::engine(
        "network-removed",
        vec![
            ("disconnect", fake::Reply::Body(200, String::new())),
            (
                "networks/lemonfiber_old",
                fake::Reply::Body(200, HELD.to_owned()),
            ),
        ],
    );

    let removed = Daemon::at(&engine.socket).removed("lemonfiber_old").await;

    assert!(removed.is_ok(), "{removed:?}");
    let asked = engine.asked_for();
    assert!(
        asked.iter().any(|path| path.contains("disconnect")),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .filter(|path| path.ends_with("networks/lemonfiber_old"))
            .count()
            == 2,
        "inspected, then removed: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_network_the_engine_will_not_let_go_of_is_said_to_be_refused() {
    let refusing = fake::engine(
        "network-refused",
        vec![
            (
                "disconnect",
                fake::Reply::Body(500, r#"{"message":"busy"}"#.to_owned()),
            ),
            (
                "networks/lemonfiber_old",
                fake::Reply::Body(200, HELD.to_owned()),
            ),
        ],
    );
    let unknown = fake::engine(
        "network-unknown",
        vec![(
            "networks",
            fake::Reply::Body(500, r#"{"message":"down"}"#.to_owned()),
        )],
    );

    assert!(Daemon::at(&refusing.socket)
        .removed("lemonfiber_old")
        .await
        .is_err());
    assert!(Daemon::at(&unknown.socket)
        .removed("lemonfiber_old")
        .await
        .is_err());
    assert!(Daemon::at(&unknown.socket)
        .networks("lemonfiber")
        .await
        .is_err());
    refusing.stop().await;
    unknown.stop().await;
}

/// A volume as the engine describes one.
#[cfg(unix)]
const VOLUME: &str = concat!(
    r#"{"Name":"lemonfiber_caddy_data","Driver":"local","Mountpoint":"/var/lib/docker/v","#,
    r#""Labels":{},"Scope":"local","Options":{}}"#
);

#[cfg(unix)]
#[tokio::test]
async fn whether_a_volume_is_kept_is_answered_and_an_engine_that_fails_says_so() {
    let engine = fake::engine(
        "volumes",
        vec![
            ("volumes/kept", fake::Reply::Body(200, VOLUME.to_owned())),
            (
                "volumes/broken",
                fake::Reply::Body(500, r#"{"message":"down"}"#.to_owned()),
            ),
        ],
    );
    let daemon = Daemon::at(&engine.socket);

    assert!(matches!(daemon.kept("kept").await, Ok(true)));
    assert!(matches!(daemon.kept("absent").await, Ok(false)));
    assert!(matches!(
        daemon.kept("broken").await,
        Err(Failure::Unreachable { .. })
    ));
    engine.stop().await;
}

/// The archive the engine hands over for a volume holding one file.
#[cfg(unix)]
fn archived() -> String {
    let mut out = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_ustar();
    header.set_size(5);
    header.set_mode(0o600);
    header.set_cksum();
    let _ = out.append_data(&mut header, "from/kept.db", "state".as_bytes());
    String::from_utf8(out.into_inner().unwrap_or_default()).unwrap_or_default()
}

#[cfg(unix)]
#[tokio::test]
async fn a_volume_is_copied_out_through_a_container_that_is_removed_after() {
    let dir = Scratch::new("engine-copied");
    let within = dir.join("stack");
    let into = within.join("mounts").join("caddy").join("data");
    let _ = std::fs::create_dir_all(&within);
    let mut engine = fake::engine(
        "copied",
        vec![
            ("/archive", fake::Reply::Body(200, archived())),
            (
                "containers/create",
                fake::Reply::Body(201, r#"{"Id":"c1","Warnings":[]}"#.to_owned()),
            ),
            ("containers/c1", fake::Reply::Body(204, String::new())),
        ],
    );

    let copied = Daemon::at(&engine.socket)
        .copied("lemonfiber_caddy_data", "caddy:2", &into, &within)
        .await;

    assert!(copied.is_ok(), "{copied:?}");
    assert_eq!(
        std::fs::read_to_string(into.join("kept.db"))
            .ok()
            .as_deref(),
        Some("state")
    );
    let asked = engine.asked_for();
    assert!(
        asked.iter().any(|path| path.contains("containers/c1?")),
        "removed: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_image_the_engine_has_not_got_is_pulled_before_the_container_is_made_again() {
    let dir = Scratch::new("engine-pulled");
    let within = dir.join("stack");
    let _ = std::fs::create_dir_all(&within);
    let mut engine = fake::engine(
        "pulled",
        vec![
            (
                "containers/create",
                fake::Reply::Body(404, r#"{"message":"No such image"}"#.to_owned()),
            ),
            (
                "images/create",
                fake::Reply::Body(200, r#"{"status":"pulled"}"#.to_owned()),
            ),
        ],
    );

    let copied = Daemon::at(&engine.socket)
        .copied(
            "lemonfiber_caddy_data",
            "caddy:2",
            &within.join("data"),
            &within,
        )
        .await;

    assert!(copied.is_err());
    let asked = engine.asked_for();
    assert!(
        asked.iter().any(|path| path.contains("images/create")),
        "{asked:?}"
    );
    assert_eq!(
        asked
            .iter()
            .filter(|path| path.contains("containers/create"))
            .count(),
        2,
        "{asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_archive_that_cannot_be_written_where_it_goes_is_refused() {
    let dir = Scratch::new("engine-unwritten");
    let within = dir.join("stack");
    let _ = std::fs::create_dir_all(&within);
    let engine = fake::engine(
        "unwritten",
        vec![
            ("/archive", fake::Reply::Body(200, archived())),
            (
                "containers/create",
                fake::Reply::Body(201, r#"{"Id":"c1","Warnings":[]}"#.to_owned()),
            ),
            ("containers/c1", fake::Reply::Body(204, String::new())),
        ],
    );

    let copied = Daemon::at(&engine.socket)
        .copied(
            "lemonfiber_caddy_data",
            "caddy:2",
            &dir.join("elsewhere"),
            &within,
        )
        .await;

    assert!(copied.is_err());
    engine.stop().await;
}

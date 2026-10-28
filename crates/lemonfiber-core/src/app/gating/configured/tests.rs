use std::sync::Arc;

use lemonfiber_adapters::filesystem::Disk;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_sidecar::gate::{Accepted, Credential, File, Kind, Tokens, Upstream, Upstreams};
use lemonfiber_sidecar::TokenHash;

use super::Configured;
use crate::app::gating::path;
use crate::ports::service::{Admitted, Gated, Gating, Target};

/// The stack's own gate under a fresh project.
fn gate(scratch: &Scratch) -> Configured {
    Configured::at(scratch.path(), Arc::new(Disk))
}

/// A route to the television curator and one to the media server, each admitting one
/// token.
fn routes() -> Vec<Gated> {
    vec![
        Gated {
            target: Target::Curator {
                media_type: "tv".to_owned(),
            },
            path: "sonarr".to_owned(),
            address: "http://sonarr:8989".to_owned(),
            majors: Vec::new(),
            credential: "sonarrs-key".to_owned(),
            tokens: vec![TokenHash::of("tv-token").as_str().to_owned()],
        },
        Gated {
            target: Target::MediaServer { pins: Vec::new() },
            path: "jellyfin".to_owned(),
            address: "http://jellyfin:8096".to_owned(),
            majors: vec![10],
            credential: "gates-key".to_owned(),
            tokens: vec![TokenHash::of("media-token").as_str().to_owned()],
        },
    ]
}

/// The routes admitted are the two files the gate reads, and are answered back with each
/// credential by its SHA-256 alone.
#[tokio::test]
async fn the_routes_admitted_are_the_files_the_gate_reads_and_answered_by_hash() {
    let scratch = Scratch::new("gate-configured-admit");
    let gate = gate(&scratch);

    assert_eq!(gate.admitted().await.ok(), Some(Vec::new()));
    assert!(gate.admit(&routes()).await.is_ok());

    let upstreams = std::fs::read_to_string(path(scratch.path(), File::Upstreams))
        .ok()
        .and_then(|text| Upstreams::read(&text).ok());
    assert_eq!(
        upstreams,
        Some(Upstreams::of(vec![
            Upstream {
                route: "sonarr".to_owned(),
                kind: Kind::Sonarr,
                address: "http://sonarr:8989".to_owned(),
                credential: Credential::new("sonarrs-key"),
                majors: Vec::new(),
                pins: lemonfiber_sidecar::Pins::default(),
            },
            Upstream {
                route: "jellyfin".to_owned(),
                kind: Kind::Jellyfin,
                address: "http://jellyfin:8096".to_owned(),
                credential: Credential::new("gates-key"),
                majors: vec![10],
                pins: lemonfiber_sidecar::Pins::default(),
            },
        ]))
    );
    let tokens = std::fs::read_to_string(path(scratch.path(), File::Tokens))
        .ok()
        .and_then(|text| Tokens::read(&text).ok());
    assert_eq!(
        tokens,
        Some(Tokens::of(vec![
            Accepted {
                route: "sonarr".to_owned(),
                tokens: vec![TokenHash::of("tv-token")],
            },
            Accepted {
                route: "jellyfin".to_owned(),
                tokens: vec![TokenHash::of("media-token")],
            },
        ]))
    );
    let answered = gate.admitted().await.unwrap_or_default();
    assert_eq!(
        answered,
        routes().iter().map(Admitted::of).collect::<Vec<_>>()
    );
    let said = serde_json::to_string(&answered).unwrap_or_default();
    assert!(
        !said.contains("sonarrs-key") && !said.contains("gates-key"),
        "{said}"
    );
}

/// A route to a curator the gate speaks to none of is refused, and nothing is written.
#[tokio::test]
async fn a_route_to_a_curator_of_music_is_refused_and_nothing_written() {
    let scratch = Scratch::new("gate-configured-music");
    let gate = gate(&scratch);
    let mut music = routes();
    music.truncate(1);
    for route in &mut music {
        route.target = Target::Curator {
            media_type: "music".to_owned(),
        };
    }

    assert!(gate.admit(&music).await.is_err());
    assert!(!path(scratch.path(), File::Upstreams).exists());
    assert!(!path(scratch.path(), File::Tokens).exists());
}

/// A film curator's route is the gate's film kind, and is answered as the curator of
/// films it is.
#[tokio::test]
async fn a_film_curators_route_round_trips_as_one() {
    let scratch = Scratch::new("gate-configured-films");
    let gate = gate(&scratch);
    let films = vec![Gated {
        target: Target::Curator {
            media_type: "movies".to_owned(),
        },
        path: "radarr".to_owned(),
        address: "http://radarr:7878".to_owned(),
        majors: Vec::new(),
        credential: "radarrs-key".to_owned(),
        tokens: Vec::new(),
    }];

    assert!(gate.admit(&films).await.is_ok());

    let written =
        std::fs::read_to_string(path(scratch.path(), File::Upstreams)).unwrap_or_default();
    assert!(written.contains("\"radarr\""), "{written}");
    assert_eq!(
        gate.admitted().await.ok(),
        Some(films.iter().map(Admitted::of).collect())
    );
}

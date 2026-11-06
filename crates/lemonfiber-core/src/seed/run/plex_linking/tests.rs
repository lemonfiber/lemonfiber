use super::{admitting, gate_tokens, route};
use crate::ports::digest::sha256;
use crate::ports::service::{Admitted, OutsideAccount, OutsideServer, PlexLink, Target, PLEX_PORT};

const KEY: &str = "managed-account-token";

/// The Plex route `plex` as the gate admits it, admitting `tokens`.
fn admitted(tokens: &[String]) -> Vec<Admitted> {
    vec![Admitted {
        target: Target::Plex {
            pins: Vec::new(),
            link: PlexLink::Managed,
        },
        path: "plex".to_owned(),
        address: "https://plex:32443".to_owned(),
        majors: Vec::new(),
        credential: sha256(KEY),
        tokens: tokens.to_vec(),
    }]
}

/// A fronted link.
fn fronted() -> PlexLink {
    PlexLink::Fronted {
        account: OutsideAccount {
            id: "1".to_owned(),
            uuid: "u".to_owned(),
            username: "lemonfiber-gate".to_owned(),
            title: "lemonfiber-gate".to_owned(),
        },
        members: Vec::new(),
        server: OutsideServer {
            name: "Living room".to_owned(),
            id: "m".to_owned(),
        },
    }
}

#[test]
fn the_plex_route_is_the_gate_at_plexs_own_port_with_no_path() {
    let at = route();
    assert_eq!(at.host, crate::app::gating::SERVICE);
    assert_eq!(at.port, PLEX_PORT);
    assert!(at.base.is_empty());
}

#[test]
fn each_mode_admits_its_own_tokens_on_the_plex_route() {
    let gate_token = sha256("gate-token");
    let now = vec![sha256(KEY), gate_token.clone()];

    assert_eq!(admitting(&PlexLink::Managed, KEY, &now), vec![sha256(KEY)]);
    assert!(admitting(
        &PlexLink::Owner {
            account: "4815162342".to_owned()
        },
        KEY,
        &now
    )
    .is_empty());
    assert_eq!(admitting(&fronted(), KEY, &now), vec![gate_token]);
}

#[test]
fn the_gate_tokens_are_every_token_on_the_route_but_the_managed_accounts() {
    let gate_token = sha256("gate-token");
    let on = admitted(&[sha256(KEY), gate_token.clone()]);

    assert_eq!(gate_tokens(&on, "plex", KEY), vec![gate_token]);
    assert!(gate_tokens(&on, "jellyfin", KEY).is_empty());
    assert!(gate_tokens(&admitted(&[sha256(KEY)]), "plex", KEY).is_empty());
}

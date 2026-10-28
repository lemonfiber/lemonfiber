use std::collections::BTreeMap;

use lemonfiber_plugin::Manifest;

use super::super::waiting::waiting;
use crate::model::{Waiting, Wanted};
use crate::plugin::Installed;

const GUARD: &str = r#"
schema_version = 1

[plugin]
id          = "guard"
name        = "Guard"
version     = "1.0.0"
description = "Carries the downloads through a tunnel"
without_it  = "Downloads over the open line"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "tunnel"
name        = "Tunnel"
image       = "example.invalid/tunnel"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "important"
config_path = "/config"

[service.env]
VPN_TYPE      = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
ACCOUNT       = { setting = "guard:account", default = "", recorded_with = "0000" }
WIREGUARD_KEY = { secret = "wireguard-key" }
OPENVPN_USER  = { secret = "openvpn-user" }

[[secret]]
id            = "wireguard-key"
of            = "tunnel"
why           = "Dials the tunnel"
recorded_with = "minted"
needs         = { "guard:vpn-type" = ["wireguard"] }

[[secret]]
id            = "openvpn-user"
of            = "tunnel"
why           = "Signs in over OpenVPN"
recorded_with = "minted"
needs         = { "guard:vpn-type" = ["openvpn"] }
"#;

fn guard() -> Vec<Installed> {
    Manifest::from_toml(GUARD)
        .iter()
        .map(Installed::of)
        .collect()
}

/// What waits, where the settings hold what `recorded` lists and the secrets `held`
/// names hold a value.
fn waits(recorded: &[(&str, &str)], held: &[&str]) -> Vec<Waiting> {
    let ctx = crate::test_support::a_context().build();
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let Some(manifest) = manifest else {
        unreachable!("the stack reads");
    };
    let recorded: BTreeMap<&str, &str> = recorded.iter().copied().collect();
    waiting(
        &manifest,
        &guard(),
        &|name| recorded.get(name).map(|value| (*value).to_owned()),
        &|name| held.contains(&name),
    )
}

fn tunnel(wants: Vec<Wanted>) -> Vec<Waiting> {
    vec![Waiting {
        service: "tunnel".to_owned(),
        wants,
    }]
}

#[test]
fn a_service_waits_for_what_only_the_operator_holds_where_its_needs_are_met() {
    assert_eq!(guard().len(), 1, "the plugin reads");
    assert_eq!(
        waits(&[], &[]),
        tunnel(vec![
            Wanted::Setting {
                setting: "guard:account".to_owned(),
                choices: None,
            },
            Wanted::Secret {
                secret: "guard:wireguard-key".to_owned(),
            },
        ])
    );
    assert_eq!(
        waits(&[("guard:vpn-type", "openvpn")], &["guard:wireguard-key"]),
        tunnel(vec![
            Wanted::Setting {
                setting: "guard:account".to_owned(),
                choices: None,
            },
            Wanted::Secret {
                secret: "guard:openvpn-user".to_owned(),
            },
        ])
    );
    assert_eq!(
        waits(
            &[("guard:vpn-type", "openvpn"), ("guard:account", "me")],
            &["guard:openvpn-user"]
        ),
        Vec::new()
    );
}

#[test]
fn a_setting_holding_a_value_outside_its_choices_is_waited_for_with_them() {
    assert_eq!(
        waits(&[("guard:vpn-type", "ipsec"), ("guard:account", "me")], &[]),
        tunnel(vec![Wanted::Setting {
            setting: "guard:vpn-type".to_owned(),
            choices: Some(vec!["wireguard".to_owned(), "openvpn".to_owned()]),
        }])
    );
}

#[test]
fn a_service_running_through_one_held_back_is_held_back_waiting_for_it() {
    let placed = |pairs: &[(&str, &str)]| crate::topology::Topology {
        placed: pairs
            .iter()
            .map(|(service, host)| {
                (
                    (*service).to_owned(),
                    crate::topology::Placed::Inside((*host).to_owned()),
                )
            })
            .chain([("alone".to_owned(), crate::topology::Placed::Nowhere)])
            .collect(),
        ..crate::topology::Topology::default()
    };
    let held = |service: &str, host: &str| Waiting {
        service: service.to_owned(),
        wants: vec![Wanted::Service {
            service: host.to_owned(),
        }],
    };
    let tunnel = tunnel(vec![Wanted::Secret {
        secret: "guard:wireguard-key".to_owned(),
    }]);
    let mut found = super::super::waiting::through(
        tunnel.clone(),
        &placed(&[
            ("torrents", "tunnel"),
            ("seeding", "torrents"),
            ("free", "elsewhere"),
        ]),
    );
    found.sort_by(|one, other| one.service.cmp(&other.service));
    let mut expected = tunnel.clone();
    expected.extend([held("seeding", "torrents"), held("torrents", "tunnel")]);
    expected.sort_by(|one, other| one.service.cmp(&other.service));
    assert_eq!(found, expected);
    assert_eq!(
        super::super::waiting::through(Vec::new(), &placed(&[("torrents", "tunnel")])),
        Vec::new()
    );
}

#[tokio::test]
async fn status_names_each_service_held_back_beside_the_services_and_not_among_them() {
    let machine = super::changing::holding("waiting-status", super::changing::running(&[]), GUARD);
    let report = crate::app::dispatch(
        crate::app::Command::Status { forms: Vec::new() },
        &machine.ctx,
    )
    .await;
    let Ok(crate::app::Outcome::Status(report)) = report else {
        unreachable!("the status reads");
    };
    assert_eq!(
        report.waiting,
        tunnel(vec![
            Wanted::Setting {
                setting: "guard:account".to_owned(),
                choices: None,
            },
            Wanted::Secret {
                secret: "guard:wireguard-key".to_owned(),
            },
        ])
    );
    assert!(
        !report.services.iter().any(|service| service.id == "tunnel"),
        "{:?}",
        report.services
    );
}

#[tokio::test]
async fn a_settings_change_names_what_waits_once_it_is_made() {
    let machine = super::changing::holding("waiting-change", super::changing::running(&[]), GUARD);
    let change = super::changing::change(
        super::changing::asking(
            &machine.ctx,
            &[
                ("guard:vpn-type", Some("openvpn")),
                ("guard:account", Some("me")),
            ],
            None,
        )
        .await,
    );
    assert_eq!(
        change.map(|one| one.waiting),
        Some(tunnel(vec![Wanted::Secret {
            secret: "guard:openvpn-user".to_owned(),
        }]))
    );
}

/// The profile the project written for `machine` gives the guard's service.
fn profile(machine: &super::changing::Machine) -> Option<String> {
    let text = std::fs::read_to_string(&machine.document).unwrap_or_default();
    serde_norway::from_str::<serde_norway::Value>(&text)
        .ok()?
        .get("services")?
        .get("tunnel")?
        .get("profiles")?
        .get(0)?
        .as_str()
        .map(str::to_owned)
}

#[tokio::test]
async fn a_service_held_back_is_written_apart_and_started_by_the_act_giving_its_last_need() {
    let machine =
        super::changing::holding("waiting-released", super::changing::running(&[]), GUARD);
    assert_eq!(profile(&machine).as_deref(), Some(crate::project::WAITING));
    let at = crate::settings::secrets_at(&machine.ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name: "guard:wireguard-key",
        value: "a-key",
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());

    let given = [("guard:account", Some("me"))];
    let offer = super::changing::change(super::changing::asking(&machine.ctx, &given, None).await)
        .map(|one| one.agreement)
        .unwrap_or_default();
    let made =
        super::changing::change(super::changing::asking(&machine.ctx, &given, Some(&offer)).await);
    assert_eq!(made.as_ref().map(|one| one.applied), Some(true));
    assert_eq!(
        made.as_ref().map(|one| one.starts.clone()),
        Some(vec!["tunnel".to_owned()])
    );
    assert_eq!(made.map(|one| one.waiting), Some(Vec::new()));
    assert_ne!(profile(&machine).as_deref(), Some(crate::project::WAITING));
    assert!(machine
        .runner
        .seen()
        .iter()
        .any(|argv| argv.iter().any(|word| word == "up")
            && argv.last().map(String::as_str) == Some("tunnel")));
}

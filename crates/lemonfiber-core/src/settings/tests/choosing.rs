use super::changing::{asking, change, code, holding, journal, moved, recorded, running, Machine};
use crate::journal::Kind;

const TYPE: &str = "guard:vpn-type";
const PROVIDER: &str = "guard:provider";

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
VPN_TYPE = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
PROVIDER = { setting = "guard:provider", default = "mullvad", one_of = [
    "mullvad",
    { values = ["pia"], needs = { "guard:vpn-type" = ["openvpn"] } },
] }
FORWARDING = { setting = "guard:forwarding", default = "off", one_of = [
    { values = ["off", "on"], needs = { "guard:vpn-type" = ["openvpn"] } },
] }
PORT = { setting = "guard:port", default = "0", needs = { "guard:vpn-type" = ["openvpn", "custom"] } }
"#;

fn guard(name: &str) -> Machine {
    holding(&format!("choosing-{name}"), running(&[]), GUARD)
}

/// What one act asking for `changing` comes to once its offer is answered.
async fn agreed(
    machine: &Machine,
    changing: &[(&str, Option<&str>)],
) -> Result<crate::model::SettingsReport, Box<crate::error::Problem>> {
    let offer = change(asking(&machine.ctx, changing, None).await)
        .map(|one| one.agreement)
        .unwrap_or_default();
    asking(&machine.ctx, changing, Some(&offer)).await
}

fn said(report: &Result<crate::model::SettingsReport, Box<crate::error::Problem>>) -> String {
    report
        .as_ref()
        .err()
        .map(|problem| format!("{} {}", problem.summary, problem.meaning))
        .unwrap_or_default()
}

#[tokio::test]
async fn a_value_outside_its_settings_choices_is_refused_naming_them_and_nothing_moves() {
    let machine = guard("outside");
    let before = std::fs::read_to_string(&machine.document).ok();

    let provider = asking(&machine.ctx, &[(PROVIDER, Some("pia"))], None).await;
    assert_eq!(code(&provider), Some("SETTING-7"));
    assert!(
        said(&provider).contains("guard:provider cannot hold pia; its choices are mullvad"),
        "{}",
        said(&provider)
    );
    let kind = asking(&machine.ctx, &[(TYPE, Some("ipsec"))], None).await;
    assert!(
        said(&kind)
            .contains("guard:vpn-type cannot hold ipsec; its choices are wireguard, openvpn"),
        "{}",
        said(&kind)
    );
    let forwarding = asking(&machine.ctx, &[("guard:forwarding", Some("on"))], None).await;
    assert!(
        said(&forwarding).contains(
            "guard:forwarding cannot hold on; its choices are none, given what the settings it \
             depends on hold"
        ),
        "{}",
        said(&forwarding)
    );

    assert_eq!(recorded(&machine, PROVIDER), None);
    assert!(journal(&machine.ctx).is_empty());
    assert_eq!(std::fs::read_to_string(&machine.document).ok(), before);
}

#[tokio::test]
async fn settings_whose_choices_hang_on_each_other_move_together_in_one_journalled_act() {
    let machine = guard("together");
    let both = [(TYPE, Some("openvpn")), (PROVIDER, Some("pia"))];

    let made = change(agreed(&machine, &both).await);
    assert_eq!(made.as_ref().map(|one| one.applied), Some(true));
    assert_eq!(
        made.as_ref().map(moved),
        Some(vec![
            (PROVIDER, None, Some("pia")),
            (TYPE, None, Some("openvpn"))
        ])
    );
    assert_eq!(
        made.map(|one| one.regenerates),
        Some(vec!["tunnel".to_owned()])
    );
    assert_eq!(recorded(&machine, TYPE).as_deref(), Some("openvpn"));
    assert_eq!(recorded(&machine, PROVIDER).as_deref(), Some("pia"));
    let written = std::fs::read_to_string(&machine.document).unwrap_or_default();
    assert!(written.contains("VPN_TYPE: openvpn"), "{written}");
    assert!(written.contains("PROVIDER: pia"), "{written}");
    let settings: Vec<String> = journal(&machine.ctx)
        .into_iter()
        .filter_map(|one| match one.kind {
            Kind::Setting { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    assert_eq!(settings, [PROVIDER, TYPE]);
}

#[tokio::test]
async fn an_act_leaving_another_settings_recorded_value_outside_its_choices_is_refused() {
    let machine = guard("leaving");
    let both = [(TYPE, Some("openvpn")), (PROVIDER, Some("pia"))];
    assert!(agreed(&machine, &both).await.is_ok());

    for leaving in [[(TYPE, Some("wireguard"))], [(TYPE, None)]] {
        let refused = asking(&machine.ctx, &leaving, None).await;
        assert_eq!(code(&refused), Some("SETTING-7"), "{leaving:?}");
        assert!(
            said(&refused).contains("guard:provider cannot hold pia; its choices are mullvad"),
            "{}",
            said(&refused)
        );
    }
    let cleared = agreed(&machine, &[(TYPE, None), (PROVIDER, None)]).await;
    assert_eq!(
        change(cleared).as_ref().map(moved),
        Some(vec![
            (PROVIDER, Some("pia"), None),
            (TYPE, Some("openvpn"), None)
        ])
    );
    assert_eq!(recorded(&machine, TYPE), None);
    assert_eq!(recorded(&machine, PROVIDER), None);
}

#[tokio::test]
async fn only_the_settings_that_move_are_changed_and_none_moving_changes_nothing() {
    let machine = guard("already");
    assert!(agreed(&machine, &[(TYPE, Some("openvpn"))]).await.is_ok());

    let one = change(
        asking(
            &machine.ctx,
            &[(TYPE, Some("openvpn")), (PROVIDER, Some("pia"))],
            None,
        )
        .await,
    );
    assert_eq!(
        one.as_ref().map(moved),
        Some(vec![(PROVIDER, None, Some("pia"))])
    );

    let none = asking(
        &machine.ctx,
        &[(TYPE, Some("openvpn")), (PROVIDER, None)],
        None,
    )
    .await;
    assert_eq!(code(&none), Some("SETTING-6"));
    assert!(
        said(&none).starts_with("guard:provider and guard:vpn-type already hold that"),
        "{}",
        said(&none)
    );
    let cleared = asking(
        &machine.ctx,
        &[(PROVIDER, None), ("guard:other", None)],
        None,
    )
    .await;
    assert_eq!(code(&cleared), Some("SETTING-1"), "{}", said(&cleared));
}

#[tokio::test]
async fn the_settings_read_carries_each_choice_with_its_needs_and_each_reference_with_its_own() {
    let machine = guard("read");
    let read = |name: &str| {
        crate::settings::run::listing(&machine.ctx)
            .ok()
            .and_then(|report| report.settings.into_iter().find(|one| one.name == name))
    };
    let provider = |open: bool| {
        Some(vec![
            crate::model::Choosable {
                value: "mullvad".to_owned(),
                needs: std::collections::BTreeMap::new(),
                open: true,
            },
            crate::model::Choosable {
                value: "pia".to_owned(),
                needs: [(TYPE.to_owned(), vec!["openvpn".to_owned()])].into(),
                open,
            },
        ])
    };
    assert_eq!(read(PROVIDER).and_then(|one| one.choices), provider(false));
    let port = |met: bool| {
        Some(vec![(
            [(
                TYPE.to_owned(),
                vec!["openvpn".to_owned(), "custom".to_owned()],
            )]
            .into(),
            met,
        )])
    };
    let needing = |name: &str| {
        read(name).map(|one| {
            one.readers
                .into_iter()
                .map(|reader| (reader.needs, reader.met))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(needing("guard:port"), port(false));
    assert_eq!(read("guard:port").and_then(|one| one.choices), None);

    assert!(agreed(&machine, &[(TYPE, Some("openvpn"))]).await.is_ok());
    assert_eq!(read(PROVIDER).and_then(|one| one.choices), provider(true));
    assert_eq!(needing("guard:port"), port(true));
}

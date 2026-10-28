use std::collections::BTreeMap;

use super::{broken, Choice, Reading};
use crate::container::values::{Constant, Secret, Value};

/// An environment read from TOML.
fn env(text: &str) -> BTreeMap<String, Value> {
    toml::from_str(text).unwrap_or_default()
}

/// A secret read from TOML.
fn secret(text: &str) -> Option<Secret> {
    toml::from_str(text).ok()
}

/// What `services` break, each as where and what.
fn wrong(services: &[Reading<'_>]) -> Vec<(String, String)> {
    broken(services)
}

const GUARD: &str = r#"
VPN_TYPE = { setting = "gluetun:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
VPN_SERVICE_PROVIDER = { setting = "gluetun:provider", default = "mullvad", one_of = [
    "mullvad",
    { values = ["private internet access"], needs = { "gluetun:vpn-type" = ["openvpn"] } },
] }
WIREGUARD_PRIVATE_KEY = { setting = "gluetun:wireguard-key", default = "", recorded_with = "minted", needs = { "gluetun:vpn-type" = ["wireguard"] } }
"#;

fn reading<'a>(env: &'a BTreeMap<String, Value>, secrets: &'a [&'a Secret]) -> Reading<'a> {
    Reading {
        service: "gluetun",
        env,
        namespace: "gluetun",
        secrets,
    }
}

#[test]
fn choices_and_needs_declared_as_the_rules_ask_break_nothing() {
    let env = env(GUARD);
    let user = secret(
        r#"
id = "openvpn-user"
of = "gluetun"
why = "Signs in to the provider over OpenVPN."
needs = { "gluetun:vpn-type" = ["openvpn"] }
"#,
    );
    let Some(user) = user else {
        unreachable!("the secret reads");
    };
    let held = [&user];
    assert_eq!(wrong(&[reading(&env, &held)]), Vec::new());
    let provider = env.get("VPN_SERVICE_PROVIDER");
    let Some(Value::Setting(provider)) = provider else {
        unreachable!("the provider is a setting");
    };
    let listed: Vec<Vec<String>> = provider
        .one_of
        .iter()
        .flatten()
        .map(|choice| choice.values().iter().map(Constant::text).collect())
        .collect();
    assert_eq!(
        listed,
        vec![
            vec!["mullvad".to_owned()],
            vec!["private internet access".to_owned()]
        ]
    );
    assert!(provider.one_of.iter().flatten().any(|choice| choice
        .needs()
        .is_some_and(|needs| needs.contains_key("gluetun:vpn-type"))));
    assert!(matches!(
        provider.one_of.as_deref().and_then(<[Choice]>::first),
        Some(Choice::Always(_))
    ));
}

#[test]
fn choices_declared_empty_twice_or_on_two_references_are_refused_by_name() {
    let env = env(r#"
A = { setting = "gluetun:vpn-type", default = "wireguard", one_of = [] }
B = { setting = "gluetun:provider", default = "mullvad", one_of = ["mullvad", { values = ["mullvad"], needs = {} }] }
C = { setting = "gluetun:provider", default = "mullvad", one_of = ["mullvad"] }
"#);
    let mut found = wrong(&[reading(&env, &[])]);
    found.sort();
    assert_eq!(
        found,
        vec![
            (
                "service gluetun.env.A.default".to_owned(),
                "wireguard is not one of setting gluetun:vpn-type's values: ".to_owned()
            ),
            (
                "service gluetun.env.A.one_of".to_owned(),
                "lists no value setting gluetun:vpn-type accepts".to_owned()
            ),
            (
                "service gluetun.env.B.one_of".to_owned(),
                "lists mullvad twice among setting gluetun:provider's values".to_owned()
            ),
            (
                "service gluetun.env.C.one_of".to_owned(),
                "declares the choices of setting gluetun:provider, which service gluetun.env.B \
                 declares already"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn a_default_outside_its_settings_values_is_refused_unless_only_the_operator_holds_it() {
    let env = env(r#"
A = { setting = "gluetun:vpn-type", default = "ipsec", one_of = ["wireguard", "openvpn"] }
B = { setting = "gluetun:vpn-type", default = "", recorded_with = "openvpn" }
"#);
    assert_eq!(
        wrong(&[reading(&env, &[])]),
        vec![(
            "service gluetun.env.A.default".to_owned(),
            "ipsec is not one of setting gluetun:vpn-type's values: wireguard, openvpn".to_owned()
        )]
    );
}

#[test]
fn needs_name_only_another_setting_the_service_reads_and_only_its_values() {
    let env = env(r#"
A = { setting = "gluetun:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
B = { setting = "gluetun:provider", default = "mullvad", needs = { "gluetun:provider" = ["mullvad"], "gluetun:elsewhere" = ["x"], "gluetun:vpn-type" = ["ipsec", "openvpn"], "gluetun:region" = ["anywhere"] } }
C = { setting = "gluetun:region", default = "europe" }
"#);
    let stray = secret(
        r#"
id = "openvpn-user"
of = "gluetun"
why = "Signs in to the provider over OpenVPN."
needs = { "gluetun:nothing" = ["x"] }
"#,
    );
    let Some(stray) = stray else {
        unreachable!("the secret reads");
    };
    let held = [&stray];
    assert_eq!(
        wrong(&[reading(&env, &held)]),
        vec![
            (
                "service gluetun.env.B.needs".to_owned(),
                "names setting gluetun:elsewhere, which service gluetun does not read".to_owned()
            ),
            (
                "service gluetun.env.B.needs".to_owned(),
                "names setting gluetun:provider, which it reads itself".to_owned()
            ),
            (
                "service gluetun.env.B.needs".to_owned(),
                "lists ipsec for setting gluetun:vpn-type, which is not one of its values: \
                 wireguard, openvpn"
                    .to_owned()
            ),
            (
                "secret gluetun:openvpn-user.needs".to_owned(),
                "names setting gluetun:nothing, which service gluetun does not read".to_owned()
            ),
        ]
    );
}

#[test]
fn a_value_needing_another_names_only_what_the_service_reads() {
    let env = env(r#"
A = { setting = "gluetun:provider", default = "mullvad", one_of = ["mullvad", { values = ["pia"], needs = { "gluetun:vpn-type" = ["openvpn"] } }] }
"#);
    assert_eq!(
        wrong(&[reading(&env, &[])]),
        vec![(
            "service gluetun.env.A.one_of".to_owned(),
            "names setting gluetun:vpn-type, which service gluetun does not read".to_owned()
        )]
    );
}

#[test]
fn needs_are_met_where_every_setting_holds_a_listed_value_the_default_where_none_is_recorded() {
    let env = env(GUARD);
    let key = env.get("WIREGUARD_PRIVATE_KEY");
    let Some(Value::Setting(key)) = key else {
        unreachable!("the key is a setting");
    };
    let defaulted = |setting: &str| super::defaulted(&env, setting);
    assert_eq!(defaulted("gluetun:vpn-type").as_deref(), Some("wireguard"));
    assert_eq!(defaulted("gluetun:nothing"), None);
    assert!(super::met(&key.needs, &defaulted));
    assert!(!super::met(&key.needs, &|setting: &str| {
        (setting == "gluetun:vpn-type").then(|| "openvpn".to_owned())
    }));
    assert!(!super::met(&key.needs, &|_: &str| None));
    assert!(super::met(&super::Needs::new(), &|_: &str| None));
}

#[test]
fn a_settings_choices_are_the_values_whose_needs_the_other_settings_meet() {
    let env = env(GUARD);
    let wireguard = |setting: &str| super::defaulted(&env, setting);
    assert_eq!(
        super::offered(&env, "gluetun:provider", &wireguard),
        Some(vec!["mullvad".to_owned()])
    );
    let openvpn = |setting: &str| (setting == "gluetun:vpn-type").then(|| "openvpn".to_owned());
    assert_eq!(
        super::offered(&env, "gluetun:provider", &openvpn),
        Some(vec![
            "mullvad".to_owned(),
            "private internet access".to_owned()
        ])
    );
    assert_eq!(
        super::offered(&env, "gluetun:wireguard-key", &openvpn),
        None
    );
    assert_eq!(super::offered(&env, "gluetun:nothing", &openvpn), None);
}

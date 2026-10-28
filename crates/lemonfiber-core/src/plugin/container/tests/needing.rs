use lemonfiber_manifest::container::{Fields, Value};

use super::super::super::installed::Placed;
use super::{installed, placed, written};
use crate::project::Reading;

fn environment(setting: &dyn Fn(&str, &str) -> Option<String>) -> Vec<(String, Option<String>)> {
    let env: std::collections::BTreeMap<String, Value> = toml::from_str(
        r#"
VPN_TYPE = { setting = "komga:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
WIREGUARD_ADDRESSES = { setting = "komga:addresses", default = "10.0.0.2/32", needs = { "komga:vpn-type" = ["wireguard"] } }
OPENVPN_PROTOCOL = { setting = "komga:protocol", default = "udp", needs = { "komga:vpn-type" = ["openvpn"] } }
"#,
    )
    .unwrap_or_default();
    let reading = Reading {
        setting,
        ..Reading::bare()
    };
    let document = written(
        &installed(vec![Placed {
            fields: Fields {
                env,
                ..Fields::default()
            },
            ..placed()
        }]),
        &reading,
    );
    let entry = serde_norway::from_str::<serde_norway::Value>(&document)
        .ok()
        .and_then(|document| document.get("services")?.get("komga").cloned())
        .unwrap_or_default();
    ["VPN_TYPE", "WIREGUARD_ADDRESSES", "OPENVPN_PROTOCOL"]
        .into_iter()
        .map(|name| {
            (
                name.to_owned(),
                entry
                    .get("environment")
                    .and_then(|environment| environment.get(name))
                    .and_then(serde_norway::Value::as_str)
                    .map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn a_variable_is_written_only_where_the_settings_it_needs_hold_what_it_lists() {
    let unset = |_: &str, _: &str| None;
    assert_eq!(
        environment(&unset),
        vec![
            ("VPN_TYPE".to_owned(), Some("wireguard".to_owned())),
            (
                "WIREGUARD_ADDRESSES".to_owned(),
                Some("10.0.0.2/32".to_owned())
            ),
            ("OPENVPN_PROTOCOL".to_owned(), None),
        ]
    );
    let openvpn = |_: &str, name: &str| (name == "komga:vpn-type").then(|| "openvpn".to_owned());
    assert_eq!(
        environment(&openvpn),
        vec![
            ("VPN_TYPE".to_owned(), Some("openvpn".to_owned())),
            ("WIREGUARD_ADDRESSES".to_owned(), None),
            ("OPENVPN_PROTOCOL".to_owned(), Some("udp".to_owned())),
        ]
    );
}

#[test]
fn a_secret_is_written_only_where_the_settings_it_needs_hold_what_it_lists() {
    let env: std::collections::BTreeMap<String, Value> = toml::from_str(
        r#"
VPN_TYPE = { setting = "komga:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
OPENVPN_USER = { secret = "openvpn-user" }
"#,
    )
    .unwrap_or_default();
    let placed = Placed {
        fields: Fields {
            env,
            ..Fields::default()
        },
        ..placed()
    };
    let mut one = installed(vec![placed.clone()]);
    one.declared.secrets = vec![crate::plugin::Secret {
        id: "openvpn-user".to_owned(),
        of: "komga".to_owned(),
        why: "Signs in to the provider over OpenVPN.".to_owned(),
        minted: false,
        serves: None,
        was: None,
        needs: toml::from_str(r#""komga:vpn-type" = ["openvpn"]"#).unwrap_or_default(),
        needed_to_start: false,
    }];
    let named = |setting: &dyn Fn(&str, &str) -> Option<String>| -> Vec<String> {
        let reading = super::super::Reading {
            setting,
            ..super::super::Reading::bare()
        };
        super::super::variables(&one, &placed, &reading)
            .into_iter()
            .map(|variable| variable.name)
            .collect()
    };
    assert!(named(&|_, _| None).is_empty());
    assert_eq!(
        named(&|_, name| (name == "komga:vpn-type").then(|| "openvpn".to_owned())),
        ["OPENVPN_USER"]
    );
}

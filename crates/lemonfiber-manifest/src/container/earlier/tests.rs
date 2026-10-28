use std::collections::BTreeMap;

use super::super::values::{Constant, Secret, Setting, Value};
use super::{broken, clashing, distinct, of, Earlier};

fn reading(setting: &str, was: Option<&str>) -> Value {
    Value::Setting(Setting {
        setting: setting.to_owned(),
        default: Constant::Text(String::new()),
        recorded_with: None,
        was: was.map(str::to_owned),
        needs: crate::container::choices::Needs::new(),
        one_of: None,
    })
}

fn held(id: &str, was: Option<&str>) -> Secret {
    Secret {
        id: id.to_owned(),
        of: "gluetun".to_owned(),
        why: "Dials the tunnel".to_owned(),
        minted: super::super::values::Minted::Not,
        serves: None,
        carried_from: None,
        recorded_with: None,
        was: was.map(str::to_owned),
        needs: crate::container::choices::Needs::new(),
    }
}

fn kept(key: &str, name: &str, secret: bool) -> Earlier {
    Earlier {
        key: key.to_owned(),
        name: name.to_owned(),
        secret,
    }
}

#[test]
fn every_setting_and_secret_naming_a_variable_is_read_once() {
    let env: BTreeMap<String, Value> = [
        (
            "A".to_owned(),
            reading("gluetun:provider", Some("VPN_PROVIDER")),
        ),
        (
            "B".to_owned(),
            reading("gluetun:provider", Some("VPN_PROVIDER")),
        ),
        ("C".to_owned(), reading("gluetun:countries", None)),
        (
            "D".to_owned(),
            Value::Constant(Constant::Text("x".to_owned())),
        ),
    ]
    .into_iter()
    .collect();
    let secrets = [
        held("wireguard-key", Some("WIREGUARD_PRIVATE_KEY")),
        held("other", None),
    ];
    assert_eq!(
        of([&env], "gluetun", &secrets),
        vec![
            kept("VPN_PROVIDER", "gluetun:provider", false),
            kept("WIREGUARD_PRIVATE_KEY", "gluetun:wireguard-key", true),
        ]
    );
}

#[test]
fn a_variable_two_name_or_no_variables_name_is_refused_naming_both() {
    let named = distinct([
        kept("VPN_PROVIDER", "gluetun:provider", false),
        kept("VPN_PROVIDER", "gluetun:provider", false),
        kept("VPN_PROVIDER", "gluetun:key", true),
        kept("vpn-type", "gluetun:type", false),
    ]);
    assert_eq!(
        broken(&named),
        vec![
            (
                "setting gluetun:type.was".to_owned(),
                "vpn-type is not upper-case letters, digits and \"_\", starting with a letter or \
                 \"_\""
                    .to_owned()
            ),
            (
                "secret gluetun:key.was".to_owned(),
                "VPN_PROVIDER is named by setting gluetun:provider and secret gluetun:key"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn a_variable_another_names_for_something_else_clashes_and_for_the_same_does_not() {
    let own = [
        kept("VPN_PROVIDER", "gluetun:provider", false),
        kept("UMASK", "unpackerr:umask", false),
    ];
    let others = [
        kept("VPN_PROVIDER", "gluetun:provider", false),
        kept("UMASK", "other:umask", false),
    ];
    let clashes: Vec<(String, String)> = clashing(&own, &others)
        .into_iter()
        .map(|(one, other)| (one.named(), other.named()))
        .collect();
    assert_eq!(
        clashes,
        vec![(
            "setting unpackerr:umask".to_owned(),
            "setting other:umask".to_owned()
        )]
    );
}

use std::collections::BTreeMap;

use lemonfiber_fixtures::ports::Chance;
use lemonfiber_manifest::container::{
    Constant, Minted, RecordedWith, Secret, SecretOf, Setting, Value,
};

use super::{named, StandIns, UNMADE};

fn secret(id: &str, recorded_with: Option<RecordedWith>) -> Secret {
    Secret {
        id: id.to_owned(),
        of: "gluetun".to_owned(),
        why: "Dial the tunnel".to_owned(),
        minted: Minted::Not,
        serves: None,
        carried_from: None,
        recorded_with,
    }
}

fn setting(name: &str, recorded_with: Option<RecordedWith>) -> Value {
    Value::Setting(Setting {
        setting: name.to_owned(),
        default: Constant::Text(String::new()),
        recorded_with,
    })
}

fn guard() -> crate::plugin::Placed {
    let mut placed = crate::test_support::a_placed("gluetun", &[], None, None);
    placed.fields.env = BTreeMap::from([
        (
            "PROVIDER".to_owned(),
            setting(
                "gluetun:provider",
                Some(RecordedWith::Constant(Constant::Text("custom".to_owned()))),
            ),
        ),
        (
            "PORT".to_owned(),
            setting(
                "gluetun:port",
                Some(RecordedWith::Constant(Constant::Number(51820))),
            ),
        ),
        (
            "PUBLIC_KEY".to_owned(),
            setting("gluetun:endpoint-key", Some(RecordedWith::Minted)),
        ),
        ("COUNTRIES".to_owned(), setting("gluetun:countries", None)),
        (
            "PRIVATE_KEY".to_owned(),
            Value::Secret(SecretOf {
                secret: "wireguard-key".to_owned(),
            }),
        ),
        (
            "ENTERED".to_owned(),
            Value::Secret(SecretOf {
                secret: "entered".to_owned(),
            }),
        ),
        ("TZ_NAME".to_owned(), Value::Constant(Constant::Flag(true))),
    ]);
    placed
}

fn secrets() -> Vec<Secret> {
    vec![
        secret("wireguard-key", Some(RecordedWith::Minted)),
        secret("entered", None),
    ]
}

#[test]
fn each_setting_declaring_a_stand_in_is_given_it_and_only_one_made_is_scrubbed() {
    let random = Chance::exactly(Some(vec![0xfb; 32]));
    let mut stand_ins = StandIns::of(&[], &random).unwrap_or_default();
    let env = stand_ins.settings(&guard(), &random);
    let made = "+/v7".repeat(10) + "+/s=";
    assert_eq!(
        env,
        Ok(vec![
            "PORT=51820".to_owned(),
            "PROVIDER=custom".to_owned(),
            format!("PUBLIC_KEY={made}"),
        ])
    );
    assert_eq!(stand_ins.made, vec![made]);
}

#[test]
fn each_secret_declaring_a_stand_in_is_given_it_wherever_the_service_names_it() {
    let random = Chance::exactly(Some(vec![0x00; 32]));
    let stand_ins = StandIns::of(&secrets(), &random).unwrap_or_default();
    let made = "A".repeat(43) + "=";
    assert_eq!(stand_ins.secret("wireguard-key"), Some(made.as_str()));
    assert_eq!(stand_ins.secret("entered"), None);
    assert_eq!(
        stand_ins.secrets(&guard()),
        vec![format!("PRIVATE_KEY={made}")]
    );
    assert_eq!(
        stand_ins.held().collect::<Vec<_>>(),
        vec![("wireguard-key", made.as_str())]
    );
    assert_eq!(stand_ins.made, vec![made]);
}

#[test]
fn a_stand_in_to_be_made_without_random_bytes_is_refused_and_a_constant_needs_none() {
    let none = Chance::exactly(None);
    assert_eq!(
        StandIns::of(&secrets(), &none).map(|_| ()),
        Err(UNMADE.to_owned())
    );
    assert_eq!(
        StandIns::default().settings(&guard(), &none),
        Err(UNMADE.to_owned())
    );
    let constant = [secret(
        "token",
        Some(RecordedWith::Constant(Constant::Text(
            "documented".to_owned(),
        ))),
    )];
    let stand_ins = StandIns::of(&constant, &none).unwrap_or_default();
    assert_eq!(stand_ins.secret("token"), Some("documented"));
    assert!(
        stand_ins.made.is_empty(),
        "a declared constant is not scrubbed"
    );
}

#[test]
fn the_variables_a_recording_stands_in_for_are_named_in_order() {
    assert_eq!(
        named(&guard(), &secrets()),
        vec!["PORT", "PRIVATE_KEY", "PROVIDER", "PUBLIC_KEY"]
    );
    assert_eq!(
        named(&guard(), &[]),
        vec!["PORT", "PROVIDER", "PUBLIC_KEY"],
        "a secret the plugin does not declare is not stood in for"
    );
}

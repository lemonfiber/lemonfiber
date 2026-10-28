use lemonfiber_core::model::{
    Choosable, Owner, Reader, SecretFrom, SecretRead, SettingChange, SettingMove, SettingRead,
    SettingsReport,
};

fn reader(service: &str, variable: &str, default: Option<&str>) -> Reader {
    Reader {
        service: service.to_owned(),
        variable: variable.to_owned(),
        default: default.map(str::to_owned),
        met: true,
        needs: std::collections::BTreeMap::new(),
    }
}

fn listing() -> SettingsReport {
    SettingsReport {
        settings: vec![
            SettingRead {
                name: "gluetun:countries".to_owned(),
                whose: Owner::Namespace {
                    namespace: "gluetun".to_owned(),
                },
                readers: vec![reader("gluetun", "SERVER_COUNTRIES", Some("Netherlands"))],
                value: Some("Belgium".to_owned()),
                environment: Some("VPN_COUNTRIES".to_owned()),
                choices: None,
            },
            SettingRead {
                name: "gluetun:provider".to_owned(),
                whose: Owner::Namespace {
                    namespace: "gluetun".to_owned(),
                },
                readers: vec![reader("gluetun", "VPN_SERVICE_PROVIDER", Some(""))],
                value: None,
                environment: None,
                choices: None,
            },
            SettingRead {
                name: "LEMONFIBER_DEVICE_KEY_APPS".to_owned(),
                whose: Owner::Core,
                readers: Vec::new(),
                value: None,
                environment: None,
                choices: None,
            },
        ],
        secrets: vec![SecretRead {
            name: "komga:token".to_owned(),
            whose: Owner::Namespace {
                namespace: "komga".to_owned(),
            },
            readers: vec![reader("komga", "KOMGA_TOKEN", None)],
            from: Some(SecretFrom::Recipe),
            held: true,
        }],
        change: None,
        rehearsed: false,
        unvetted: None,
        waiting: Vec::new(),
    }
}

fn change(value: Option<&str>, applied: bool) -> SettingChange {
    SettingChange {
        settings: vec![SettingMove {
            setting: "gluetun:countries".to_owned(),
            was: Some("Netherlands".to_owned()),
            value: value.map(str::to_owned),
            consequence: None,
        }],
        regenerates: vec!["gluetun".to_owned()],
        restarts: Vec::new(),
        applied,
        agreement: "1a2b3c4d-5e6f7a8b".to_owned(),
        unrestarted: None,
        waiting: Vec::new(),
        starts: Vec::new(),
    }
}

#[test]
fn the_listing_reads_each_setting_with_its_readers_and_each_secret_without_its_value() {
    let text = super::settings(&listing()).text();
    assert_eq!(
        text,
        "Settings the services read:\n\
         \n  gluetun:countries  set to Belgium  (gluetun's)\n\
         \x20   reads     gluetun as SERVER_COUNTRIES, Netherlands by default\n\
         \x20   held      in the environment file as VPN_COUNTRIES\n\
         \n  gluetun:provider  at its default  (gluetun's)\n\
         \x20   reads     gluetun as VPN_SERVICE_PROVIDER, empty by default\n\
         \n  LEMONFIBER_DEVICE_KEY_APPS  at its default  (the core's)\n\
         \x20   reads     nothing in the stack; the core reads it\n\
         \n\
         Secrets the services read, never shown:\n\
         \n  komga:token  held, captured by a recipe  (komga's)\n\
         \x20   reads     komga as KOMGA_TOKEN\n\
         \n\
         Change one with lemonfiber settings set <setting>=<value>; a secret is replaced \
         with lemonfiber credentials --rotate."
    );
}

#[test]
fn a_change_unanswered_names_what_it_regenerates_and_the_offer_to_answer() {
    let report = SettingsReport {
        change: Some(change(Some("Belgium, France"), false)),
        ..listing()
    };
    assert_eq!(
        super::settings(&report).text(),
        "gluetun:countries would hold Belgium, France.\n\
         \x20 was          Netherlands\n\
         \x20 regenerates  gluetun\n\
         \x20 restarts     nothing, as nothing running reads it\n\
         \n\
         Nothing has been changed. To make the change, answer this offer by name:\n\
         \x20 lemonfiber settings set \"gluetun:countries=Belgium, France\" --offer \
         1a2b3c4d-5e6f7a8b"
    );
}

#[test]
fn a_clear_made_says_the_default_is_written_again_and_what_was_not_restarted() {
    let mut made = change(None, true);
    made.restarts = vec!["gluetun".to_owned()];
    made.starts = vec!["torrents".to_owned()];
    made.unrestarted = Some("the engine is not running".to_owned());
    for one in &mut made.settings {
        one.consequence =
            Some("changing this affects which provider carries the torrents".to_owned());
    }
    let report = SettingsReport {
        change: Some(made),
        ..listing()
    };
    assert_eq!(
        super::settings(&report).text(),
        "gluetun:countries was cleared, so its default is written again.\n\
         \x20 was          Netherlands\n\
         \x20 regenerates  gluetun\n\
         \x20 restarts     gluetun\n\
         \x20 starts       torrents\n\
         \n\
         changing this affects which provider carries the torrents\n\
         \n\
         The change is made, and what reads it was not restarted: the engine is not running"
    );
}

#[test]
fn a_change_moving_several_settings_names_each_and_answers_them_together() {
    let mut several = change(Some("openvpn"), false);
    several.settings = vec![
        SettingMove {
            setting: "gluetun:provider".to_owned(),
            was: None,
            value: Some("private internet access".to_owned()),
            consequence: None,
        },
        SettingMove {
            setting: "gluetun:vpn-type".to_owned(),
            was: Some("wireguard".to_owned()),
            value: Some("openvpn".to_owned()),
            consequence: None,
        },
    ];
    let report = SettingsReport {
        change: Some(several),
        ..listing()
    };
    assert_eq!(
        super::settings(&report).text(),
        "gluetun:provider would hold private internet access.\n\
         \x20 was          at its default\n\
         \n\
         gluetun:vpn-type would hold openvpn.\n\
         \x20 was          wireguard\n\
         \n\
         \x20 regenerates  gluetun\n\
         \x20 restarts     nothing, as nothing running reads them\n\
         \n\
         Nothing has been changed. To make the change, answer this offer by name:\n\
         \x20 lemonfiber settings set \"gluetun:provider=private internet access\" \
         \"gluetun:vpn-type=openvpn\" --offer 1a2b3c4d-5e6f7a8b"
    );
}

#[test]
fn a_rehearsed_clear_says_it_was_a_rehearsal_and_names_the_clear() {
    let report = SettingsReport {
        change: Some(change(None, false)),
        rehearsed: true,
        ..listing()
    };
    let text = super::settings(&report).text();
    assert!(
        text.starts_with("gluetun:countries would be cleared"),
        "{text}"
    );
    assert!(
        text.contains("Nothing was written, because this was a rehearsal."),
        "{text}"
    );
    assert!(
        text.ends_with("  lemonfiber settings clear gluetun:countries --offer 1a2b3c4d-5e6f7a8b"),
        "{text}"
    );
}

#[test]
fn every_source_of_a_secret_is_said_and_a_stack_reading_none_says_so() {
    let secret = |name: &str, from: Option<SecretFrom>| SecretRead {
        name: name.to_owned(),
        whose: Owner::Namespace {
            namespace: "reader".to_owned(),
        },
        readers: Vec::new(),
        from,
        held: false,
    };
    let report = SettingsReport {
        secrets: vec![
            secret("reader:typed", Some(SecretFrom::Operator)),
            secret("reader:granted", Some(SecretFrom::Granted)),
            secret("reader:minted", Some(SecretFrom::Minted)),
            secret("reader:none", None),
        ],
        ..listing()
    };
    let text = super::settings(&report).text();
    assert!(
        text.contains("  reader:typed  not held, entered by you  (reader's)"),
        "{text}"
    );
    assert!(
        text.contains("  reader:granted  not held, another service's credential, granted"),
        "{text}"
    );
    assert!(
        text.contains("  reader:minted  not held, made by lemonfiber at install  (reader's)"),
        "{text}"
    );
    assert!(
        text.contains("  reader:none  not held  (reader's)"),
        "{text}"
    );
    let empty = SettingsReport {
        secrets: Vec::new(),
        ..listing()
    };
    assert!(super::settings(&empty)
        .text()
        .contains("  No service reads a secret."));
}

#[test]
fn a_set_made_says_what_it_now_holds() {
    let report = SettingsReport {
        change: Some(change(Some("Belgium"), true)),
        ..listing()
    };
    let text = super::settings(&report).text();
    assert!(
        text.starts_with("gluetun:countries now holds Belgium.\n"),
        "{text}"
    );
    assert!(!text.contains("--offer"), "{text}");
}

#[test]
fn a_change_names_the_operators_overlay_as_unvetted_where_there_is_one() {
    let overlay = lemonfiber_core::model::Unvetted {
        overlay: "/home/op/.config/lemonfiber/overlay.yml".to_owned(),
        services: vec!["gluetun".to_owned()],
    };
    let report = SettingsReport {
        change: Some(change(Some("Belgium"), false)),
        unvetted: Some(overlay.clone()),
        ..listing()
    };
    let said = super::settings(&report).text();
    assert!(
        said.ends_with(
            "\n\nUnvetted: your overlay /home/op/.config/lemonfiber/overlay.yml is merged over \
             the project and changes gluetun; nothing in it is held to what lemonfiber checks."
        ),
        "{said}"
    );
    let listed = super::settings(&SettingsReport {
        unvetted: Some(overlay),
        ..listing()
    })
    .text();
    assert!(!listed.contains("Unvetted"), "{listed}");
}

#[test]
fn a_setting_with_choices_lists_each_and_a_reference_says_what_it_is_written_beside() {
    let mut report = listing();
    let first = Reader {
        needs: [("gluetun:vpn-type".to_owned(), vec!["openvpn".to_owned()])].into(),
        met: false,
        ..reader("gluetun", "OPENVPN_PROTOCOL", Some("udp"))
    };
    let two = Reader {
        needs: [(
            "gluetun:vpn-type".to_owned(),
            vec!["openvpn".to_owned(), "custom".to_owned()],
        )]
        .into(),
        met: true,
        ..reader("gluetun", "PORT", Some("0"))
    };
    report.settings = vec![SettingRead {
        name: "gluetun:provider".to_owned(),
        whose: Owner::Namespace {
            namespace: "gluetun".to_owned(),
        },
        readers: vec![first, two],
        value: None,
        environment: None,
        choices: Some(vec![
            Choosable {
                value: "mullvad".to_owned(),
                needs: std::collections::BTreeMap::new(),
                open: true,
            },
            Choosable {
                value: "pia".to_owned(),
                needs: [("gluetun:vpn-type".to_owned(), vec!["openvpn".to_owned()])].into(),
                open: false,
            },
            Choosable {
                value: "proton".to_owned(),
                needs: [("gluetun:vpn-type".to_owned(), vec!["wireguard".to_owned()])].into(),
                open: true,
            },
        ]),
    }];
    let text = super::settings(&report).text();
    assert!(
        text.contains(
            "    reads     gluetun as OPENVPN_PROTOCOL, udp by default, written where \
             gluetun:vpn-type is openvpn, which is not so now\n"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "    reads     gluetun as PORT, 0 by default, written where gluetun:vpn-type is one \
             of openvpn, custom\n"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "    choices   mullvad, pia (where gluetun:vpn-type is openvpn; not now), proton \
             (where gluetun:vpn-type is wireguard)"
        ),
        "{text}"
    );
}

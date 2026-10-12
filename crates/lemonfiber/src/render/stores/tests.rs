use lemonfiber_core::model::{
    Owner, Reader, SecretFrom, SecretRead, SettingChange, SettingRead, SettingsReport,
};

fn reader(service: &str, variable: &str, default: Option<&str>) -> Reader {
    Reader {
        service: service.to_owned(),
        variable: variable.to_owned(),
        default: default.map(str::to_owned),
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
            },
            SettingRead {
                name: "gluetun:provider".to_owned(),
                whose: Owner::Namespace {
                    namespace: "gluetun".to_owned(),
                },
                readers: vec![reader("gluetun", "VPN_SERVICE_PROVIDER", Some(""))],
                value: None,
                environment: None,
            },
            SettingRead {
                name: "LEMONFIBER_DEVICE_KEY_APPS".to_owned(),
                whose: Owner::Core,
                readers: Vec::new(),
                value: None,
                environment: None,
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
    }
}

fn change(value: Option<&str>, applied: bool) -> SettingChange {
    SettingChange {
        setting: "gluetun:countries".to_owned(),
        was: Some("Netherlands".to_owned()),
        value: value.map(str::to_owned),
        regenerates: vec!["gluetun".to_owned()],
        restarts: Vec::new(),
        consequence: None,
        applied,
        agreement: "1a2b3c4d-5e6f7a8b".to_owned(),
        unrestarted: None,
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
         Change one with lemonfiber settings set <setting> <value>; a secret is replaced \
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
         \x20 lemonfiber settings set gluetun:countries \"Belgium, France\" --offer \
         1a2b3c4d-5e6f7a8b"
    );
}

#[test]
fn a_clear_made_says_the_default_is_written_again_and_what_was_not_restarted() {
    let mut made = change(None, true);
    made.restarts = vec!["gluetun".to_owned()];
    made.unrestarted = Some("the engine is not running".to_owned());
    made.consequence = Some("changing this affects which provider carries the torrents".to_owned());
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
         \n\
         changing this affects which provider carries the torrents\n\
         \n\
         The change is made, and what reads it was not restarted: the engine is not running"
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

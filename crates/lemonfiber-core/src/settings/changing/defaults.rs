use std::collections::BTreeMap;

use super::{fallback, offer, OFFERED};
use crate::model::{Reader, ReaderDefault, SettingDefault, SettingMove};
use crate::settings::readers::Readers;

fn reader(service: &str, default: Option<&str>, met: bool) -> Reader {
    Reader {
        service: service.to_owned(),
        variable: "MODE".to_owned(),
        default: default.map(str::to_owned),
        needs: BTreeMap::new(),
        met,
    }
}

fn read_by(readers: Vec<Reader>) -> Readers {
    Readers {
        settings: BTreeMap::from([("comics:mode".to_owned(), readers)]),
        ..Readers::default()
    }
}

#[test]
fn readers_that_agree_declare_one_default() {
    let readers = read_by(vec![
        reader("komga", Some("slow"), true),
        reader("worker", Some("slow"), true),
        reader("idle", Some("fast"), false),
    ]);

    assert_eq!(
        fallback("comics:mode", &readers),
        SettingDefault::Declared {
            value: "slow".to_owned()
        }
    );
}

#[test]
fn readers_that_differ_are_each_named_with_whether_they_are_met() {
    let readers = read_by(vec![
        reader("komga", Some("slow"), true),
        reader("worker", Some(""), true),
        reader("idle", None, false),
    ]);

    let SettingDefault::PerReader { readers } = fallback("comics:mode", &readers) else {
        unreachable!("readers declaring different defaults were not listed one by one");
    };
    assert_eq!(
        readers,
        vec![
            ReaderDefault {
                service: "komga".to_owned(),
                variable: "MODE".to_owned(),
                default: Some("slow".to_owned()),
                met: true,
            },
            ReaderDefault {
                service: "worker".to_owned(),
                variable: "MODE".to_owned(),
                default: Some(String::new()),
                met: true,
            },
            ReaderDefault {
                service: "idle".to_owned(),
                variable: "MODE".to_owned(),
                default: None,
                met: false,
            },
        ]
    );
}

#[test]
fn with_nothing_declared_or_nobody_met_there_is_no_default() {
    for readers in [
        Vec::new(),
        vec![reader("komga", None, true)],
        vec![reader("komga", Some("slow"), false)],
    ] {
        assert_eq!(
            fallback("comics:mode", &read_by(readers)),
            SettingDefault::Nothing
        );
    }
}

#[test]
fn a_core_setting_falls_back_to_the_cores_own_default() {
    let Some((name, default)) = crate::settings::own::CORE.iter().find_map(|(name, _)| {
        crate::settings::own::unset(name)
            .and_then(crate::settings::own::Unset::written)
            .map(|default| (*name, default))
    }) else {
        unreachable!("no core setting declares a default");
    };

    assert_eq!(
        fallback(name, &Readers::default()),
        SettingDefault::Core { value: default }
    );
}

#[test]
fn a_default_that_moved_moves_the_offer_and_is_named() {
    let moving = |value: &str| SettingMove {
        setting: "comics:mode".to_owned(),
        was: None,
        value: None,
        consequence: None,
        default: SettingDefault::Declared {
            value: value.to_owned(),
        },
    };
    let read = offer(&[moving("slow")], &[], &[], &[]);
    let now = offer(&[moving("fast")], &[], &[], &[]);

    assert_eq!(
        crate::agreement::differs(&read, &now, &OFFERED),
        vec!["the defaults it falls back to"]
    );
}

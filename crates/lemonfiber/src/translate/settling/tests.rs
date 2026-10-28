use clap::Parser as _;
use lemonfiber::cli::{Cli, Request};
use lemonfiber_core::app::{Command, Settling};

fn translated(args: &[&str]) -> Option<Command> {
    match Cli::try_parse_from(args).ok()?.command? {
        Request::Settings { change } => Some(super::settings(change)),
        _ => None,
    }
}

#[test]
fn the_bare_word_reads_and_the_two_verbs_under_it_carry_their_offer() {
    assert_eq!(
        translated(&["lemonfiber", "settings"]),
        Some(Command::Settings(Settling::Read))
    );
    assert_eq!(
        translated(&[
            "lemonfiber",
            "settings",
            "set",
            "gluetun:countries=Belgium, France",
            "gluetun:vpn-type=a=b",
            "--offer",
            "1a2b3c4d-5e6f7a8b",
        ]),
        Some(Command::Settings(Settling::Set {
            values: [
                ("gluetun:countries".to_owned(), "Belgium, France".to_owned()),
                ("gluetun:vpn-type".to_owned(), "a=b".to_owned()),
            ]
            .into(),
            agreement: Some("1a2b3c4d-5e6f7a8b".to_owned()),
        }))
    );
    assert_eq!(
        translated(&[
            "lemonfiber",
            "settings",
            "clear",
            "gluetun:countries",
            "gluetun:vpn-type"
        ]),
        Some(Command::Settings(Settling::Clear {
            settings: vec![
                "gluetun:countries".to_owned(),
                "gluetun:vpn-type".to_owned()
            ],
            agreement: None,
        }))
    );
    for refused in [
        &["lemonfiber", "settings", "set", "domain"][..],
        &["lemonfiber", "settings", "set", "=value"],
        &["lemonfiber", "settings", "set"],
        &["lemonfiber", "settings", "clear"],
    ] {
        assert_eq!(translated(refused), None, "{refused:?}");
    }
}

use lemonfiber::cli::{NotifyCommand, NotifyKind, NotifyRetries, NotifySecrets, RawNotify};
use lemonfiber_core::app::Command;
use lemonfiber_core::model::BackendKind;
use lemonfiber_core::notify::configuring::{Adding, Asked, Offering, Setting};

use super::notifying;

fn environment(name: &str) -> Option<String> {
    (name == "NTFY_TOKEN").then(|| "tk_secret".to_owned())
}

fn asked(action: NotifyCommand) -> Command {
    notifying(RawNotify { action }, &environment)
}

fn secrets(token: Option<&str>, reader: Option<&str>) -> NotifySecrets {
    NotifySecrets {
        token_env: token.map(str::to_owned),
        reader_env: reader.map(str::to_owned),
    }
}

#[test]
fn an_add_reads_its_secret_from_the_variable_named_and_an_unset_one_as_nothing() {
    let added = asked(NotifyCommand::Add {
        id: "home-ntfy".to_owned(),
        kind: NotifyKind::Ntfy,
        url: "http://ntfy.home:2586".to_owned(),
        topic: Some("stack".to_owned()),
        secrets: secrets(Some("NTFY_TOKEN"), Some("UNSET")),
        hosted: true,
        retries: NotifyRetries {
            attempts: Some(3),
            delays: Some(vec![1, 10]),
        },
    });

    assert_eq!(
        added,
        Command::Notify(Asked::Add(Box::new(Adding {
            id: "home-ntfy".to_owned(),
            kind: BackendKind::Ntfy,
            url: "http://ntfy.home:2586".to_owned(),
            topic: Some("stack".to_owned()),
            token: Some("tk_secret".to_owned()),
            reader: Some(String::new()),
            hosted: true,
            attempts: Some(3),
            delays: Some(vec![1, 10]),
        })))
    );
}

#[test]
fn every_kind_is_spoken_as_the_core_names_it() {
    for (kind, spoken) in [
        (NotifyKind::Ntfy, BackendKind::Ntfy),
        (NotifyKind::Gotify, BackendKind::Gotify),
        (NotifyKind::Webhook, BackendKind::Webhook),
        (NotifyKind::Apprise, BackendKind::Apprise),
    ] {
        let Command::Notify(Asked::Add(adding)) = asked(NotifyCommand::Add {
            id: "one".to_owned(),
            kind,
            url: "http://one.home".to_owned(),
            topic: None,
            secrets: NotifySecrets::default(),
            hosted: false,
            retries: NotifyRetries::default(),
        }) else {
            unreachable!("an add is an add")
        };
        assert_eq!(adding.kind, spoken);
        assert_eq!((adding.token, adding.reader), (None, None));
    }
}

#[test]
fn a_set_offers_withdraws_or_leaves_the_household_alone() {
    let set = |members: Option<&str>, no_members: bool| {
        asked(NotifyCommand::Set {
            id: "home-ntfy".to_owned(),
            secrets: secrets(Some("NTFY_TOKEN"), None),
            retries: NotifyRetries::default(),
            members: members.map(str::to_owned),
            no_members,
        })
    };
    let setting = |members: Option<Offering>| {
        Command::Notify(Asked::Set(Box::new(Setting {
            id: "home-ntfy".to_owned(),
            token: Some("tk_secret".to_owned()),
            members,
            ..Setting::default()
        })))
    };

    assert_eq!(
        set(Some("Phones"), false),
        setting(Some(Offering::Offer("Phones".to_owned())))
    );
    assert_eq!(set(None, true), setting(Some(Offering::Withdraw)));
    assert_eq!(set(None, false), setting(None));
}

#[test]
fn list_test_and_remove_name_the_back_end_alone() {
    assert_eq!(asked(NotifyCommand::List), Command::Notify(Asked::List));
    assert_eq!(
        asked(NotifyCommand::Test {
            id: "home-ntfy".to_owned()
        }),
        Command::Notify(Asked::Test {
            id: "home-ntfy".to_owned()
        })
    );
    assert_eq!(
        asked(NotifyCommand::Remove {
            id: "home-ntfy".to_owned()
        }),
        Command::Notify(Asked::Remove {
            id: "home-ntfy".to_owned()
        })
    );
}

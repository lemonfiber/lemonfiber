use std::path::{Path, PathBuf};

use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::config::Protocols;

use lemonfiber_core::prerequisites::prerequisites;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Step};

use super::{parse_ids, Flags, RawSetup, SetupFlags};
use crate::prompt::fixtures::{answered, raw, wizard, workable};

#[test]
fn a_flag_run_answers_from_what_it_was_given() {
    // The very same walk drives this as drives the terminal, so a flag run and
    // an interactive one cannot answer differently.
    // Everything a non-interactive run can be told, so each answer comes from
    // a flag rather than from a default.
    let given = RawSetup {
        indexer_url: Some("http://indexer.test".to_owned()),
        indexer_key: Some("the-key".to_owned()),
        usenet_host: Some("news.test".to_owned()),
        usenet_user: Some("me".to_owned()),
        usenet_pass: Some("secret".to_owned()),
        service_user: Some("1000:1000".to_owned()),
        autostart: Some(true),
        ..workable()
    };
    let flags = SetupFlags::parse(given).unwrap_or(SetupFlags::none());
    let prompt = Flags::new(flags, PathBuf::from("/elsewhere"));
    assert_eq!(
        prompt.protocols(),
        Protocols {
            usenet: true,
            torrent: true
        }
    );
    // Named, so the flag wins over the default this was built with.
    assert_eq!(prompt.data_location(), PathBuf::from("/srv/media"));
    assert_eq!(
        prompt.credential(),
        Some(("http://indexer.test".to_owned(), "the-key".to_owned()))
    );
    assert!(prompt.usenet_provider().is_some());
    assert_eq!(prompt.service_user(), Some((1000, 1000)));
    assert!(matches!(prompt.library(), Library::JellyfinDocker));
    assert!(prompt.household());
    assert!(prompt.autostart());
    // Consent given up front is what stands in for a person confirming.
    assert!(prompt.confirm(&wizard().plan()));
    assert!(prompt.storage_warning(
        Path::new("/data"),
        &StorageWarning::CopyOnly { limitation: None }
    ));
    assert!(matches!(
        prompt.credential_failed(&Validation::Rejected {
            detail: "401".to_owned()
        }),
        CredentialChoice::Proceed
    ));
    // The interactive courtesies are nothing at all without a person.
    prompt.prerequisites(&prerequisites(Protocols::both()));
    prompt.hardlinks(Path::new("/data"), None);
    prompt.credential_valid("Prowlarr");
}

#[test]
fn a_flag_run_without_consent_keeps_nothing_it_could_not_prove() {
    // No `--yes`: an unproven credential is left unset rather than stored, and
    // a location that cannot hardlink is not used on someone's behalf.
    let prompt = Flags::new(SetupFlags::none(), PathBuf::from("/srv/media"));
    assert!(matches!(
        prompt.credential_failed(&Validation::Unreachable {
            detail: "no answer".to_owned()
        }),
        CredentialChoice::Skip
    ));
    assert!(!prompt.storage_warning(
        Path::new("/srv/media"),
        &StorageWarning::Untested {
            reason: "absent".to_owned()
        }
    ));
    assert!(!prompt.confirm(&wizard().plan()));
    // What was not given falls back to the same defaults the terminal offers.
    assert_eq!(prompt.data_location(), PathBuf::from("/srv/media"));
    assert_eq!(prompt.credential(), None);
    assert_eq!(prompt.usenet_provider(), None);
    assert_eq!(prompt.service_user(), None);
    assert!(!prompt.household());
    assert!(!prompt.autostart());
    assert!(matches!(prompt.library(), Library::JellyfinDocker));
    assert_eq!(
        prompt.protocols(),
        Protocols {
            usenet: true,
            torrent: true
        }
    );
}

#[test]
fn each_library_choice_can_be_named_on_the_command_line() {
    for (given, expected) in [
        ("docker", Library::JellyfinDocker),
        ("native", Library::JellyfinNative),
        ("NONE", Library::None),
    ] {
        let flags = SetupFlags::parse(RawSetup {
            library: Some(given.to_owned()),
            ..raw()
        });
        let chosen = flags.ok().and_then(|flags| flags.library);
        assert_eq!(
            format!("{chosen:?}"),
            format!("{:?}", Some(expected)),
            "--library {given}"
        );
    }
}

#[test]
fn each_way_of_fetching_content_can_be_named_on_the_command_line() {
    for (given, usenet, torrent) in [
        ("both", true, true),
        ("usenet", true, false),
        ("torrent", false, true),
        ("TORRENTS", false, true),
        ("none", false, false),
        ("neither", false, false),
    ] {
        let flags = SetupFlags::parse(RawSetup {
            protocols: Some(given.to_owned()),
            ..raw()
        });
        assert_eq!(
            flags.ok().and_then(|flags| flags.protocols),
            Some(Protocols { usenet, torrent }),
            "--protocols {given}"
        );
    }
}

#[test]
fn a_choice_the_command_line_does_not_offer_names_what_it_expected() {
    // The message has to say what would have worked, or the operator is left
    // guessing at a vocabulary nothing shows them.
    let protocols = SetupFlags::parse(RawSetup {
        protocols: Some("carrier pigeon".to_owned()),
        ..raw()
    });
    assert!(protocols
        .err()
        .is_some_and(|message| message.contains("both, usenet, torrent or none")));
    let library = SetupFlags::parse(RawSetup {
        library: Some("plex".to_owned()),
        ..raw()
    });
    assert!(library
        .err()
        .is_some_and(|message| message.contains("docker, native or none")));
}

#[test]
fn a_container_user_that_is_not_a_pair_names_what_was_expected() {
    let flags = SetupFlags::parse(RawSetup {
        service_user: Some("me".to_owned()),
        ..raw()
    });
    assert!(flags
        .err()
        .is_some_and(|message| message.contains("must be UID:GID")));
}

#[test]
fn a_step_that_is_no_question_asks_for_no_flag() {
    // Only a question can be answered by a flag; the rest of the walk is work,
    // and naming a flag for it would be nonsense.
    assert_eq!(SetupFlags::none().flag_for(Step::Welcome), None);
}

#[test]
fn the_container_user_is_read_as_a_pair_or_left_to_the_image() {
    assert_eq!(answered(&["1000:1000"]).service_user(), Some((1000, 1000)));
    assert_eq!(answered(&[""]).service_user(), None);
    assert_eq!(parse_ids("1000:1000"), Some((1000, 1000)));
    assert_eq!(parse_ids("1000"), None);
    assert_eq!(parse_ids("x:y"), None);
}

#[test]
fn a_fresh_non_interactive_run_names_every_flag_it_needs() {
    let missing = SetupFlags::none().missing(&wizard());
    for expected in [
        "--protocols",
        "--data-location",
        "--library",
        "--household",
        "--autostart",
        "--yes",
    ] {
        assert!(
            missing.iter().any(|flag| flag.contains(expected)),
            "{expected} should be named as needed, got {missing:?}"
        );
    }
}

#[test]
fn a_fully_flagged_run_needs_nothing_more() {
    let flags = SetupFlags::parse(workable())
        .ok()
        .unwrap_or_else(SetupFlags::none);
    assert!(
        flags.missing(&wizard()).is_empty(),
        "every required flag is present, so none is named"
    );
}

#[test]
fn an_indexer_and_container_user_are_optional_not_required() {
    // Neither an indexer nor a container user is named as missing: an unset
    // indexer is a supported end, and the container user falls to the image
    // default, so a run without them is complete.
    let flags = SetupFlags::parse(workable())
        .ok()
        .unwrap_or(SetupFlags::none());
    // Compared whole rather than searched: a workable run is missing nothing at
    // all, and a search over an empty list proves nothing about either one.
    assert_eq!(flags.missing(&wizard()), Vec::<&str>::new());
}

#[test]
fn malformed_flag_values_are_rejected_with_a_named_reason() {
    let bad_protocol = RawSetup {
        protocols: Some("bogus".to_owned()),
        ..raw()
    };
    assert!(SetupFlags::parse(bad_protocol).is_err());

    // Half an indexer is refused: both parts or neither.
    let half_indexer = RawSetup {
        indexer_url: Some("http://idx".to_owned()),
        ..raw()
    };
    assert!(SetupFlags::parse(half_indexer).is_err());

    // Half a provider is refused too: host with no login.
    let half_provider = RawSetup {
        usenet_host: Some("news.test".to_owned()),
        ..raw()
    };
    assert!(SetupFlags::parse(half_provider).is_err());
}

#[test]
fn a_complete_provider_flag_set_is_offered_to_the_wizard() {
    let complete = RawSetup {
        usenet_host: Some("news.test".to_owned()),
        usenet_user: Some("person".to_owned()),
        usenet_pass: Some("secret".to_owned()),
        ..raw()
    };
    let flags = SetupFlags::parse(complete)
        .ok()
        .unwrap_or_else(SetupFlags::none);
    // The flag run answers the provider question from the flags, defaulting the
    // port to the TLS standard and TLS to on.
    let entry = Flags::new(flags, PathBuf::from("/tmp")).usenet_provider();
    assert!(matches!(
        entry,
        Some(ProviderEntry { host, port, tls, .. }) if host == "news.test" && port == 563 && tls
    ));
}

#[test]
fn each_notification_appetite_can_be_named_on_the_command_line() {
    use lemonfiber_core::alert::Appetite;
    for (given, expected) in [
        ("problems", Appetite::ProblemsOnly),
        ("completions", Appetite::WithCompletions),
        ("EVERYTHING", Appetite::Everything),
    ] {
        // Through the parse an operator's command line actually takes, so the
        // flag is proven wired and not merely parseable.
        let raw = RawSetup {
            notifications: Some(given.to_owned()),
            ..raw()
        };
        let prompt = SetupFlags::parse(raw).map(|flags| Flags::new(flags, PathBuf::new()));
        assert_eq!(
            prompt.map(|prompt| prompt.notifications()),
            Ok(expected),
            "{given}"
        );
    }
    // And a word that is none of them says which ones it expected, rather than
    // quietly falling back to a preset the operator did not ask for.
    let bad = RawSetup {
        notifications: Some("loudly".to_owned()),
        ..raw()
    };
    let refused = SetupFlags::parse(bad).err().unwrap_or_default();
    assert!(
        refused.contains("problems, completions or everything"),
        "{refused}"
    );
}

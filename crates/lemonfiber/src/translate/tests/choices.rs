//! Quality, configuration, bundles and downloads, as commands.

use super::*;

/// Naming no service restarts whatever the form holds, which is the form alone.
#[test]
fn a_restart_carries_the_one_form_and_only_the_services_named() {
    assert_eq!(
        restarting("media".to_owned(), vec!["sonarr".to_owned()]),
        Command::Restart {
            forms: vec!["media".to_owned()],
            services: vec!["sonarr".to_owned()],
        }
    );
    assert_eq!(
        restarting("media".to_owned(), Vec::new()),
        Command::Restart {
            forms: vec!["media".to_owned()],
            services: Vec::new(),
        }
    );
}

/// The words are the title, and the search happens only where it was asked for.
///
/// A term typed unquoted arrives as words and is one title again; the flag is what
/// separates a trace that reads from one that spends a real search.
#[test]
fn a_trace_joins_its_words_and_searches_only_when_asked() {
    assert_eq!(
        traced(&["the".to_owned(), "wire".to_owned()], Some(2), true),
        Command::Trace {
            term: "the wire".to_owned(),
            season: Some(2),
            searching: true,
        }
    );
    assert_eq!(
        traced(&["dune".to_owned()], None, false),
        Command::Trace {
            term: "dune".to_owned(),
            season: None,
            searching: false,
        }
    );
}

#[test]
fn each_configuration_action_becomes_its_own_command() {
    assert_eq!(
        configuration(ConfigAction::Get {
            key: "DATA_ROOT".to_owned()
        }),
        Command::ConfigGet {
            key: "DATA_ROOT".to_owned()
        }
    );
    assert_eq!(
        configuration(ConfigAction::Set {
            key: "DATA_ROOT".to_owned(),
            value: "/srv".to_owned(),
            confirm: false,
            wait: false
        }),
        Command::ConfigSet(Setting::to("DATA_ROOT", "/srv"))
    );
    // The agreement carried through rather than acted on here: a change staged
    // for want of one is what the core answers with, not something a translation
    // could decide.
    assert_eq!(
        configuration(ConfigAction::Set {
            key: "DATA_ROOT".to_owned(),
            value: "/srv".to_owned(),
            confirm: true,
            wait: false
        }),
        Command::ConfigSet(Setting::to("DATA_ROOT", "/srv").agreed(true))
    );
    // And the offer to wait carried through the same way: what waiting means is
    // the core's answer for every surface, not something a translation decides.
    assert_eq!(
        configuration(ConfigAction::Set {
            key: "LEMONFIBER_TORRENT".to_owned(),
            value: "off".to_owned(),
            confirm: false,
            wait: true
        }),
        Command::ConfigSet(
            Setting::to("LEMONFIBER_TORRENT", "off").waiting(Waiting::ForTheDownloads)
        )
    );
    assert_eq!(configuration(ConfigAction::Show), Command::ConfigShow);
}

#[test]
fn showing_and_reapplying_the_quality_choice_need_no_argument() {
    assert_eq!(
        quality(QualityCommand::Show),
        Ok(Command::Quality(QualityAction::Show))
    );
    assert_eq!(
        quality(QualityCommand::Reapply),
        Ok(Command::Quality(QualityAction::Reapply))
    );
}

#[test]
fn a_preset_can_be_set_for_everything_or_for_one_media_type() {
    let everything = quality(QualityCommand::Set {
        preset: "balanced".to_owned(),
        media_type: None,
        confirm: false,
    });
    assert!(matches!(
        everything,
        Ok(Command::Quality(QualityAction::Set {
            preset: Preset::Balanced,
            media_type: None,
            confirm: false
        }))
    ));
    let television = quality(QualityCommand::Set {
        preset: "maximum".to_owned(),
        media_type: Some("tv".to_owned()),
        confirm: true,
    });
    assert!(matches!(
        television,
        Ok(Command::Quality(QualityAction::Set {
            preset: Preset::Maximum,
            confirm: true,
            ..
        }))
    ));
}

#[test]
fn a_music_format_is_its_own_command_rather_than_a_preset() {
    // Music has no resolution, so it is set by format and reaches the service
    // asynchronously — a different command, not a variant of the same one.
    assert!(matches!(
        quality(QualityCommand::Set {
            preset: "lossless".to_owned(),
            media_type: Some("music".to_owned()),
            confirm: false,
        }),
        Ok(Command::QualityMusic {
            format: Format::Lossless
        })
    ));
}

#[test]
fn upgrading_existing_content_is_its_own_command() {
    // It reaches the services asynchronously rather than recording a choice, so
    // it is not a quality action but a command of its own.
    assert_eq!(
        quality(QualityCommand::Upgrade { confirm: true }),
        Ok(Command::QualityUpgrade { confirm: true })
    );
}

#[test]
fn a_preset_or_media_type_this_build_does_not_know_is_a_usage_error() {
    // Named rather than guessed at: the operator gets to see what was expected.
    for (preset, media_type) in [
        ("gorgeous", None),
        ("balanced", Some("audiobooks".to_owned())),
        ("mp3", Some("music".to_owned())),
    ] {
        assert_eq!(
            quality(QualityCommand::Set {
                preset: preset.to_owned(),
                media_type,
                confirm: false,
            }),
            Err(USAGE)
        );
    }
}

#[test]
fn a_bundle_asked_for_at_a_shell_goes_where_the_shell_is() {
    // The one thing this decides that a browser's request cannot: a shell has a
    // filesystem in front of it, so a bundle with no path named goes beside the
    // operator rather than into a directory they would have to be told about.
    let asked = Asked {
        write: true,
        logs: 12,
        filenames: true,
        reveal: vec!["INDEXER_KEY".to_owned()],
        confirm: true,
        out: None,
    };
    assert_eq!(
        bundling(asked),
        Command::Support {
            write: true,
            wanted: Wanted::asked(12, Filenames::Shown, vec!["INDEXER_KEY".to_owned()], true),
            dest: Destination::Beside,
        }
    );
}

#[test]
fn a_bundle_told_where_to_go_goes_there() {
    let asked = Asked {
        write: true,
        logs: 50,
        filenames: false,
        reveal: Vec::new(),
        confirm: false,
        out: Some(std::path::PathBuf::from("/tmp/bundle.tar.gz")),
    };
    assert!(matches!(
        bundling(asked),
        Command::Support {
            dest: Destination::At(path),
            ..
        } if path == std::path::Path::new("/tmp/bundle.tar.gz")
    ));
}

/// One invitation, told what to do about unrated content or told nothing.
fn offering(unrated: Option<RawUnrated>) -> Command {
    invitation(
        "ana".to_owned(),
        RawAllowance {
            libraries: vec!["Films".to_owned()],
            age_limit: Some(12),
            unrated,
        },
    )
}

/// The same invitation as it reaches the core, told what to do or told nothing.
fn reaching(unrated: Option<Unrated>) -> Command {
    Command::Invite {
        name: "ana".to_owned(),
        allowance: Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: Some(12),
            unrated,
        },
        confirm: true,
    }
}

/// The other word reaches the other answer, so the two cannot be one flag read
/// twice — the case above carries the first, and each has to reach its own.
#[test]
fn the_other_word_reaches_the_other_answer() {
    assert_eq!(
        offering(Some(RawUnrated::Allow)),
        reaching(Some(Unrated::LetThrough))
    );
}

/// Saying nothing about unrated content carries nothing.
///
/// Leaving the flag out is not choosing to let it through; it is saying nothing,
/// which leaves the answer to whatever a restriction carries by default. A value
/// written here for a word nobody typed would be this surface deciding on the
/// household's behalf.
#[test]
fn saying_nothing_about_unrated_content_carries_nothing() {
    assert_eq!(offering(None), reaching(None));
}

/// The download every stopping case here names.
const HELD: &str = "A.Show.S01E01";

/// Named alone, it asks what stopping it would cost and agrees to nothing.
#[test]
fn a_download_named_alone_asks_what_stopping_it_would_cost() {
    assert_eq!(
        letting(HELD.to_owned(), None),
        Command::StopSeeding {
            download: HELD.to_owned(),
            agreement: None
        }
    );
}

/// The offer typed back is the agreement the core compares against what stands.
#[test]
fn the_offer_typed_back_is_the_agreement_the_core_compares() {
    assert_eq!(
        letting(HELD.to_owned(), Some("3f2a1b9c".to_owned())),
        Command::StopSeeding {
            download: HELD.to_owned(),
            agreement: Some("3f2a1b9c".to_owned())
        }
    );
}

/// An empty answer is no answer, rather than one the core goes and fails to match.
#[test]
fn an_empty_answer_is_no_answer_at_all() {
    assert_eq!(
        letting(HELD.to_owned(), Some("   ".to_owned())),
        Command::StopSeeding {
            download: HELD.to_owned(),
            agreement: None
        }
    );
}

/// Asking about what is hosted is a reading, and only the words underneath change it.
#[test]
fn a_bare_word_reads_and_does_not_install_anything() {
    assert_eq!(hosting(None), Command::Hosting(Keeping::Read));
}

/// Both of the two long commands are reachable through one word, and the guard
/// carries the forms it was named against.
#[test]
fn either_of_them_can_be_installed_and_taken_back_again() {
    assert_eq!(
        hosting(Some(HostingCommand::Install {
            what: Kept::Watch,
            forms: vec!["tv".to_owned()],
        })),
        Command::Hosting(Keeping::Install {
            what: Hostable::Watch,
            forms: vec!["tv".to_owned()],
        })
    );
    assert_eq!(
        hosting(Some(HostingCommand::Install {
            what: Kept::Expiring,
            forms: Vec::new(),
        })),
        Command::Hosting(Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new(),
        })
    );
    assert_eq!(
        hosting(Some(HostingCommand::Remove { what: Kept::Watch })),
        Command::Hosting(Keeping::Remove {
            what: Hostable::Watch
        })
    );
    assert_eq!(
        hosting(Some(HostingCommand::Remove {
            what: Kept::Expiring
        })),
        Command::Hosting(Keeping::Remove {
            what: Hostable::Expiring
        })
    );
}

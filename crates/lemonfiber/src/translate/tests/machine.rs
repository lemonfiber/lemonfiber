//! Diagnoses, removals, credentials and what is already here, as commands.

use super::*;

/// The start a login makes is asked for by the same word as the other two, and
/// takes no forms.
///
/// It is the third thing this machine can be asked to keep, and the one where the
/// asking *is* the answer to the autostart question — so an operator who types it
/// and gets a guard, or gets nothing, has had a decision about every login taken
/// for them by a translation. Which forms come back is the boot record's business
/// and never the command line's: a start pinned here would be pinned at install
/// time and stale by the next form the operator ran.
#[test]
fn the_start_that_runs_at_a_login_is_asked_for_by_name_and_takes_no_forms() {
    assert_eq!(
        hosting(Some(HostingCommand::Install {
            what: Kept::Boot,
            forms: Vec::new(),
        })),
        Command::Hosting(Keeping::Install {
            what: Hostable::Boot,
            forms: Vec::new(),
        })
    );
    assert_eq!(
        hosting(Some(HostingCommand::Remove { what: Kept::Boot })),
        Command::Hosting(Keeping::Remove {
            what: Hostable::Boot
        })
    );
}

/// Naming no category at all asks for the whole suite rather than for nothing.
#[test]
fn naming_no_category_asks_for_the_whole_suite() {
    assert_eq!(narrowed(None), Ok(Narrowing::Suite));
}

/// A check inside a category is narrowed to that check, under the name given.
#[test]
fn a_check_inside_a_category_is_narrowed_to_that_check() {
    assert_eq!(
        narrowed(Some("storage.space")),
        Ok(Narrowing::Check("storage.space".to_owned()))
    );
}

/// A name that is neither a category nor a check inside one is a usage error, so
/// that a mistyped narrowing is corrected rather than quietly running everything.
#[test]
fn a_name_that_is_neither_a_category_nor_a_check_is_a_usage_error() {
    assert_eq!(narrowed(Some("nonsense")), Err(USAGE));
}

/// A preset this build offers reaches the command; one it does not is a usage
/// error naming the ones there are, rather than a quiet fall back to the default
/// that would leave the operator believing they had changed something.
#[test]
fn a_notification_preset_is_taken_by_name_and_refused_by_name() {
    assert_eq!(
        alerts(AlertCommand::Show),
        Ok(Command::Alerts(AlertAction::Show))
    );
    assert_eq!(
        alerts(AlertCommand::Set {
            preset: "everything".to_owned(),
        }),
        Ok(Command::Alerts(AlertAction::Set(Appetite::Everything)))
    );
    assert_eq!(
        alerts(AlertCommand::Set {
            preset: "with-completions".to_owned(),
        }),
        Ok(Command::Alerts(AlertAction::Set(Appetite::WithCompletions)))
    );
    assert_eq!(
        alerts(AlertCommand::Set {
            preset: "loud".to_owned(),
        }),
        Err(USAGE)
    );
}

/// What a diagnosis was narrowed to reaches the command alongside what it was
/// asked to accept and whether it may disrupt.
#[test]
fn a_diagnosis_carries_what_it_was_narrowed_to() {
    assert_eq!(
        diagnosing(Some("storage.space"), true, Some("STORAGE-1".to_owned())),
        Ok(Command::Doctor {
            narrowing: Narrowing::Check("storage.space".to_owned()),
            disruptive: true,
            accept: Some("STORAGE-1".to_owned()),
        })
    );
}

/// A removal as the command line accepted it.
fn removing_asked(
    tier: lemonfiber::cli::RawRemoval,
    agreed: Option<&str>,
    wait: bool,
) -> lemonfiber::cli::RawRemoving {
    lemonfiber::cli::RawRemoving {
        tier,
        confirm: true,
        agreed: agreed.map(str::to_owned),
        wait,
    }
}

/// Each of the four words reaches the removal it names, and nothing else.
///
/// Compared whole rather than field by field: what this is about is that one
/// word becomes one command, and a comparison that read only the removal out of
/// it would pass on a translation that had dropped everything else.
#[test]
fn each_word_reaches_the_removal_it_names() {
    use lemonfiber::cli::RawRemoval;
    use lemonfiber_core::app::Removing;
    use lemonfiber_core::uninstall::Tier;

    let pairs = [
        (RawRemoval::Stop, Tier::Stop),
        (RawRemoval::Services, Tier::Services),
        (RawRemoval::Configuration, Tier::Configuration),
        (RawRemoval::Media, Tier::Media),
    ];
    for (written, meant) in pairs {
        assert_eq!(
            super::super::removing(removing_asked(written, None, false)),
            Command::Uninstall(Removing::surveying(meant).confirmed(true)),
            "{written:?}"
        );
    }
}

/// Nothing typed is nothing agreed to: a flag given empty is an answer to no
/// listing, and carrying it would be a name the core goes and fails to match.
#[test]
fn an_empty_answer_to_a_listing_is_dropped_rather_than_carried() {
    use lemonfiber::cli::RawRemoval;
    use lemonfiber_core::app::Removing;
    use lemonfiber_core::uninstall::Tier;

    let media = || Removing::surveying(Tier::Media).confirmed(true);

    assert_eq!(
        super::super::removing(removing_asked(RawRemoval::Media, Some("  "), false)),
        Command::Uninstall(media())
    );
    assert_eq!(
        super::super::removing(removing_asked(RawRemoval::Media, Some("deadbeef"), false)),
        Command::Uninstall(media().agreeing(Some("deadbeef".to_owned())))
    );
}

/// The wait is carried as the word the core knows it by rather than as a flag.
#[test]
fn asking_a_removal_to_wait_reaches_it_as_a_wait() {
    use lemonfiber::cli::RawRemoval;
    use lemonfiber_core::app::Removing;
    use lemonfiber_core::uninstall::Tier;

    let stopping = || Removing::surveying(Tier::Stop).confirmed(true);

    assert_eq!(
        super::super::removing(removing_asked(RawRemoval::Stop, None, true)),
        Command::Uninstall(stopping().waiting(Waiting::ForTheDownloads))
    );
    assert_eq!(
        super::super::removing(removing_asked(RawRemoval::Stop, None, false)),
        Command::Uninstall(stopping())
    );
}

/// Naming nothing is the reading, which is what people type.
#[test]
fn a_credentials_command_with_no_flags_is_the_reading() {
    assert_eq!(
        credentials(RawCredentials {
            reveal: None,
            rotate: None,
            confirm: false,
        }),
        Command::Credentials(Asking::Read)
    );
}

/// Naming one to reveal is an act on that credential, and the confirmation the
/// operator gave belongs to it rather than to the reading beside it.
///
/// Carried through rather than acted on here: what an unconfirmed reveal answers
/// with is a warning written where the value would otherwise be, which is the
/// core's to say and not this translation's.
#[test]
fn a_reveal_carries_the_name_and_whether_it_was_confirmed() {
    assert_eq!(
        credentials(RawCredentials {
            reveal: Some("Indexer API key".to_owned()),
            rotate: None,
            confirm: true,
        }),
        Command::Credentials(Asking::Reveal {
            credential: "Indexer API key".to_owned(),
            confirmed: true,
        })
    );

    // A reveal wins over a rotation named in the same breath, and an unconfirmed
    // one still reaches the core, which is where the warning is written.
    assert_eq!(
        credentials(RawCredentials {
            reveal: Some("Indexer API key".to_owned()),
            rotate: Some("Sonarr API key".to_owned()),
            confirm: false,
        }),
        Command::Credentials(Asking::Reveal {
            credential: "Indexer API key".to_owned(),
            confirmed: false,
        })
    );
}

#[test]
fn a_rotation_carries_the_name_and_nothing_else() {
    assert_eq!(
        credentials(RawCredentials {
            reveal: None,
            rotate: Some("Indexer API key".to_owned()),
            confirm: false,
        }),
        Command::Credentials(Asking::Rotate {
            credential: "Indexer API key".to_owned(),
        })
    );
}

/// Typing `migrate` to see what is here must not have taken over the stack.
#[test]
fn naming_nothing_surveys_and_changes_none_of_it() {
    assert_eq!(
        super::super::migrating(None),
        lemonfiber_core::app::MigrateAction::Survey
    );
}

/// Carrying records across carries its confirmation through the same way.
#[test]
fn carrying_records_across_carries_its_confirmation_through() {
    let asked = lemonfiber::cli::MigrateCommand::Import { confirm: true };
    assert_eq!(
        super::super::migrating(Some(&asked)),
        lemonfiber_core::app::MigrateAction::Act {
            mode: lemonfiber_core::migration::mode::Mode::Import,
            confirmed: true,
        }
    );
}

/// Standing in place of it carries its confirmation through the same way.
#[test]
fn standing_in_place_carries_its_confirmation_through() {
    let asked = lemonfiber::cli::MigrateCommand::Replace { confirm: true };
    assert_eq!(
        super::super::migrating(Some(&asked)),
        lemonfiber_core::app::MigrateAction::Act {
            mode: lemonfiber_core::migration::mode::Mode::Replace,
            confirmed: true,
        }
    );
}

/// Standing beside carries its confirmation through the same way.
#[test]
fn standing_beside_carries_its_confirmation_through() {
    let asked = lemonfiber::cli::MigrateCommand::Beside { confirm: true };
    assert_eq!(
        super::super::migrating(Some(&asked)),
        lemonfiber_core::app::MigrateAction::Act {
            mode: lemonfiber_core::migration::mode::Mode::Beside,
            confirmed: true,
        }
    );
}

/// The bare word reads what this stack wires to what; the verb under it changes
/// one of those links. A command line that asked for a listing cannot become one
/// that changes a stack by being mistyped, and that is the whole reason the write
/// is a subcommand rather than a pair of flags.
#[test]
fn asking_what_is_wired_reads_and_the_verb_under_it_writes() {
    assert_eq!(super::super::wiring(None), Command::Wiring(Linking::Read));
    assert_eq!(
        super::super::wiring(Some(lemonfiber::cli::WiringCommand::Fill {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
        })),
        Command::Wiring(Linking::Fill(Filling {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
        }))
    );
}

/// Confirming is carried through as given: it is the operator saying they have
/// backed up what the rehearsal named.
#[test]
fn adopting_carries_the_confirmation_through_as_it_was_given() {
    let asked = lemonfiber::cli::MigrateCommand::Adopt { confirm: true };
    assert_eq!(
        super::super::migrating(Some(&asked)),
        lemonfiber_core::app::MigrateAction::Act {
            mode: lemonfiber_core::migration::mode::Mode::Adopt,
            confirmed: true,
        }
    );
    let unconfirmed = lemonfiber::cli::MigrateCommand::Adopt { confirm: false };
    assert_eq!(
        super::super::migrating(Some(&unconfirmed)),
        lemonfiber_core::app::MigrateAction::Act {
            mode: lemonfiber_core::migration::mode::Mode::Adopt,
            confirmed: false,
        }
    );
}

/// Which door a word under `plugin` goes through, and the command it becomes.
fn door(read: lemonfiber::cli::PluginCommand) -> Option<Command> {
    match super::super::plugin(read) {
        super::super::Under::Dispatched(command) => Some(command),
        super::super::Under::Published(_) => None,
    }
}

/// The two words about this machine become commands, and the path the operator
/// typed is carried through untouched.
#[test]
fn the_words_about_this_machine_become_commands() {
    assert_eq!(
        door(lemonfiber::cli::PluginCommand::Install {
            path: std::path::PathBuf::from("/srv/komga")
        }),
        Some(Command::Plugins(plugins::Asked::Install {
            path: std::path::PathBuf::from("/srv/komga")
        }))
    );
    assert_eq!(
        door(lemonfiber::cli::PluginCommand::Installed),
        Some(Command::Plugins(plugins::Asked::Installed))
    );
    assert_eq!(
        door(lemonfiber::cli::PluginCommand::Remove {
            plugin: "komga".to_owned()
        }),
        Some(Command::Plugins(plugins::Asked::Remove {
            plugin: "komga".to_owned()
        })),
        "a removal names the plugin rather than a path, because the source may be gone"
    );
    assert_eq!(
        door(lemonfiber::cli::PluginCommand::Update {
            path: std::path::PathBuf::from("/srv/komga")
        }),
        Some(Command::Plugins(plugins::Asked::Update {
            path: std::path::PathBuf::from("/srv/komga")
        })),
        "an update names the new version's source, because that is what is coming on"
    );
}

/// And the five that are documents this build generated go the other way,
/// because there is no stack to ask and nothing to decide.
#[test]
fn the_documents_an_author_reads_are_not_dispatched() {
    for read in [
        Authoring::Capabilities,
        Authoring::ExtensionPoints,
        Authoring::Schema,
        Authoring::Claims {
            path: std::path::PathBuf::from("/srv/komga"),
        },
        Authoring::Provenance {
            path: std::path::PathBuf::from("/srv/komga"),
            keys: Vec::new(),
        },
    ] {
        assert_eq!(door(lemonfiber::cli::PluginCommand::Authoring(read)), None);
    }
}

//! What a rehearsal carries into the run, and what a real run carries.

use super::*;

/// A rehearsal of the eight that answer twice is the answer they already give
/// unconfirmed, so the go-ahead is taken back on the way in.
#[test]
fn a_rehearsal_carries_the_confirmable_commands_without_their_yes() {
    let rehearsing = crate::test_support::a_context().build().rehearsing();
    for asked in [
        Command::Reset {
            agreement: Some("00000000-00000000".to_owned()),
        },
        Command::Remove {
            name: "ana".to_owned(),
            agreement: Some("00000000-00000000-00000000".to_owned()),
        },
        Command::QualityUpgrade { confirm: true },
        Command::Migrate(MigrateAction::Act {
            mode: crate::migration::mode::Mode::Adopt,
            confirmed: true,
        }),
        Command::Update(update::Asked {
            service: None,
            confirm: true,
            wait: Waiting::Never,
            offer: None,
        }),
        Command::Restore(Restoring {
            archive: restore::Kept::Named("anything".to_owned()),
            repoint: false,
            consent: restore::Consent::Standing,
        }),
        Command::Repair {
            consent: repair::Consent::Standing,
            disruptive: false,
        },
        bundling(true),
    ] {
        let carried = carried(asked.clone(), &rehearsing);
        assert_ne!(carried, asked, "{asked:?} kept the yes it was given");
    }
}

/// And a rehearsal of a replacement answering its offer is the reading that names
/// one: the offer is its yes, so it is the offer that is taken back.
#[test]
fn a_rehearsed_replacement_is_the_reading_of_what_it_would_stop() {
    let rehearsing = crate::test_support::a_context().build().rehearsing();
    let answering = Command::Migrate(MigrateAction::Replace {
        offer: Some("5c3a1d20".to_owned()),
    });
    assert_eq!(
        carried(answering, &rehearsing),
        Command::Migrate(MigrateAction::Replace { offer: None })
    );
}

/// And what a rehearsal of a support bundle carries is the read beside it, rather
/// than merely something other than what was asked.
///
/// Its own case because `assert_ne!` above is satisfied by any difference, and the
/// difference that matters here is which of the command's two halves runs: the
/// describing one, which is already classified as changing nothing.
#[test]
fn a_rehearsed_support_bundle_is_the_run_that_describes_one() {
    let rehearsing = crate::test_support::a_context().build().rehearsing();
    assert_eq!(carried(bundling(true), &rehearsing), bundling(false));
}

/// And a real run carries exactly what it was handed, because the withholding is
/// what a rehearsal is rather than something the dispatcher does to everybody.
#[test]
fn a_real_run_carries_the_command_it_was_given() {
    let real = crate::test_support::a_context().build();
    let asked = Command::Reset {
        agreement: Some("00000000-00000000".to_owned()),
    };
    assert_eq!(carried(asked.clone(), &real), asked);
}

/// The same split, on the three other commands that carry a read and a write under
/// one name.
#[test]
fn every_command_that_carries_both_is_split_on_which_it_is() {
    for (command, expected) in [
        (Command::Migrate(MigrateAction::Survey), Rehearsal::Reads),
        (Command::Hosting(Keeping::Read), Rehearsal::Reads),
        (Command::Setup(SetupAction::Where), Rehearsal::Reads),
        (Command::Setup(SetupAction::Apply), Rehearsal::Reports),
        (Command::Wiring(Linking::Read), Rehearsal::Reads),
        (
            Command::Wiring(Linking::Fill(Filling {
                capability: "indexer.search".to_owned(),
                service: "nzbhydra2".to_owned(),
                reason: None,
                agreement: None,
            })),
            Rehearsal::Reports,
        ),
        (Command::Settings(Settling::Read), Rehearsal::Reads),
        (
            Command::Settings(Settling::Set {
                values: [("domain".to_owned(), "home.example".to_owned())].into(),
                agreement: None,
            }),
            Rehearsal::Reports,
        ),
        (
            Command::Settings(Settling::Clear {
                settings: vec!["domain".to_owned()],
                agreement: None,
            }),
            Rehearsal::Reports,
        ),
    ] {
        assert!(
            asked(&command).rehearsal == expected,
            "{command:?} was read as the wrong half of what it carries"
        );
    }
}

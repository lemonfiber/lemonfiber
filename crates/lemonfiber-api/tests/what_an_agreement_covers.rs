//! What the agreement each action carries is an agreement *to*.
//!
//! [`TAKES_AGREEMENT`] is a list of the actions whose command has a field for the
//! operator's yes. It is not a list of the actions that rehearse first, and the two
//! were read as one thing for long enough that three artefacts came to disagree: the
//! table, the doc comment over it and the code underneath.
//!
//! So each of the seven is held to where its agreement actually lands, one at a time,
//! rather than to a rule they were assumed to share. A guard that asserted the shared
//! rule would have to be either false or vacuous — false for the two that do not
//! follow it, and vacuous if it were weakened until they did.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still.

use lemonfiber_api::actions::{named, Arguments, Refused, TAKES_AGREEMENT};
use lemonfiber_core::app::bundle::{Wanted, LINES};
use lemonfiber_core::app::repair::Consent;
use lemonfiber_core::app::restore::{self, Kept};
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::bundle::Filenames;
use lemonfiber_core::quality::Preset;

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

/// A bundle asked for with the two fields this is about, and careful defaults for
/// everything else.
fn bundling(write: bool, reveal: &[&str], confirm: bool) -> Arguments {
    Arguments {
        write,
        reveal: reveal.iter().map(|named| (*named).to_owned()).collect(),
        confirm,
        ..Arguments::default()
    }
}

// ── The actions that carry an agreement, and every one can be given it ─────────

#[test]
fn every_action_the_table_names_accepts_the_agreement_it_names_them_for() {
    // The table's own claim, which is the only one all of them share: this is the list
    // of names whose command has somewhere to put a yes. An entry that did not
    // would refuse the very argument the table says it takes.
    for action in TAKES_AGREEMENT {
        let given = Arguments {
            confirm: true,
            // The three that cannot be asked for at all without a subject, so what
            // this holds is the agreement rather than the missing one.
            preset: (*action == "quality-set").then(|| Preset::Balanced.label().to_owned()),
            archive: (*action == "restore").then(|| "a-backup.tar.gz".to_owned()),
            name: (*action == "remove").then(|| "ana".to_owned()),
            ..Arguments::default()
        };
        assert_eq!(refusal(action, given), None, "{action}");
    }
}

// ── Three of them are the fork: the agreement decides whether it happens ───────

#[test]
fn a_reset_an_upgrade_and_a_restore_carry_the_agreement_into_the_command() {
    assert_eq!(
        command(
            "reset",
            Arguments {
                confirm: true,
                ..Arguments::default()
            }
        ),
        Some(Command::Reset { confirm: true })
    );
    assert_eq!(
        command(
            "quality-upgrade",
            Arguments {
                confirm: true,
                ..Arguments::default()
            }
        ),
        Some(Command::QualityUpgrade { confirm: true })
    );
    assert_eq!(
        command(
            "restore",
            Arguments {
                archive: Some("a-backup.tar.gz".to_owned()),
                confirm: true,
                ..Arguments::default()
            }
        ),
        Some(Command::Restore {
            archive: Kept::Named("a-backup.tar.gz".to_owned()),
            repoint: false,
            // A yes carrying no listing is the standing consent, which is what
            // `confirm` alone has always meant here.
            consent: restore::Consent::Standing,
        })
    );
}

// ── A repair's agreement is the yes to an offer, which is its own group ────────

#[test]
fn a_repair_without_the_agreement_is_the_offer_and_with_it_alone_is_standing() {
    assert_eq!(
        command("repair", Arguments::default()),
        Some(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );
    assert_eq!(
        command(
            "repair",
            Arguments {
                confirm: true,
                ..Arguments::default()
            }
        ),
        Some(Command::Repair {
            consent: Consent::Standing,
            disruptive: false,
        })
    );
}

// ── A quality choice's agreement is a cost, not a go-ahead ────────────────────

#[test]
fn a_quality_choice_reaches_the_same_command_agreed_to_or_not() {
    // The one entry the shared reading is false for. Both runs are the write; the
    // agreement rides along to answer a cost the core weighs against this host —
    // software transcoding — and on a host that transcodes in hardware there is no
    // cost for it to answer at all.
    //
    // Held here so that a surface putting an account in front of one of these has
    // to change this test to do it, rather than inheriting a promise the table
    // never made.
    let chosen = |confirm: bool| {
        command(
            "quality-set",
            Arguments {
                preset: Some(Preset::Maximum.label().to_owned()),
                confirm,
                ..Arguments::default()
            },
        )
    };
    let recorded = |confirm: bool| {
        Some(Command::Quality(QualityAction::Set {
            preset: Preset::Maximum,
            media_type: None,
            confirm,
        }))
    };

    assert_eq!(chosen(false), recorded(false));
    assert_eq!(chosen(true), recorded(true));
}

// ── The one removal with a ratio behind it takes no blanket yes at all ────────

/// One completed download, as a client and the disk account both name it.
const HELD: &str = "A.Show.S01E01.1080p";

/// An offer, as one names itself.
const READ: &str = "0f0f0f0f";

#[test]
fn stopping_one_download_seeding_refuses_the_yes_the_account_beside_it_takes() {
    // The account's yes is to reclaiming what costs nothing. This takes the one thing
    // on the disk that costs something, so the account's shape of yes is refused by
    // name here rather than quietly accepted: what this takes is the offer's own name,
    // which nobody holds who did not read what it cost.
    assert!(
        !TAKES_AGREEMENT.contains(&"stop-seeding"),
        "a blanket yes would be agreement from somebody who read no consequence"
    );
    assert_eq!(
        refusal(
            "stop-seeding",
            Arguments {
                download: Some(HELD.to_owned()),
                confirm: true,
                ..Arguments::default()
            }
        ),
        Some(Refused::Unwanted {
            action: "stop-seeding".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn the_offer_read_is_the_whole_of_how_this_one_is_agreed_to() {
    // Without it the request is the offer and removes nothing; with it, it is the
    // answer to that offer and nothing else could have been.
    assert_eq!(
        command(
            "stop-seeding",
            Arguments {
                download: Some(HELD.to_owned()),
                ..Arguments::default()
            }
        ),
        Some(Command::StopSeeding {
            download: HELD.to_owned(),
            agreement: None
        })
    );
    assert_eq!(
        command(
            "stop-seeding",
            Arguments {
                download: Some(HELD.to_owned()),
                offer: Some(READ.to_owned()),
                ..Arguments::default()
            }
        ),
        Some(Command::StopSeeding {
            download: HELD.to_owned(),
            agreement: Some(READ.to_owned())
        })
    );
}

#[test]
fn an_answer_naming_no_download_has_lost_the_only_subject_that_makes_it_safe() {
    assert_eq!(
        refusal(
            "stop-seeding",
            Arguments {
                offer: Some(READ.to_owned()),
                ..Arguments::default()
            }
        ),
        Some(Refused::Missing {
            action: "stop-seeding".to_owned(),
            argument: "download".to_owned()
        })
    );
}

// ── A bundle's agreement is the revealing, and the file is the other field ─────

#[test]
fn a_bundles_agreement_lands_on_the_revealing_and_never_on_the_file() {
    // The other entry the shared reading is false for, and the one it was false
    // about in both directions: the agreement does not decide whether a file is
    // written, and the field that does is not the agreement.
    //
    // A yes with nothing named to reveal writes nothing at all.
    assert_eq!(
        command("support", bundling(false, &[], true)),
        Some(Command::Support {
            write: false,
            wanted: Wanted::asked(LINES, Filenames::Replaced, Vec::new(), true),
            dest: Destination::Kept,
        })
    );
    // And a file asked for with no yes is written, revealing nothing — which is
    // what makes the agreement an agreement to the revealing.
    assert_eq!(
        command("support", bundling(true, &[], false)),
        Some(Command::Support {
            write: true,
            wanted: Wanted::asked(LINES, Filenames::Replaced, Vec::new(), false),
            dest: Destination::Kept,
        })
    );
}

#[test]
fn a_setting_named_to_be_shown_travels_beside_the_yes_that_answers_it() {
    // The pair the agreement exists for, carried on one value so the flag that
    // publishes a credential and the yes to publishing it cannot arrive apart.
    let asked = command("support", bundling(true, &["QBITTORRENT_PASSWORD"], true));
    let wanted = asked.and_then(|command| match command {
        Command::Support { wanted, .. } => Some(wanted),
        _ => None,
    });

    assert_eq!(
        wanted,
        Some(Wanted::asked(
            LINES,
            Filenames::Replaced,
            vec!["QBITTORRENT_PASSWORD".to_owned()],
            true,
        ))
    );
}

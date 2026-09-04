//! Which of the core's own commands each name reaches.
//!
//! The whole of this surface's authority. A command is what the command line
//! produces too, so an action reaching a command cannot be something only a browser
//! can do — and a name outside the table is refused rather than invented, which is
//! what keeps the two surfaces the same size.
//!
//! Only the actions that *change* something are here. Asking what the stack is
//! doing is a read and has an endpoint of its own; a write that also happens to
//! report is still a write — and so is one that mostly waits. A guard exists to
//! stop the services when the drive under them goes, and a walk searches, grabs
//! and imports; that both spend most of their time watching does not make either
//! of them a question.
//!
//! Two of them answer with what a read answers and are here for the same reason.
//! Widened to the checks that disturb a running system, a diagnosis takes the tunnel
//! away to prove it comes back and spends a real search against the indexers; widened
//! to asking the indexers what they carry, a trace spends one too. So both are asked
//! for at the door changes are asked for, whatever their answers look like. A read
//! that disturbed something would not be a read.

use lemonfiber_core::app::bundle::{Wanted, LINES};
use lemonfiber_core::app::restore::Kept;
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{BandwidthAsked, Command, QualityAction, Waiting};
use lemonfiber_core::doctor::Narrowing;

mod household;

use super::asked::{unwanted, Arguments, Disturbing};
use super::reading::{consent, diagnosing, following, listing, quality, widening};
use super::Refused;

/// Every action this surface offers, in the order they are worth reading.
///
/// Held as a list so that what the surface offers can be counted and checked
/// against what it translates, rather than being knowable only by reading a
/// match arm at a time.
pub const OFFERED: &[&str] = &[
    "up",
    "down",
    "switch",
    "restart",
    "pull",
    "config-set",
    "quality-set",
    "quality-reapply",
    "quality-upgrade",
    "seed",
    "adopt",
    "reset",
    "forget",
    "space",
    "stop-seeding",
    "bandwidth",
    "backup",
    "invite",
    "remove",
    "reissue",
    "household-allow",
    "household-approve",
    "household-decline",
    "support",
    "restore",
    "watch",
    "walkthrough",
    "diagnose",
    "repair",
    "undo",
    "accept",
    "search",
];

/// The actions that must be told what to act on.
///
/// A guard is one of them for a reason the other three do not share: it is not
/// that the request has lost its subject, but that a watch with nothing to stop
/// would see the drive vanish and have nothing to do about it. The command line
/// refuses all four the same way.
const NAMES_ITS_FORMS: [&str; 4] = ["switch", "restart", "pull", "watch"];

/// The setting a change names, and what to change it to.
///
/// Named apart from the table for the reason the household three are: both halves are
/// required, each is refused by its own name, and a reading that can refuse belongs
/// beside its refusals rather than inside a list of arms.
fn setting(key: Option<String>, value: Option<String>) -> Result<Command, Refused> {
    let missing = |argument: &str| Refused::Missing {
        action: "config-set".to_owned(),
        argument: argument.to_owned(),
    };
    match (key, value) {
        (Some(key), Some(value)) => Ok(Command::ConfigSet { key, value }),
        (None, _) => Err(missing("key")),
        (_, None) => Err(missing("value")),
    }
}

/// Which completed download to stop seeding, and the offer being answered.
///
/// Named apart for the same reason, and it is the one where the subject matters most:
/// this is the only request on this surface that destroys something outside the
/// machine's own filesystem, and one naming no download has lost the thing that makes
/// it safe. It takes no `confirm` — the yes is the offer's own name, so the only way
/// to reach the removal is through the run that said what it costs.
fn stopping(download: Option<String>, offer: Option<String>) -> Result<Command, Refused> {
    let download = download.ok_or_else(|| Refused::Missing {
        action: "stop-seeding".to_owned(),
        argument: "download".to_owned(),
    })?;
    Ok(Command::StopSeeding {
        download,
        agreement: offer,
    })
}

/// The command a warning being answered names, or why it names none.
///
/// Over the whole suite. Only something a run warns about can be answered, and a
/// narrowed run is a run that may not have raised it — so what a browser narrows is
/// the diagnosis it reads, not the warning it answers.
///
/// Apart from the table for the reason [`about_a_person`] is: the table is one row per
/// request, and an action that has to say what it lacks before it can name a command
/// is longer than a row.
fn accepting(check: Option<String>, disruptive: Disturbing) -> Result<Command, Refused> {
    let Some(check) = check else {
        return Err(Refused::Missing {
            action: "accept".to_owned(),
            argument: "check".to_owned(),
        });
    };
    Ok(Command::Doctor {
        narrowing: Narrowing::Suite,
        disruptive: disruptive.included(),
        accept: Some(check),
    })
}

/// The command an action names, or why it names none.
///
/// # Errors
///
/// Returns the [`Refused`] a caller should be answered with.
pub fn named(action: &str, given: Arguments) -> Result<Command, Refused> {
    if let Some(refused) = beforehand(action, &given) {
        return Err(refused);
    }
    // Everything addressed to somebody who lives here goes next door before this
    // takes the carrier apart: an account offered, a password taken off, an account
    // taken away, what the household may ask for, and one thing it already asked for.
    // Each of them has to say what it lacks before it can name a command, which is
    // longer than a row — and none of the fields they use is one this table reads.
    if household::about_the_household(action) {
        return household::asked_for(action, given);
    }
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    let Arguments {
        forms,
        services,
        wait,
        service,
        key,
        value,
        preset,
        media_type,
        archive,
        repoint,
        write,
        logs,
        filenames,
        reveal,
        only,
        check,
        disruptive,
        offer,
        agreed,
        confirm,
        item,
        term,
        season,
        download,
        down,
        up,
        active,
        line,
        cap,
        exceeded,
        unrestricted_for,
        ..
    } = given;
    match action {
        // Starting named services and bringing a form up are different requests
        // rather than one request with an argument, exactly as stopping them and
        // tearing a form down are, and Compose spells both pairs differently.
        "up" if services.is_empty() => Ok(Command::Up { forms }),
        "up" => Ok(Command::Start { forms, services }),
        "down" if services.is_empty() => Ok(Command::Down { forms, wait }),
        "down" => Ok(Command::Halt { forms, services }),
        "switch" => Ok(Command::Switch { forms }),
        "restart" => Ok(Command::Restart { forms, services }),
        "pull" => Ok(Command::Pull { forms }),
        "config-set" => setting(key, value),
        "quality-set" => match preset {
            Some(preset) => quality(&preset, media_type, confirm),
            None => Err(needs("preset")),
        },
        "quality-reapply" => Ok(Command::Quality(QualityAction::Reapply)),
        "quality-upgrade" => Ok(Command::QualityUpgrade { confirm }),
        "seed" => Ok(Command::Seed),
        "adopt" => Ok(Command::Adopt),
        "reset" => Ok(Command::Reset { confirm }),
        // Unconfirmed it lists what would go, which is the same listing `/api/stored`
        // answers with — so what a browser agrees to is what it was shown.
        "forget" => Ok(Command::Forget { confirm }),
        // The same reading twice, over the operator's own disk: unconfirmed it is the
        // account `/api/space` answers with, and confirmed it takes only what that
        // account named as costing nothing.
        "space" => Ok(Command::Space { confirm }),
        // The one thing that account names and leaves alone, asked for on its own.
        "stop-seeding" => stopping(download, offer),
        // And the same reading twice over the line: given nothing it is the account
        // `/api/bandwidth` answers with, and given a limit it declares one and hands
        // it to every download client. Not one of the seven is read here — what a
        // share means is the core's answer for all three surfaces at once.
        "bandwidth" => Ok(Command::Bandwidth(BandwidthAsked {
            down,
            up,
            active,
            line,
            cap,
            exceeded,
            unrestricted_for,
        })),
        "backup" => Ok(Command::Backup { service }),
        // The two reads this surface serves twice, each reaching the same command its
        // own endpoint reaches and widened by the same word the command line widens
        // it with. Neither is a second reading of the stack; each is the reading that
        // changes it — the tunnel taken away to prove the killswitch, and a live
        // search spent against the indexers. A read that disturbed something would
        // not be a read, and a POST is the only door this surface has that is not one.
        "diagnose" => widening(action, disruptive).and_then(|()| diagnosing(only)),
        "search" => widening(action, disruptive).and_then(|()| following(term, season)),
        // The offer and the yes are one action because they are one request read
        // twice: unconfirmed it says what each repair would do and what else
        // changes if it does, and confirmed it carries out what was agreed to.
        "repair" => consent(confirm, offer, agreed).map(|consent| Command::Repair {
            consent,
            disruptive: disruptive.included(),
        }),
        // No subject at all. Which repair was last, what reversing it takes and
        // which of those need a service to reach are the core's to decide, so
        // there is nothing here for a caller to name.
        "undo" => Ok(Command::Undo),
        "accept" => accepting(check, disruptive),
        // The bundle goes where lemonfiber keeps its own files. A browser has no
        // filesystem in front of it and no path it could name that would mean
        // anything here, so the destination is settled rather than asked for —
        // which answers *which path*, the only web-specific question a bundle has.
        "support" => Ok(Command::Support {
            write,
            wanted: Wanted::asked(logs.unwrap_or(LINES), filenames, reveal, confirm),
            dest: Destination::Kept,
        }),
        // By the name it was written under, never by a path. The server runs as the
        // operator, so a path it accepted would be a path it could read; a name is
        // resolved beneath the backups directory by the core and nowhere else.
        "restore" => match archive {
            Some(name) => listing(confirm, offer).map(|consent| Command::Restore {
                archive: Kept::Named(name),
                repoint,
                consent,
            }),
            None => Err(needs("archive")),
        },
        "watch" => Ok(Command::Watch { forms }),
        // Naming nothing is a request rather than an omission: a walk asked for
        // nothing in particular suggests something likely to work, which is what an
        // operator with an empty library needs. Blank is nothing named, not an
        // empty title, so a browser that sent the field and left it alone asks the
        // same thing as one that left it out.
        "walkthrough" => Ok(Command::Walkthrough {
            item: item.filter(|named| !named.trim().is_empty()),
        }),
        _ => Err(Refused::Unknown {
            name: action.to_owned(),
        }),
    }
}

/// Why a request is refused before the command it names is looked for.
///
/// The three refusals that are about the request as a whole rather than about
/// something one action needs: a subject that cannot be left out, an argument the
/// action's command has nowhere to put, and a pair that names two requests at once.
/// None of them depends on which command is reached, so all three are settled
/// before anything looks.
fn beforehand(action: &str, given: &Arguments) -> Option<Refused> {
    // Naming nothing means everything for the actions that can mean it, and is a
    // mistake for the four that cannot: switching to nothing, restarting nothing,
    // fetching nothing and guarding nothing are each a request with nothing to act
    // on, and the command line refuses all four the same way.
    if NAMES_ITS_FORMS.contains(&action) && given.forms.is_empty() {
        return Some(Refused::Missing {
            action: action.to_owned(),
            argument: "forms".to_owned(),
        });
    }
    if let Some(refused) = unwanted(action, given, OFFERED) {
        return Some(refused);
    }
    // Two arguments, two requests. A teardown that waits and a stop of named
    // services are different things to ask for, and a run given both would have to
    // drop one of them.
    (action == "down" && given.wait == Waiting::ForTheDownloads && !given.services.is_empty()).then(
        || Refused::Together {
            action: action.to_owned(),
            argument: "wait".to_owned(),
            alongside: "services".to_owned(),
        },
    )
}

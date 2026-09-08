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
use lemonfiber_core::app::{Command, Hostable, Keeping, Removing, Waiting, HOSTABLE};
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::uninstall::{Tier, TIERS};

mod choosing;
mod household;
mod migrating;
mod sharing;

use super::asked::{unwanted, Arguments, Disturbing};
use super::reading::{consent, diagnosing, following, listing, widening};
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
    "migrate-adopt",
    "reset",
    "forget",
    "uninstall",
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
    "hosting-install",
    "hosting-remove",
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
    // Apart for the same reason: seven fields no other row reads, and nothing here to
    // refuse — what a share or a window means is the core's answer for every surface.
    if sharing::about_the_line(action) {
        return Ok(sharing::asked_for(given));
    }
    // And again, twice: one field and nothing to refuse for the first, two fields and
    // one refusal for the second.
    if migrating::about_a_setup_already_here(action) {
        return Ok(migrating::asked_for(&given));
    }
    if choosing::about_the_quality(action) {
        return choosing::asked_for(action, &given);
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
        kept,
        tier,
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
        // Unconfirmed it is the listing `/api/uninstall` answers with, so what a
        // browser agrees to is what it was shown. Which removal is required: one
        // with none named has lost the only part of it that decides what goes.
        "uninstall" => removing(tier, confirm, offer, wait),
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
        // Which one is required of both halves, and a word naming none of them is
        // refused by name rather than taken for whichever came first in the list.
        "hosting-install" => {
            keeping(action, kept).map(|what| Command::Hosting(Keeping::Install { what, forms }))
        }
        "hosting-remove" => {
            keeping(action, kept).map(|what| Command::Hosting(Keeping::Remove { what }))
        }
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

/// Which long-running command was named, or why the word names none.
///
/// Named apart from the table for the reason the setting is: it can refuse, and a
/// reading that can refuse belongs beside its refusal rather than inside a list of
/// arms. What it offers is built from the list itself, so a command that becomes
/// Which of the four removals was asked for, and what was answered about it.
///
/// The removal is required: one with none named has lost the only part of it that
/// decides what goes, and defaulting it would mean a request that lost a word in
/// transit removing something nobody asked about. A word that names none of the four
/// is refused by name, with the four listed — for the reason a hostable is.
///
/// Nothing typed is nothing agreed to, which is why an empty name is dropped rather
/// than carried: a listing answered with an empty name is an answer to no listing.
fn removing(
    tier: Option<String>,
    confirm: bool,
    offer: Option<String>,
    wait: Waiting,
) -> Result<Command, Refused> {
    let Some(named) = tier else {
        return Err(Refused::Missing {
            action: "uninstall".to_owned(),
            argument: "tier".to_owned(),
        });
    };
    let chosen = Tier::named(&named).ok_or_else(|| Refused::Unrecognised {
        argument: named,
        offered: TIERS
            .iter()
            .map(|tier| tier.name())
            .collect::<Vec<_>>()
            .join(", "),
    })?;
    Ok(Command::Uninstall(
        Removing::surveying(chosen)
            .confirmed(confirm)
            .agreeing(offer.filter(|given| !given.trim().is_empty()))
            .waiting(wait),
    ))
}

/// hostable is offered here without anybody remembering to say so.
fn keeping(action: &str, kept: Option<String>) -> Result<Hostable, Refused> {
    let Some(named) = kept else {
        return Err(Refused::Missing {
            action: action.to_owned(),
            argument: "kept".to_owned(),
        });
    };
    Hostable::named(&named).ok_or_else(|| Refused::Unrecognised {
        argument: named,
        offered: HOSTABLE
            .iter()
            .map(|one| one.name())
            .collect::<Vec<_>>()
            .join(", "),
    })
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

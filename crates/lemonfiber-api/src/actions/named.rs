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
use lemonfiber_core::app::repair::Consent;
use lemonfiber_core::app::restore::{self, Kept};
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{Allowance, Command, QualityAction, Waiting};
use lemonfiber_core::audio::Format;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;

use super::asked::{unwanted, Arguments, Disturbing};
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
    "backup",
    "invite",
    "remove",
    "reissue",
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

/// The command one of the three household actions names.
///
/// All are addressed to a member rather than to a form, a file or a service, and all are
/// refused the same way when nobody is named — so the refusal is written once.
///
/// What they may watch reaches only the one that makes an account. The other two are
/// given it and drop it, which they may because the carrier refuses it to them first:
/// a library or an age limit named to a reissue or a removal is turned away by name
/// before this is reached.
fn about_a_person(
    action: &str,
    name: Option<String>,
    confirm: bool,
    libraries: Vec<String>,
    age_limit: Option<u32>,
) -> Result<Command, Refused> {
    let name = name.ok_or_else(|| Refused::Missing {
        action: action.to_owned(),
        argument: "name".to_owned(),
    })?;
    let allowance = Allowance {
        libraries,
        age_limit,
    };
    match action {
        "invite" => Ok(Command::Invite { name, allowance }),
        "reissue" => Ok(Command::Reissue { name }),
        _ => Ok(Command::Remove { name, confirm }),
    }
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
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    let Arguments {
        forms,
        services,
        wait,
        service,
        name,
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
        libraries,
        age_limit,
        term,
        season,
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
        "config-set" => match (key, value) {
            (Some(key), Some(value)) => Ok(Command::ConfigSet { key, value }),
            (None, _) => Err(needs("key")),
            (_, None) => Err(needs("value")),
        },
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
        "backup" => Ok(Command::Backup { service }),
        // The three addressed to a person rather than a form, a file or a service.
        // Unconfirmed, a removal says what it takes — their watch history, and every
        // request they made — and touches neither service, so what a browser agrees
        // to is what it was shown, the way a forget's agreement is. A reissue asks
        // for no agreement: nothing is destroyed and nothing is listed first, and
        // what ends is a password nobody here knew.
        "invite" | "reissue" | "remove" => {
            about_a_person(action, name, confirm, libraries, age_limit)
        }
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

/// What a diagnosis was narrowed to, or why the name it was given narrows to
/// nothing.
///
/// Naming none is the whole suite, which is the fork the command line takes on the
/// same flag. A name that is neither a group nor a check inside one is refused
/// rather than run as an empty suite, because a report of nothing found reads as
/// nothing wrong.
fn narrowed(only: Option<String>) -> Result<Narrowing, Refused> {
    let Some(named) = only else {
        return Ok(Narrowing::Suite);
    };
    Narrowing::parse(&named).ok_or_else(|| Refused::Unrecognised {
        argument: "only".to_owned(),
        offered: "try a group such as vpn, or one check by the name a finding gives it, \
                  such as vpn.killswitch"
            .to_owned(),
    })
}

/// What a repairing run was given consent for, or why it names none.
///
/// Three shapes, and the yes is what tells them apart. Without it the request is
/// the offer itself, which changes nothing and is what an operator reads before
/// deciding. With it and nothing else it is standing consent — the decision taken
/// in advance the command line spells `--yes`, which is a thing somebody does
/// deliberately rather than a default anybody falls into. With it and the offer it
/// was read in, it is consent given to that offer, for the repairs it names.
///
/// The rest are refused rather than read charitably. An agreement that does not say
/// which offer it answered cannot be checked against the offer that stands, and an
/// offer nobody agreed to any of is consent that has lost its subject — and both,
/// let through, would carry out a request nobody made.
fn consent(confirm: bool, offer: Option<String>, agreed: Vec<String>) -> Result<Consent, Refused> {
    let needs = |argument: &str| Refused::Missing {
        action: "repair".to_owned(),
        argument: argument.to_owned(),
    };
    match (confirm, offer, agreed.is_empty()) {
        (false, None, true) => Ok(Consent::Offer),
        (true, None, true) => Ok(Consent::Standing),
        (true, Some(offer), false) => Ok(Consent::Given {
            offer,
            repairs: agreed,
        }),
        (true, Some(_), true) => Err(needs("agreed")),
        (true, None, false) => Err(needs("offer")),
        (false, _, _) => Err(needs("confirm")),
    }
}

/// What a restoring run was given consent for, or why it names none.
///
/// The same three shapes a repair's consent has, and the same word tells them
/// apart. Without the yes the request is the listing itself, which overwrites
/// nothing and is what an operator reads before deciding. With it and the listing
/// it was read in, it is consent given to that listing. With it and nothing else it
/// is standing consent — the decision taken before there was a listing, which is
/// what a shell run with the agreement typed in advance is, and what a screen that
/// lists and asks inside one process is.
///
/// The fourth is refused rather than read charitably: naming the listing without
/// the yes is an answer to a question nobody was asked, and it would restore or not
/// restore depending on which half was believed.
///
/// Apart from [`consent`] because they are not the same decision. A repair's yes
/// also says *which* repairs, out of an offer of several; a restore has one archive
/// and nothing to choose out of it, so there is no `agreed` here to be missing.
fn listing(confirm: bool, offer: Option<String>) -> Result<restore::Consent, Refused> {
    match (confirm, offer) {
        (false, None) => Ok(restore::Consent::List),
        (true, None) => Ok(restore::Consent::Standing),
        (true, Some(listing)) => Ok(restore::Consent::Given { listing }),
        (false, Some(_)) => Err(Refused::Missing {
            action: "restore".to_owned(),
            argument: "confirm".to_owned(),
        }),
    }
}

/// The widening a read's second door requires, or the refusal for want of it.
///
/// Two actions here are the widened form of a read this surface also serves, and both
/// require the word rather than defaulting it: without it each is that read, and two
/// ways to ask one thing is the arrangement every read on this surface is kept out of.
/// Which read each widens is what the names say — `diagnose` over `/api/checks`, and
/// `search` over `/api/trace` — because one word answering at two doors is that same
/// arrangement wearing a disguise.
fn widening(action: &str, disruptive: Disturbing) -> Result<(), Refused> {
    if disruptive.included() {
        return Ok(());
    }
    Err(Refused::Missing {
        action: action.to_owned(),
        argument: "disruptive".to_owned(),
    })
}

/// The diagnosis a widened run asks for, narrowed as the reading it follows was.
fn diagnosing(only: Option<String>) -> Result<Command, Refused> {
    narrowed(only).map(|narrowing| Command::Doctor {
        narrowing,
        disruptive: true,
        accept: None,
    })
}

/// Following one item with the indexers asked, or why it follows nothing.
///
/// The term is required for the reason it is required of the read: a trace with
/// nothing to follow is a request that has lost its subject, and answering it with
/// everything would be answering a question nobody asked. Blank is nothing named
/// rather than an empty title, so a browser that sent the field and left it alone is
/// refused as one that left it out.
fn following(term: Option<String>, season: Option<u32>) -> Result<Command, Refused> {
    let term = term
        .filter(|term| !term.trim().is_empty())
        .ok_or_else(|| Refused::Missing {
            action: "search".to_owned(),
            argument: "term".to_owned(),
        })?;
    Ok(Command::Trace {
        term,
        season,
        searching: true,
    })
}

/// A quality choice, which is two commands depending on what it is about.
///
/// Music has no resolution: choosing for it picks an audio format rather than a
/// resolution preset, and reaches the service rather than only recording, so it
/// routes to its own command — the same fork the command line takes.
fn quality(preset: &str, media_type: Option<String>, confirm: bool) -> Result<Command, Refused> {
    if media_type.as_deref() == Some("music") {
        let Some(format) = Format::from_label(preset) else {
            return Err(Refused::Unrecognised {
                argument: "preset".to_owned(),
                offered: "try compact, lossless, or hi-res".to_owned(),
            });
        };
        return Ok(Command::QualityMusic { format });
    }
    let Some(preset) = Preset::from_label(preset) else {
        return Err(Refused::Unrecognised {
            argument: "preset".to_owned(),
            offered: "try space-saving, balanced, high-quality, or maximum".to_owned(),
        });
    };
    // Asked as one value and answered with one expression rather than through an
    // early return: a block that only ever ends by leaving leaves its own closing
    // brace with nothing to run it.
    let known = media_type
        .as_deref()
        .is_none_or(|named| Kind::ALL.iter().any(|kind| kind.media_type() == named));
    if known {
        Ok(Command::Quality(QualityAction::Set {
            preset,
            media_type,
            confirm,
        }))
    } else {
        Err(Refused::Unrecognised {
            argument: "media_type".to_owned(),
            offered: "try tv, movies, or music".to_owned(),
        })
    }
}

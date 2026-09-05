//! Reading one argument into the shape its command takes.
//!
//! Apart from the mapping next door because they are different questions. That one
//! is which command a name reaches; this is what one of its arguments has to *be*
//! before the command can take it, and what refusing it sounds like — a name that
//! narrows to nothing, a yes with nothing to be a yes to, a preset this build does
//! not know.
//!
//! Each of these refuses in its own words rather than returning nothing, because
//! what a caller can do about a word this surface does not recognise is spell it
//! differently, and a bare refusal does not tell them which word or what to spell
//! instead. They are gathered here so that the shape of a refusal is settled in one
//! place and the mapping beside them stays a mapping.

use lemonfiber_core::app::repair::Consent;
use lemonfiber_core::app::restore;
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::audio::Format;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;

use super::asked::Disturbing;
use super::Refused;

/// What a diagnosis was narrowed to, or why the name it was given narrows to
/// nothing.
///
/// Naming none is the whole suite, which is the fork the command line takes on the
/// same flag. A name that is neither a group nor a check inside one is refused
/// rather than run as an empty suite, because a report of nothing found reads as
/// nothing wrong.
pub(super) fn narrowed(only: Option<String>) -> Result<Narrowing, Refused> {
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
pub(super) fn consent(
    confirm: bool,
    offer: Option<String>,
    agreed: Vec<String>,
) -> Result<Consent, Refused> {
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
pub(super) fn listing(confirm: bool, offer: Option<String>) -> Result<restore::Consent, Refused> {
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
pub(super) fn widening(action: &str, disruptive: Disturbing) -> Result<(), Refused> {
    if disruptive.included() {
        return Ok(());
    }
    Err(Refused::Missing {
        action: action.to_owned(),
        argument: "disruptive".to_owned(),
    })
}

/// The diagnosis a widened run asks for, narrowed as the reading it follows was.
pub(super) fn diagnosing(only: Option<String>) -> Result<Command, Refused> {
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
pub(super) fn following(term: Option<String>, season: Option<u32>) -> Result<Command, Refused> {
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
pub(super) fn quality(
    preset: &str,
    media_type: Option<String>,
    confirm: bool,
) -> Result<Command, Refused> {
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

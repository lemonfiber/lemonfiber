//! What one read's parameters come to, a function per read that takes any.
//!
//! Apart from the table beside it because they are two jobs. That one says which
//! command a path names, one line a read; this says what the values on it mean — and
//! every one of these is a decision rather than a lookup: naming none and naming an
//! empty one are different requests, a word that is none of four is refused rather
//! than read as the safest, and a count outside what a read answers with is turned
//! away rather than rounded into range.
//!
//! The reads that take nothing have no function here, which is the shape of the file
//! rather than an omission: there is nothing for them to mean.

use lemonfiber_core::app::{Command, Diagnosing, Removing, Tracing, Waiting, Whom};
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::uninstall::Tier;
use lemonfiber_core::update::run as update;

use crate::refusal::Refusal;

/// The object of an update read that is this binary.
pub(crate) const THIS_BINARY: &str = "self";

/// The object of an update read that is the stack it operates.
pub(crate) const THE_STACK: &str = "stack";

/// The removal a name asks for, read and nothing more.
///
/// Naming none reads the one that removes nothing, which is the safe reading and the
/// one a browser opening the page has not chosen anything by.
pub(super) fn removing(tier: Option<String>) -> Result<Command, Refusal> {
    let Some(named) = tier else {
        return Ok(Command::Uninstall(Removing::surveying(Tier::Stop)));
    };
    Tier::named(&named)
        .map(|tier| Command::Uninstall(Removing::surveying(tier)))
        .ok_or(Refusal::NoSuchRemoval)
}

/// Which of the two things that can be moved forward was asked about.
///
/// Naming a version asks the binary about that one and naming none asks about whatever
/// is newest, which is the fork the command line takes on the same word. Neither half
/// replaces anything, so both are reads.
pub(super) fn moving(what: Option<&str>, to: Option<String>) -> Result<Command, Refusal> {
    match what {
        Some(THIS_BINARY) => Ok(Command::SelfUpdate { to }),
        Some(THE_STACK) => Ok(Command::Update(update::Asked {
            service: None,
            confirm: false,
            wait: Waiting::Never,
            offer: None,
        })),
        _ => Err(Refusal::NoUpdateObject),
    }
}

/// A diagnosis, narrowed or whole.
///
/// A read looks and does not touch, so it neither accepts a warning nor opts into
/// the checks that disturb a running system; both of those change something.
pub(super) const fn diagnosing(narrowing: Narrowing) -> Command {
    Command::Doctor(Diagnosing {
        narrowing,
        disruptive: false,
        accept: None,
    })
}

/// The diagnosis a request asked for, or nothing where it named a group of checks
/// that is not one lemonfiber knows.
pub(super) fn narrowed(only: Option<&str>) -> Option<Command> {
    match only {
        None => Some(diagnosing(Narrowing::Suite)),
        Some(name) => Narrowing::parse(name).map(diagnosing),
    }
}

/// Every setting, or the one that was named.
///
/// Naming none and naming an empty one are different requests here, which is why
/// this cannot do what a restore does with a name it was given none of and read the
/// empty one as absent: absent already means every setting, so an empty name read
/// that way would answer a question nobody asked. It is refused instead — and
/// refused here rather than at whichever surface supplied it, so a line typed at a
/// screen and a query string arriving empty are answered in the same sentence.
///
/// What the refusal is worth is the reading it keeps out: an empty name carried to the
/// core matches nothing and comes back as a listing of no settings, which reads as
/// "there is no such setting" about a setting nobody named.
pub(super) fn setting(key: Option<String>) -> Result<Command, Refusal> {
    match key {
        None => Ok(Command::ConfigShow),
        Some(key) if key.is_empty() => Err(Refusal::NoSetting),
        Some(key) => Ok(Command::ConfigGet { key }),
    }
}

/// Whether a parameter that takes a yes or a no said one, or nothing where it said
/// something that is neither.
///
/// Not given is not asked for. A word that is neither is a mistake to correct rather
/// than a request to answer as though it had said no, because the two answers are
/// different and a caller that meant yes would read one it never asked for.
pub(crate) fn told(said: Option<&str>) -> Option<bool> {
    match said {
        // Not given is not asked for, which is the same answer as having said so.
        None | Some("false") => Some(false),
        Some("true") => Some(true),
        Some(_) => None,
    }
}

/// Whom a household read is answered as, where it asked for the household's defaults.
///
/// Nothing where it did not, so the member it named, or nobody, decides. Naming a
/// member as well is refused rather than either half dropped: the defaults are nobody,
/// and a read asking about somebody and nobody at once has two answers.
fn as_the_defaults(member: Option<&String>, defaults: Option<&str>) -> Result<bool, Refusal> {
    match told(defaults) {
        None => Err(Refusal::NotAChoice),
        Some(true) if member.is_some() => Err(Refusal::MemberAndDefaults),
        Some(defaults) => Ok(defaults),
    }
}

/// What the household asked for, narrowed to one member or taken whole — or what
/// somebody invited with the household's defaults would be told.
///
/// Empty is refused for the reason it is refused of a setting: a member nobody named
/// would match nobody, and a report of no requests reads as "nobody has asked for
/// anything" — which is exactly the reading
/// [`lemonfiber_core::app`]'s own household reader refuses to produce when it cannot
/// reach the request service.
pub(super) fn household(
    member: Option<String>,
    defaults: Option<&str>,
) -> Result<Command, Refusal> {
    if as_the_defaults(member.as_ref(), defaults)? {
        return Ok(Command::Household {
            member: Some(Whom::Defaults),
        });
    }
    match member {
        None => Ok(Command::Household { member: None }),
        Some(member) if member.is_empty() => Err(Refusal::NoMember),
        Some(member) => Ok(Command::Household {
            member: Some(Whom::Named(member)),
        }),
    }
}

/// What is playing, for one member where one is named, or for the whole house.
///
/// An empty name is refused rather than read as having named nobody, which would
/// answer with every session where one person's was asked for.
pub(super) fn watching(member: Option<String>) -> Result<Command, Refusal> {
    match member {
        Some(member) if member.is_empty() => Err(Refusal::NoMember),
        member => Ok(Command::Playing { member }),
    }
}

/// How many holdings a shelf answers with where the caller named no number.
///
/// Enough to fill a screen and scroll through it, and small enough that a phone on a
/// household's own network is not waiting on a page it will not draw.
pub const A_SHELF: u32 = 100;

/// The most a single shelf read answers with.
///
/// There is a ceiling because the alternative is a member's phone asking for forty
/// thousand items, and a bound that nobody chose is a bound that turns up as a
/// timeout rather than as a sentence.
pub const MOST_AT_ONCE: u32 = 500;

/// Whose shelf, and how much of it — or why the request cannot be answered.
///
/// Naming nobody is refused rather than read as everybody, because there is no
/// everybody: the shelf is what one account may watch and no two accounts need have
/// the same one. The household's defaults are the one shelf that is nobody's.
pub(super) fn shelf(
    member: Option<String>,
    defaults: Option<&str>,
    most: Option<String>,
) -> Result<Command, Refusal> {
    let member = if as_the_defaults(member.as_ref(), defaults)? {
        Whom::Defaults
    } else {
        match member.filter(|member| !member.is_empty()) {
            Some(member) => Whom::Named(member),
            None => return Err(Refusal::NoShelfWithoutAMember),
        }
    };
    let most = match most.map(|most| most.parse::<u32>()) {
        None => A_SHELF,
        Some(Ok(most)) if most > MOST_AT_ONCE => return Err(Refusal::TooManyAtOnce),
        Some(Ok(most)) if most > 0 => most,
        // Nought and anything that is not a number at all. A shelf of no holdings is a
        // request for an answer that says nothing, and an empty shelf is a fact about a
        // household rather than a thing a count should be able to manufacture.
        Some(_) => return Err(Refusal::NotACount),
    };
    Ok(Command::Held { member, most })
}

/// Following one item, or why the request could not be followed.
///
/// The term is one value rather than several. The command line takes it as words so
/// it can be typed without quoting and joins them back into the title as said; every
/// other surface carries the title already whole.
///
/// Nothing is searched. A read looks and does not touch, and asking the indexers what
/// they carry spends a live search against the allowance they hold the operator to —
/// so the widened form of this is an action, at the door changes are asked for.
pub(super) fn following(term: Option<String>, season: Option<&str>) -> Result<Command, Refusal> {
    let Some(term) = term.filter(|term| !term.is_empty()) else {
        return Err(Refusal::NoTerm);
    };
    let Ok(season) = season.map(str::parse::<u32>).transpose() else {
        return Err(Refusal::NotASeason);
    };
    Ok(Command::Trace(Tracing {
        term,
        season,
        searching: false,
    }))
}

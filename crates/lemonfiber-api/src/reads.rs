//! What a read request asks for, and which command answers it.
//!
//! A read is named by the path it is served at, and the name is turned into one of
//! the core's own commands. That translation is the read half of what
//! [`crate::actions`] does for the writes, and it is here rather than inside each
//! endpoint for the same reason: a surface that assembled a command of its own
//! could ask for something no other surface can ask for.
//!
//! The endpoints and the commands do not count the same, and the mismatch runs both
//! ways. `status` and `services` are one reading of what is running, whole and
//! narrowed; `storage` is the group of checks the narrowing parameter also reaches.
//! `forms`, `config` and `explain` go the other way and are one name over two
//! commands each, because the command line spells each of them as one request that
//! forks on whether something was named. `backups` is the same fork read from the
//! other side: `lemonfiber restore` with nothing named lists what there is to restore
//! from, and this is the half of that word a browser asks for on its own, because the
//! other half is a write and writes are asked for elsewhere. `front-door` counts the
//! plainest way of all — one name, one command, and no parameter to fork on.
//!
//! Only the flags that read are here. Narrowing a diagnosis is a parameter; running
//! the checks that disturb a running system and accepting a warning both change
//! something and belong where changes are asked for.
//!
//! Two reads have no command below, because they reach none. `/api/logs` opens a
//! stream and renders a document per line, or hands back a name for a follow that
//! will not end; `/api/bundle/{name}` answers with a file, which no envelope holds.
//! Both are named here all the same, because both are asked at a door this table
//! guards and [`asked`] holds every read to the parameters it takes — including the
//! ones that take none.

mod asked;

use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::doctor::{Category, Narrowing};
use lemonfiber_core::error::Problem;

pub(crate) use asked::{Asked, FOLLOW, FORM, SERVICE, TAIL};

/// The versions in play: this binary, the stack it operates, and the engine's.
pub const VERSION: &str = "/api/version";

/// Every form the stack declares, or what naming some of them would come to.
pub const FORMS: &str = "/api/forms";

/// What the whole stack is doing.
pub const STATUS: &str = "/api/status";

/// What each service is doing, narrowed to the forms that were named.
pub const SERVICES: &str = "/api/services";

/// What the diagnostic checks found, or one group of them.
pub const CHECKS: &str = "/api/checks";

/// What the checks about the disk found.
pub const STORAGE: &str = "/api/storage";

/// Who is in the household, what each may watch, and what each has asked for.
///
/// The path keeps the name it was published under. What it answers with is described
/// by the contract rather than by the path, and a published path renamed outruns its
/// own redirect.
pub const REQUESTS: &str = "/api/requests";

/// The one address to hand somebody who lives here.
pub const FRONT_DOOR: &str = "/api/front-door";

/// Where one item is, followed by the words a person would name it with.
pub const TRACE: &str = "/api/trace";

/// The items whose downloads have stopped.
pub const STUCK: &str = "/api/stuck";

/// Every setting, or one of them by name, with credentials withheld.
pub const CONFIG: &str = "/api/config";

/// The quality choice in force, what each preset means, and what it costs.
pub const QUALITY: &str = "/api/quality";

/// What one of this product's words means, or every word there is to ask about.
pub const EXPLAIN: &str = "/api/explain";

/// Everything that leaves this machine, and what refusing each of it costs.
///
/// Named for what it lists rather than for the direction of any one request: what
/// an operator wants from this page is the whole of what goes out, lemonfiber's own
/// and the stack's, told apart.
pub const OUTBOUND: &str = "/api/outbound";

/// Everything lemonfiber keeps on this machine, where each thing is and why.
///
/// The read a browser is least able to answer for itself: a page has no filesystem
/// in front of it and cannot see the host at all.
pub const STORED: &str = "/api/stored";

/// Which app to watch on, for each kind of device somebody in the house has.
///
/// The same answer on every machine — the client landscape belongs to the
/// platforms rather than to this stack — which is why it takes nothing and reads
/// nothing. A browser wants it because the person deciding is often the one
/// already looking at a screen.
pub const CLIENTS: &str = "/api/clients";

/// The backup archives this machine has kept, by the names they were written under.
///
/// Named for what it lists rather than for the command it reaches: what these are
/// to an operator is their backups, and what they are to the core is the archives
/// it keeps. The listing is the half of a restore that comes before naming one — a
/// browser has no filesystem to look in, so a name it could not be told is a name
/// it cannot use.
pub const BACKUPS: &str = "/api/backups";

/// What the services are saying, one document a line — or, where it was asked to
/// keep reading, a name for work that will not end and lines that arrive elsewhere.
pub const LOGS: &str = "/api/logs";

/// One support bundle this run kept, handed over whole.
///
/// The other read with no command below, and for a plainer reason than the logs
/// have: what it answers with is a file rather than a value, and no envelope holds
/// one. Which file is the core's to decide, from the name in this path and the
/// directory it keeps bundles in.
///
/// The name is a path segment rather than a parameter, so a browser saving the
/// answer reads the name off the address it asked at — and this surface never has
/// to quote a name a request supplied into a header.
pub const BUNDLE: &str = "/api/bundle/{name}";

/// The reads a name reaches, in the order the endpoints declare them.
///
/// What another surface may ask for by name. A surface reaching past this list
/// would be asking the core something no browser can ask it, which is the whole
/// arrangement this exists to prevent.
pub const OFFERED: &[&str] = &[
    VERSION, FORMS, STATUS, SERVICES, CHECKS, STORAGE, REQUESTS, FRONT_DOOR, TRACE, STUCK, CONFIG,
    QUALITY, EXPLAIN, BACKUPS, OUTBOUND, STORED, CLIENTS,
];

/// What is said to a request that named nothing to follow.
pub const NO_TERM: &str = "What to follow must be named.";

/// What is said to a request whose season is not a number.
pub const NOT_A_SEASON: &str = "Which season to narrow to must be a number.";

/// What is said to a request that named no setting to read.
pub const NO_SETTING: &str = "Which setting to read must be named.";

/// What is said to a request that named no household member to narrow to.
pub const NO_MEMBER: &str = "Which member to narrow to must be named.";

/// What is said to a request naming a group of checks that is not one.
pub const NO_SUCH_GROUP: &str = "There is no group of checks and no check by that name.";

/// What is said where no read goes by the name that was asked for.
pub const NO_SUCH_READ: &str = "There is no read by that name.";

/// What a read was given, mirroring the flags its command takes.
///
/// One carrier rather than one shape per read, so a caller fills the field the read
/// names and leaves the rest. Each field is the query parameter of the same
/// meaning, taken as it was written — the parsing a season needs is done here, so
/// a request that named one badly is refused in the same words wherever it arrived.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The forms to narrow to, or to say what starting would come to.
    pub forms: Vec<String>,
    /// The household member to narrow to.
    pub member: Option<String>,
    /// What to follow, named as a person would say it.
    pub term: Option<String>,
    /// The season to narrow a trace to, as it was written.
    pub season: Option<String>,
    /// The setting to read, instead of every one of them.
    pub key: Option<String>,
    /// The group of checks, or the one check, to narrow a diagnosis to.
    pub only: Option<String>,
    /// The word to explain, instead of every word there is to ask about.
    pub word: Option<String>,
}

/// What a read was given, or why the request cannot be read as it stands.
///
/// The one door a query string goes through. Which parameters a read takes is
/// [`asked`]'s, named by the path the read is served at, so a read that takes none
/// refuses a parameter by the same rule as one that takes three.
///
/// # Errors
///
/// Returns the refusal a caller is answered with: a parameter this read does not
/// take, or one it takes once and was given twice.
pub fn wanted(read: &str, query: Option<&str>) -> Result<Wanted, Box<Problem>> {
    Asked::read(read, query).map(|asked| asked.wanted())
}

/// The command a read names, or what to say to a request that reaches none.
///
/// # Errors
///
/// Returns the one line a caller is answered with.
pub fn named(read: &str, given: Wanted) -> Result<Command, &'static str> {
    let Wanted {
        forms,
        member,
        term,
        season,
        key,
        only,
        word,
    } = given;
    match read {
        VERSION => Ok(Command::Version),
        // Naming none lists what the stack declares and naming some says what
        // starting those would come to, which is the fork `lemonfiber forms` takes
        // on the same word.
        FORMS if forms.is_empty() => Ok(Command::Forms),
        FORMS => Ok(Command::Preview { forms }),
        STATUS => Ok(Command::Ps { forms: Vec::new() }),
        SERVICES => Ok(Command::Ps { forms }),
        CHECKS => narrowed(only.as_deref()).ok_or(NO_SUCH_GROUP),
        STORAGE => Ok(diagnosing(Narrowing::Category(Category::Storage))),
        REQUESTS => household(member),
        FRONT_DOOR => Ok(Command::FrontDoor),
        TRACE => following(term, season.as_deref()),
        STUCK => Ok(Command::Stuck),
        CONFIG => setting(key),
        QUALITY => Ok(Command::Quality(QualityAction::Show)),
        // Naming an empty word is naming one this product does not explain, and is
        // refused for that by the command rather than read as having named none.
        EXPLAIN => Ok(word.map_or(Command::Glossary, |word| Command::Explain { word })),
        BACKUPS => Ok(Command::Archives),
        OUTBOUND => Ok(Command::Outbound),
        STORED => Ok(Command::Stored),
        CLIENTS => Ok(Command::Clients),
        _ => Err(NO_SUCH_READ),
    }
}

/// A diagnosis, narrowed or whole.
///
/// A read looks and does not touch, so it neither accepts a warning nor opts into
/// the checks that disturb a running system; both of those change something.
const fn diagnosing(narrowing: Narrowing) -> Command {
    Command::Doctor {
        narrowing,
        disruptive: false,
        accept: None,
    }
}

/// The diagnosis a request asked for, or nothing where it named a group of checks
/// that is not one lemonfiber knows.
fn narrowed(only: Option<&str>) -> Option<Command> {
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
/// Before this, an empty one reached the core as a setting to look for, matched
/// nothing, and came back as a listing of no settings — which reads as "there is no
/// such setting" about a setting nobody named.
fn setting(key: Option<String>) -> Result<Command, &'static str> {
    match key {
        None => Ok(Command::ConfigShow),
        Some(key) if key.is_empty() => Err(NO_SETTING),
        Some(key) => Ok(Command::ConfigGet { key }),
    }
}

/// What the household asked for, narrowed to one member or taken whole.
///
/// Empty is refused for the reason it is refused of a setting, and the answer it
/// used to give was worse: a member nobody named matched nobody, and a report of no
/// requests reads as "nobody has asked for anything" — which is exactly the reading
/// [`lemonfiber_core::app`]'s own household reader refuses to produce when it cannot
/// reach the request service.
fn household(member: Option<String>) -> Result<Command, &'static str> {
    match member {
        None => Ok(Command::Household { member: None }),
        Some(member) if member.is_empty() => Err(NO_MEMBER),
        Some(member) => Ok(Command::Household {
            member: Some(member),
        }),
    }
}

/// Following one item, or why the request could not be followed.
///
/// The term is one value rather than several. The command line takes it as words so
/// it can be typed without quoting and joins them back into the title as said; every
/// other surface carries the title already whole.
fn following(term: Option<String>, season: Option<&str>) -> Result<Command, &'static str> {
    let Some(term) = term.filter(|term| !term.is_empty()) else {
        return Err(NO_TERM);
    };
    let Ok(season) = season.map(str::parse::<u32>).transpose() else {
        return Err(NOT_A_SEASON);
    };
    Ok(Command::Trace { term, season })
}

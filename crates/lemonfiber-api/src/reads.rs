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
//! forks on whether something was named.
//!
//! Only the flags that read are here. Narrowing a diagnosis is a parameter; running
//! the checks that disturb a running system and accepting a warning both change
//! something and belong where changes are asked for.
//!
//! `/api/logs` is the one read with no command below, because it reaches none: it
//! opens a stream and renders a document per line, or hands back a name for a follow
//! that will not end — a different shape of answer rather than a different answer. It
//! is named here all the same, because it is asked with parameters like every other
//! read and [`asked`] holds every read to the parameters it takes.

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

/// What the household has asked for, and where each request stands.
pub const REQUESTS: &str = "/api/requests";

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

/// What the services are saying, one document a line — or, where it was asked to
/// keep reading, a name for work that will not end and lines that arrive elsewhere.
pub const LOGS: &str = "/api/logs";

/// The reads a name reaches, in the order the endpoints declare them.
///
/// What another surface may ask for by name. A surface reaching past this list
/// would be asking the core something no browser can ask it, which is the whole
/// arrangement this exists to prevent.
pub const OFFERED: &[&str] = &[
    VERSION, FORMS, STATUS, SERVICES, CHECKS, STORAGE, REQUESTS, TRACE, STUCK, CONFIG, QUALITY,
    EXPLAIN,
];

/// What is said to a request that named nothing to follow.
pub const NO_TERM: &str = "What to follow must be named.";

/// What is said to a request whose season is not a number.
pub const NOT_A_SEASON: &str = "Which season to narrow to must be a number.";

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
        REQUESTS => Ok(Command::Household { member }),
        TRACE => following(term, season.as_deref()),
        STUCK => Ok(Command::Stuck),
        CONFIG => Ok(key.map_or(Command::ConfigShow, |key| Command::ConfigGet { key })),
        QUALITY => Ok(Command::Quality(QualityAction::Show)),
        // Naming an empty word is naming one this product does not explain, and is
        // refused for that by the command rather than read as having named none.
        EXPLAIN => Ok(word.map_or(Command::Glossary, |word| Command::Explain { word })),
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

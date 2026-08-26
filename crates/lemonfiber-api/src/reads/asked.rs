//! The parameters a read was given, and which reads take which.
//!
//! One carrier for every read rather than one shape per read, so a handler fills the
//! parameter its read names and leaves the rest. A name the read does not take is
//! refused rather than ignored, which is the same judgement [`crate::actions::asked`]
//! makes about a field the write surface's carrier does not hold.
//!
//! A query string is a different door from a JSON body, and the reads arrived at that
//! door with nothing on it. An operator who wrote `keys` where `key` was meant asked
//! for one setting and was handed the whole configuration — values withheld, but every
//! setting this stack has and what each is called. Widening a request is worse than
//! refusing it, because a refusal is read and a wider answer looks like the answer.
//!
//! Which read takes what is a table rather than a check per handler. The reads that
//! take nothing at all are the ones a per-handler check would never have covered:
//! there was no query string to look at, so there was nowhere to write the check.
//!
//! A parameter carrying one value and given twice is refused too. Taking the first
//! drops the rest, and a request that named two things to follow and was answered
//! about one of them has been answered about something it did not ask.

use lemonfiber_core::error::{Amiss, Code, Problem, Remedy, Severity};

use super::{
    Wanted, BACKUPS, BUNDLE, CHECKS, CONFIG, EXPLAIN, FORMS, FRONT_DOOR, LOGS, QUALITY, REQUESTS,
    SERVICES, STATUS, STORAGE, STUCK, TRACE, VERSION,
};

/// Raised where a read was given a parameter its answer has nowhere to put.
const UNWANTED: Code = Code::new("READ-1");

/// Raised where a parameter carrying one value was given more than once.
const REPEATED: Code = Code::new("READ-2");

/// The parameter naming a form to narrow to.
pub(crate) const FORM: &str = "form";

/// The parameter naming a service to read.
pub(crate) const SERVICE: &str = "service";

/// The parameter saying how many existing log lines to begin with.
pub(crate) const TAIL: &str = "tail";

/// The parameter asking to keep reading as new lines arrive.
pub(crate) const FOLLOW: &str = "follow";

/// The parameter naming the household member to narrow to.
const MEMBER: &str = "member";

/// The parameter naming what to follow.
const TERM: &str = "term";

/// The parameter naming the season to narrow the per-part coverage to.
const SEASON: &str = "season";

/// The parameter naming one setting to read, instead of all of them.
const KEY: &str = "key";

/// The parameter naming what the checks are narrowed to.
const ONLY: &str = "only";

/// The parameter naming the word to explain.
const WORD: &str = "word";

/// What each read takes, and nothing else.
///
/// In the order the endpoints declare them, so this reads beside the routes rather
/// than against them. A read with an empty list takes no parameter at all, which is
/// a row worth writing: an absent row and an empty one mean the same thing here on
/// purpose, so a read added without one refuses everything rather than accepting
/// everything.
///
/// `/api/logs` and `/api/bundle/{name}` have rows here and none among the commands.
/// Neither reaches one — the first opens a stream and renders a document per line,
/// the second answers with a file — but both are asked at this door like every other
/// read, and a read exempt from this table would be the one place the refusal did
/// not reach. What the bundle is named by is a path segment rather than a parameter,
/// so what its empty row refuses is a query string it has no use for at all.
const TAKEN: &[(&str, &[&str])] = &[
    (VERSION, &[]),
    (FORMS, &[FORM]),
    (STATUS, &[]),
    (SERVICES, &[FORM]),
    (LOGS, &[FORM, SERVICE, TAIL, FOLLOW]),
    (CHECKS, &[ONLY]),
    (STORAGE, &[]),
    (REQUESTS, &[MEMBER]),
    (FRONT_DOOR, &[]),
    (TRACE, &[TERM, SEASON]),
    (STUCK, &[]),
    (CONFIG, &[KEY]),
    (QUALITY, &[]),
    (EXPLAIN, &[WORD]),
    (BACKUPS, &[]),
    (BUNDLE, &[]),
];

/// The parameters that name one of several rather than one thing.
///
/// Two, and they are the two the commands behind them take as lists: a log read is
/// narrowed to as many forms and as many services as were named. Every other
/// parameter names one thing, and naming a second is a request with two answers.
const REPEATABLE: &[&str] = &[FORM, SERVICE];

/// What a read with no row takes.
const NOTHING: &[&str] = &[];

/// What a request asked for, read from its query string.
///
/// Read here rather than through an extractor, because the router carries no query
/// parser: what this surface takes are the flags the commands take, and a flag a
/// command accepts more than once is a name given more than once.
pub(crate) struct Asked(Vec<(String, String)>);

impl Asked {
    /// The pairs a query string holds, decoded, or why the request cannot be read.
    ///
    /// # Errors
    ///
    /// Returns the refusal a caller is answered with: a parameter this read does not
    /// take, or one it takes once and was given twice. Both lie in how the request
    /// asked rather than in what it named, so both are answered 400 by the one place
    /// that decides what a refusal's status is.
    pub(crate) fn read(read: &str, query: Option<&str>) -> Result<Self, Box<Problem>> {
        let given: Vec<(String, String)> =
            form_urlencoded::parse(query.unwrap_or_default().as_bytes())
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect();
        let takes = taken(read);
        for (at, (name, _)) in given.iter().enumerate() {
            if !takes.contains(&name.as_str()) {
                return Err(Box::new(unwanted(read, name, takes)));
            }
            if !REPEATABLE.contains(&name.as_str())
                && given.iter().take(at).any(|(earlier, _)| earlier == name)
            {
                return Err(Box::new(repeated(read, name)));
            }
        }
        Ok(Self(given))
    }

    /// The value given for a name, or nothing where it was not given.
    ///
    /// One value, because a name that reached here is one this read takes once.
    pub(crate) fn one(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(given, _)| given == name)
            .map(|(_, value)| value.as_str())
    }

    /// Every value given for a name, in the order they were given.
    pub(crate) fn every(&self, name: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(given, _)| given == name)
            .map(|(_, value)| value.clone())
            .collect()
    }

    /// The flags these parameters come to.
    ///
    /// Filled the same way for every read, because which parameters a read takes has
    /// already been decided: a field left empty here is one the read has no name for,
    /// and a request that named it was refused before this ran.
    pub(crate) fn wanted(&self) -> Wanted {
        Wanted {
            forms: self.every(FORM),
            member: self.one(MEMBER).map(str::to_owned),
            term: self.one(TERM).map(str::to_owned),
            season: self.one(SEASON).map(str::to_owned),
            key: self.one(KEY).map(str::to_owned),
            only: self.one(ONLY).map(str::to_owned),
            word: self.one(WORD).map(str::to_owned),
        }
    }
}

/// What a read takes, or nothing where no row names it.
fn taken(read: &str) -> &'static [&'static str] {
    TAKEN
        .iter()
        .find(|(named, _)| *named == read)
        .map_or(NOTHING, |(_, takes)| takes)
}

/// A parameter this read has nowhere to put, refused rather than dropped.
///
/// What the read does take is the way forward, because a caller here has misspelled
/// something and the list is short enough to be the answer rather than a pointer at
/// one. What they wrote is repeated back so they can see which of the two it was.
fn unwanted(read: &str, parameter: &str, takes: &[&str]) -> Problem {
    Problem::new(
        UNWANTED,
        Severity::Error,
        format!("The read `{read}` takes no `{parameter}`"),
        "It is refused rather than dropped, because dropping it would answer a wider \
         question than the one that was asked — and a wider answer reads like the answer.",
        Remedy::new("Ask again, naming only what this read takes"),
    )
    .lies_in(Amiss::Asking)
    .with_detail(taking(takes))
}

/// A parameter that names one thing, given more than once.
fn repeated(read: &str, parameter: &str) -> Problem {
    Problem::new(
        REPEATED,
        Severity::Error,
        format!("The read `{read}` takes one `{parameter}`, and it was given more than once"),
        "Which of them was meant is not something this can work out, and answering for \
         one of them would drop the others without saying so.",
        Remedy::new("Ask again, naming it once"),
    )
    .lies_in(Amiss::Asking)
}

/// What a read takes, said as a sentence.
fn taking(takes: &[&str]) -> String {
    if takes.is_empty() {
        return "This read takes no parameters at all.".to_owned();
    }
    format!("It takes {}.", takes.join(", "))
}

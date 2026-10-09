//! Every read this surface serves, as the contract lists it.
//!
//! A client generating a method per read needs three things the kinds alone do not
//! say: where each read is asked, what it may be given, and what comes back. Each is
//! read off what the surface already routes and refuses by — the paths and the
//! parameters from the table every request is held to, and what comes back from the
//! core's own account of the command each read reaches — so a read the surface gains
//! is a read every client generates, and none is written down a second time here.

use serde::Serialize;

use lemonfiber_core::app::answered_under;
use lemonfiber_core::model::kind::Kind;

use super::logs;
use super::table::{self, taken, Wanted, BUNDLE, LOGS, OFFERED, REPEATABLE, THIS_BINARY};

/// One read, as the contract lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Read {
    /// The path it is served at, `GET` and nothing else.
    pub path: &'static str,
    /// What it may be given, in the order the table names them. Empty for a read that
    /// takes nothing, which refuses any parameter at all.
    pub parameters: Vec<Parameter>,
    /// Every kind its answer may be written under, by name. Empty for a read that
    /// answers with a file.
    pub kinds: Vec<&'static str>,
    /// Whether it answers with a file, which no envelope holds, rather than a kind.
    pub file: bool,
}

/// One query parameter a read takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parameter {
    /// Its name, as the query string spells it.
    pub name: &'static str,
    /// Whether it may be given more than once, each naming one more thing.
    pub repeatable: bool,
}

/// Every read this surface serves: the table's own, in the order it names them, then
/// the stream and the file the modules beside it answer.
#[must_use]
pub fn every() -> Vec<Read> {
    OFFERED
        .iter()
        .chain(BESIDE)
        .map(|path| Read {
            path,
            parameters: taken(path)
                .iter()
                .map(|name| Parameter {
                    name,
                    repeatable: REPEATABLE.contains(name),
                })
                .collect(),
            kinds: answering(path).iter().map(|kind| kind.as_str()).collect(),
            file: *path == BUNDLE,
        })
        .collect()
}

/// The reads served beside the table's own, by the modules that answer them: the logs,
/// which open a stream, and the bundle, which is a file.
pub(crate) const BESIDE: &[&str] = &[LOGS, BUNDLE];

/// Every kind a read answers under, by name.
///
/// The logs read reaches no command, and says what it answers with itself; the bundle
/// answers with a file. Every other read is asked once for each way it forks, and
/// answers under whatever the commands those reach answer under.
fn answering(read: &str) -> Vec<Kind> {
    let mut kinds: Vec<Kind> = if read == LOGS {
        logs::ANSWERED_UNDER.to_vec()
    } else {
        forking()
            .into_iter()
            .filter_map(|given| table::named(read, given).ok())
            .flat_map(|command| answered_under(&command).iter().copied())
            .collect()
    };
    kinds.sort_by_key(|kind| kind.as_str());
    kinds.dedup();
    kinds
}

/// The requests that between them reach every command a read can: the plainest that
/// names one, and one for each parameter that sends a read to a second command.
///
/// Named from what the parameters are rather than from any stack, because which
/// command a read reaches is decided by its path and these, never by what a service
/// or a setting is called.
fn forking() -> [Wanted; 5] {
    let plainest = Wanted::naming_everything();
    [
        Wanted {
            forms: vec![ANY.to_owned()],
            ..plainest.clone()
        },
        Wanted {
            key: Some(ANY.to_owned()),
            ..plainest.clone()
        },
        Wanted {
            word: Some(ANY.to_owned()),
            ..plainest.clone()
        },
        Wanted {
            what: Some(THIS_BINARY.to_owned()),
            ..plainest.clone()
        },
        plainest,
    ]
}

/// A value standing for whichever one a person would give, where only whether one was
/// given decides the command.
const ANY: &str = "any";

#[cfg(test)]
mod tests;

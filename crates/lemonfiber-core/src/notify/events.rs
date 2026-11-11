//! What a curator or the media server tells the core happened, taken only with the key the
//! core handed that one service, and told as the household chose to be told.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::NOTIFY_EVENTS;
use crate::error::codes::notify::EVENTS_REFUSED;
use crate::error::Severity;
use crate::error::{Problem, Remedy, State};
use crate::ports::digest::sha256;

/// The most of anything a service says that is passed on.
const SAID_MOST: usize = 200;

/// The digest of the key each service tells the core with, by the service's id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Keyed {
    /// Each service's key, as its SHA-256.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

/// What happened, as a service tells it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NotifyEvent")]
pub struct Event {
    /// Which of the four happenings it is.
    pub event: Happened,
    /// The title it is about, by the service's own reference, where it is about one.
    #[serde(default)]
    pub reference: Option<String>,
    /// The title's name, or the service's own words for a problem about itself.
    pub name: String,
    /// What the service said went wrong, where something did.
    #[serde(default)]
    pub reason: Option<String>,
    /// Whether a problem the service raised about itself is cleared.
    #[serde(default)]
    pub cleared: bool,
}

/// One of the four happenings a service tells the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyHappened")]
pub enum Happened {
    /// A title a curator fetched is imported into the library.
    Imported,
    /// A grab or an import failed.
    Failed,
    /// The service raised a problem about itself, or cleared one.
    Health,
    /// A title was added to a library of the media server.
    Added,
}

pub(crate) use reached::{address, Told};
pub use reached::{HOST, PATH};

/// Where the keys are kept, where anywhere is configured.
fn at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, NOTIFY_EVENTS)
}

/// A fresh key for `service` to tell the core with, its digest kept in place of any it had.
///
/// # Errors
///
/// Why none could be minted or kept.
pub(crate) fn minted(ctx: &Ctx, service: &str) -> Result<String, String> {
    let path = at(ctx).ok_or_else(|| "there is nowhere to keep it".to_owned())?;
    let key = crate::secret::generate(ctx.seams.random.as_ref())
        .ok_or_else(|| crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())?;
    crate::settings::kept::changed(&path, &mut |kept: &mut Keyed| {
        kept.keys.insert(service.to_owned(), sha256(&key));
        Ok(())
    })
    .map_err(|failure| failure.to_string())?;
    Ok(key)
}

/// Take `event` from whichever service `presented` is the key of, and owe every back-end
/// that receives the stack's alerts the alert it comes to, where the operator wants it.
///
/// # Errors
///
/// [`EVENTS_REFUSED`] where no service was handed `presented`.
pub fn intake(ctx: &Ctx, presented: &str, event: &Event) -> Result<String, Box<Problem>> {
    let presented = sha256(presented);
    let service = at(ctx)
        .and_then(|path| crate::settings::kept::read::<Keyed>(&path).ok())
        .and_then(|kept| {
            kept.keys
                .into_iter()
                .fold(None, |found, (service, digest)| {
                    if same(&digest, &presented) {
                        Some(service)
                    } else {
                        found
                    }
                })
        })
        .ok_or_else(refused)?;
    let (kind, severity, said) = told(&service, event);
    if !ctx.dry_run && crate::app::appetite::recorded(ctx).wants(kind, severity) {
        super::fanning::owe_now(ctx, notification(event), &said, severity >= Severity::Error);
    }
    Ok(service)
}

/// The event kind `event` is wanted by, how serious it is, and what it says: never the
/// title, nor the reason a curator gave for one, since it may be one a member asked for,
/// and only a problem a service raised about itself in its own words.
fn told(service: &str, event: &Event) -> (&'static str, Severity, String) {
    match event.event {
        Happened::Imported => (
            "import.succeeded",
            Severity::Advisory,
            format!("{service} brought a title into the library"),
        ),
        Happened::Added => (
            "import.succeeded",
            Severity::Advisory,
            format!("{service} has a new title in its library"),
        ),
        Happened::Failed => (
            "import.failed",
            Severity::Warning,
            format!("{service} could not fetch or import a title"),
        ),
        Happened::Health if event.cleared => (
            "service.health",
            Severity::Advisory,
            format!("{service} cleared: {}", cut(&event.name)),
        ),
        Happened::Health => (
            "service.health",
            Severity::Warning,
            format!("{service}: {}", cut(&event.name)),
        ),
    }
}

/// The notification kind `event` is told as: a failure or a health problem as a problem,
/// an import or an addition as a completion.
const fn notification(event: &Event) -> crate::alert::Notification {
    match event.event {
        Happened::Failed | Happened::Health => crate::alert::Notification::Problem,
        Happened::Imported | Happened::Added => crate::alert::Notification::Completion,
    }
}

/// Whether two digests are the same, in time that depends on their length alone.
fn same(kept: &str, presented: &str) -> bool {
    kept.len() == presented.len()
        && kept
            .bytes()
            .zip(presented.bytes())
            .fold(0_u8, |differs, (left, right)| differs | (left ^ right))
            == 0
}

/// `said` cut short, with no credential in it.
fn cut(said: &str) -> String {
    crate::error::withheld::without_credentials(said.trim())
        .chars()
        .take(SAID_MOST)
        .collect()
}

/// The refusal of an event no service was handed the key for.
fn refused() -> Box<Problem> {
    Box::new(
        Problem::new(
            EVENTS_REFUSED,
            "That key was handed to no service".to_owned(),
            "Nothing was told.",
            Remedy::new("Wire the stack again, so the service is handed its key"),
        )
        .in_state(State::Guided),
    )
}

mod reached;

#[cfg(test)]
mod tests;

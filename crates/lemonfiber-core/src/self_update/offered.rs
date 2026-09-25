//! Asking what has been released, and making as little of the answer as possible.
//!
//! The request carries nothing. No credential, no version, no setting, nothing that
//! would tell one installation from another — the only header beyond the two any
//! reader of this address sends is a name, constant in every copy of this program,
//! which the address requires of anybody asking and which distinguishes nobody.
//!
//! **The list rather than the latest.** There is an address that serves "the latest
//! release" and it is the wrong one here: every release of this project is published
//! as a pre-release, and that address passes over pre-releases, so it answers with
//! nothing at all. Reading the list and ordering it here is not a preference — it is
//! the only reading that has an answer.
//!
//! Ordering is [`crate::migration::version`]'s, which answers `Untellable` rather
//! than guessing which of two strings is later. A wrong answer here would tell an
//! operator to move to a version that is behind the one they have, so a tag this
//! cannot take whole is passed over rather than ranked.

use serde::Deserialize;

use crate::migration::version::{against, among_versions, Standing};
use crate::ports::http::{Method, Request};

/// How many of the most recent releases to read.
///
/// Enough that the newest is certainly among them and small enough that the answer
/// is one small document. The address serves them newest-first, so this is a bound
/// on the reply rather than a search that could run past the answer.
const HOW_MANY: usize = 10;

/// The name this program gives when it asks.
///
/// The address refuses a request that gives none, so there is one to choose and the
/// choice is what it says. The product's name and nothing after it: a version here
/// would be a fact about this installation travelling on every check, and one
/// constant string shared by every copy tells whoever reads it nothing it did not
/// already know from being asked.
const CALLED: &str = "lemonfiber";

/// Where the release page puts the boundary between what changed and how to install it.
///
/// `release-changelog.yml` writes the notes above this line and leaves what cargo-dist
/// wrote below it. Splitting on the marker rather than taking the whole body is what
/// keeps install boilerplate out of an answer about what changed — and a release whose
/// body has no marker is one written before the notes came back, which is answered with
/// nothing rather than with the boilerplate.
const BOUNDARY: &str = "<!-- the changelog is above; cargo-dist wrote what follows -->";

/// The prefix a release publishes its stack's manifest generation under.
///
/// The declaration is the asset's *name*, and its contents are never read. That is
/// what makes this free: names arrive in the same reply the version is read out of,
/// so nothing is fetched a second time and nothing new is sent to learn it.
const DECLARED: &str = "stack-schema-";

/// One file published with a release. Only the name is read.
#[derive(Debug, Deserialize)]
struct Asset {
    /// What the file is called, which is where the declaration lives.
    name: String,
}

/// One release, as much of it as this reads.
#[derive(Debug, Deserialize)]
struct Release {
    /// The tag the release was cut from.
    tag_name: String,
    /// Whether it is still a draft, and so not released at all.
    draft: bool,
    /// The files published with it.
    ///
    /// Defaulted rather than required: a reply that omits them is a reply that
    /// declares nothing, which is a thing this has an answer for.
    #[serde(default)]
    assets: Vec<Asset>,
    /// What the release page says, where it says anything.
    #[serde(default)]
    body: Option<String>,
}

/// The request that asks what has been released.
#[must_use]
pub fn asking(at: &str) -> Request {
    Request {
        method: Method::Get,
        url: format!("{at}?per_page={HOW_MANY}"),
        headers: vec![
            (
                "Accept".to_owned(),
                "application/vnd.github+json".to_owned(),
            ),
            ("User-Agent".to_owned(), CALLED.to_owned()),
        ],
        body: None,
    }
}

/// The newest version in what the address answered, where one can be told.
///
/// Nothing where the answer was not a list of releases, held none that was
/// published, or held none whose tag this can order. Each of those is the same thing
/// to a caller — availability could not be determined — and none of them is a
/// failure the operator has to do anything about.
#[must_use]
pub fn newest(answered: &str) -> Option<String> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let mut best: Option<String> = None;
    for release in released.into_iter().filter(|release| !release.draft) {
        let version = release.tag_name.trim_start_matches('v').to_owned();
        if cut_ahead(&version) {
            continue;
        }
        let later = match &best {
            None => ordered(&version),
            Some(held) => against(&version, held) == Standing::Later,
        };
        if later {
            best = Some(version);
        }
    }
    best
}

/// What the release for one version says changed, where it says anything.
///
/// Read out of the same answer `newest` is read out of, so learning what is available
/// and learning what it brought are one request rather than two. An operator weighing
/// an update is asking both questions at once, and the second is the one they decide
/// on.
#[must_use]
pub fn changed(answered: &str, version: &str) -> Option<String> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let release = released
        .into_iter()
        .find(|release| !release.draft && release.tag_name.trim_start_matches('v') == version)?;
    let body = release.body?;
    let (notes, _) = body.split_once(BOUNDARY)?;
    let notes = notes.trim();
    (!notes.is_empty()).then(|| notes.to_owned())
}

/// The manifest generation the release for `version` says its stack carries.
///
/// Read out of the same answer `newest` and `changed` are read out of, so what is
/// available, what it changed and what it carries are one request rather than three.
///
/// Nothing where the release declares none. That is not the same as declaring zero
/// and must not read like it: a release published before this was declared, or by a
/// fork that publishes none, is one whose stack generation is unknown — and a caller
/// that guessed would say something false about an update rather than say it cannot
/// tell.
#[must_use]
pub fn schema(answered: &str, version: &str) -> Option<u32> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let release = released
        .into_iter()
        .find(|release| !release.draft && release.tag_name.trim_start_matches('v') == version)?;
    release
        .assets
        .iter()
        .find_map(|asset| asset.name.strip_prefix(DECLARED))
        .and_then(|declared| declared.parse().ok())
}

/// Whether a version is one this can order at all.
///
/// Asked of the first candidate, because a list holding one unorderable tag would
/// otherwise be answered with that tag — the comparison never runs, so nothing
/// catches it. Every later candidate is caught by the comparison itself.
fn ordered(version: &str) -> bool {
    against(version, "0.0.0") != Standing::Untellable
}

/// Whether a tag names a build cut ahead of a release rather than a release.
///
/// Read here rather than left to the comparison, and the difference is the whole
/// point. A pre-release goes out while its version is still staged, carrying the
/// goals the release gate calls unmet; offering it would be recommending it. It used
/// to be skipped by accident — the tag is not a dotted run of numbers, so nothing
/// could order it — and an accident is a property that survives exactly until
/// somebody makes the parser cleverer, which is what [`among_versions`] just did.
fn cut_ahead(version: &str) -> bool {
    version.contains('-')
}

/// Where the running version stands against the newest released one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// Nothing newer has been released.
    Current,
    /// This version has been released since.
    Newer(String),
    /// The two cannot be ordered, so nothing is claimed about either.
    Untellable,
}

/// Where `running` stands against `offered`.
#[must_use]
pub fn standing(running: &str, offered: &str) -> Availability {
    match among_versions(running, offered) {
        Standing::Earlier => Availability::Newer(offered.to_owned()),
        Standing::Same | Standing::Later => Availability::Current,
        Standing::Untellable => Availability::Untellable,
    }
}

#[cfg(test)]
mod tests;

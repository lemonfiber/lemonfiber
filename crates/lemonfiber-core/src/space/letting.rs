//! Letting one completed download go, and the agreement that is its own.
//!
//! The account beside this one offers what costs nothing and leaves everything that
//! costs something with the operator. This is the other half: the one download they
//! named, with what letting it go costs said before anything is asked, and an
//! agreement that answers *that* reading and no other.
//!
//! **The agreement is deliberately not the account's.** A person who agreed to
//! reclaim what costs nothing has not thereby agreed to lose a ratio a private
//! tracker keeps their account on, so the two are named over different words and one
//! can never be spent on the other. Every name here begins with the same word for
//! this errand, which is what makes that true however the two readings happen to
//! line up.
//!
//! There is no blanket form of the yes either. The answer is the offer's own name,
//! so agreeing to this is only possible after reading the offer it names — which is
//! what "state the consequence and require confirmation" comes to when the two are
//! one request apart.

use serde::Serialize;

use super::waste::{ratio_reads, Candidate, Standing};
use crate::ports::service::Seeded;

/// What goes with a download when the client lets it go, said before it is asked for.
///
/// Both halves matter. The first is the point — the room only comes back if the copy
/// goes with the torrent — and the second is what stops an operator reading the first
/// as their library being taken: on a stack that hardlinks, the library's name for
/// the file is the media, and it is not this errand's to remove.
pub const WHAT_GOES: &str = "The copy in the downloads tree goes with it, which is \
                             where the room comes back from. Anything the library \
                             holds its own name for stays — that name is the media, \
                             and this does not touch it.";

/// The first word every one of these agreements is named over.
///
/// So that no answer given to the general cleanup can name this offer, and no answer
/// given here can be spent on the cleanup, whatever the two happen to be read over.
const ERRAND: &str = "stop seeding";

/// One completed download, what letting it go would cost, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Letting {
    /// The download, in the same words the account names it in: where it stands, what
    /// it occupies, and what removing it costs.
    pub download: Candidate,
    /// What goes with it, carried rather than left for a surface to remember.
    pub goes: String,
    /// What this offer names itself, so an answer to it can say which offer it
    /// answered.
    pub agreement: String,
    /// What became of an answered offer, and nothing where the offer is all this is.
    pub gone: Option<Gone>,
}

/// What became of a download the client was asked to let go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Gone {
    /// What the client is no longer holding.
    pub name: String,
    /// What it occupied, as the client reported it.
    pub bytes: u64,
    /// Whether this was a rehearsal, which asks the client for nothing.
    pub rehearsed: bool,
}

/// The offer to let one completed download go, named over what it says.
#[must_use]
pub fn offering(download: Candidate) -> Letting {
    let agreement = agreement(&download);
    Letting {
        download,
        goes: WHAT_GOES.to_owned(),
        agreement,
        gone: None,
    }
}

/// What one offer of this kind names itself.
///
/// Over everything that would make an operator read it differently — which one it is,
/// what it occupies, where it stands, the ratio it has earned and what removing it
/// costs — so that an offer read yesterday cannot be answered today if any of those
/// has moved. Through [`crate::agreement`], because a second way of naming a reading
/// would be a second answer to the same question.
#[must_use]
pub fn agreement(download: &Candidate) -> String {
    let bytes = download.bytes.to_string();
    let stands = stands(download.standing);
    let mut words: Vec<&str> = vec![
        ERRAND,
        download.name.as_str(),
        bytes.as_str(),
        stands.as_str(),
        WHAT_GOES,
    ];
    if let Some(consequence) = &download.consequence {
        words.push(consequence.as_str());
    }
    crate::agreement::over(&words)
}

/// Where one download stands, in a word an agreement can be named over.
///
/// The figures rather than the sentence a surface renders them into: a ratio that
/// moved is a different offer, and it is the ratio that says so wherever it is read.
fn stands(standing: Standing) -> String {
    match standing {
        Standing::NeverImported => "never imported".to_owned(),
        Standing::Seeding { ratio } => ratio_reads(ratio).map_or_else(
            || "seeding, having taken nothing".to_owned(),
            |reads| format!("seeding at {reads}"),
        ),
        Standing::LeftAlone => "left alone".to_owned(),
    }
}

/// Where one held download stands, as the account has it — or, where the walk could
/// not match it to a file, as still seeding at the ratio the client reports.
///
/// Erring towards the cost, which is the only direction this may err in. "I could not
/// find it on disk" is not evidence that nothing points at it, and of the two ways to
/// be wrong about a torrent a client is holding, the one that says nothing is at stake
/// is the one that costs somebody an account.
#[must_use]
pub fn standing_of(held: &Seeded, accounted: &[Candidate]) -> Candidate {
    accounted
        .iter()
        .find(|candidate| candidate.name == held.name)
        .cloned()
        .unwrap_or_else(|| Candidate {
            name: held.name.clone(),
            bytes: held.bytes,
            standing: Standing::Seeding { ratio: held.ratio },
            consequence: Some(super::waste::RATIO_CONSEQUENCE.to_owned()),
        })
}

#[cfg(test)]
mod tests;

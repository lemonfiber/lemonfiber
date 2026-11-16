//! Asks for members whose asks wait for approval, kept until each is ruled on, another
//! member's ask for the same title joining the one already waiting.

use serde::{Deserialize, Serialize};

use super::Kept;
use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::instant::Instant;
use crate::ports::media::Kind;
use crate::ports::service::Member;

/// One ask for members whose asks wait for approval, kept until it is ruled on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Waiting {
    /// Its number among its kind's asks, from one.
    pub number: u64,
    /// The kind of title it asks for.
    pub kind: Kind,
    /// The request service's id for the title.
    pub title: String,
    /// Every season any member on it asked for, none for a film or a series asked for
    /// whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<u32>,
    /// What the title is called, where the request service named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub called: Option<String>,
    /// The year it came out, where the request service knows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Each ask that made or joined it, in the order they were made.
    pub askers: Vec<Asker>,
    /// When it was first asked for, which its expiry counts from.
    pub asked_at: Instant,
    /// Where it stands: waiting, sent once filed, declined or expired.
    pub standing: Standing,
    /// When it last changed where it stands, where it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<Instant>,
}

/// One member's ask that made or joined a waiting ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Asker {
    /// The member it is for, by their id at the media server.
    pub member: String,
    /// Their name, as the media server gave it when they asked.
    pub name: String,
    /// The seasons they asked for, none for a film or a series asked for whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<u32>,
    /// How much of their allowance it uses: one for a film, one for each season asked.
    pub counts: u32,
    /// When they asked for it.
    pub asked_at: Instant,
}

impl Waiting {
    /// The reference it is named by on the household list.
    #[must_use]
    pub(crate) const fn named(&self) -> Reference {
        Reference::AsOwner(self.kind, self.number)
    }

    /// Whether it still waits for somebody to rule on it.
    pub(super) fn waits(&self) -> bool {
        self.standing == Standing::Waiting
    }

    /// Each member on it once, by their id, with their name and when they first asked,
    /// in the order they joined.
    pub(crate) fn members(&self) -> Vec<(&str, &str, Instant)> {
        let mut members: Vec<(&str, &str, Instant)> = Vec::new();
        for asker in &self.askers {
            if !members.iter().any(|(id, ..)| *id == asker.member) {
                members.push((&asker.member, &asker.name, asker.asked_at));
            }
        }
        members
    }

    /// How much of `member`'s allowance their asks on it use.
    pub(super) fn counts_for(&self, member: &str) -> u32 {
        self.askers
            .iter()
            .filter(|asker| asker.member == member)
            .map(|asker| asker.counts)
            .sum()
    }
}

/// The seasons `one` and `other` ask for together, none where either asks for all of it.
fn together(one: &[u32], other: &[u32]) -> Vec<u32> {
    if one.is_empty() || other.is_empty() {
        return Vec::new();
    }
    let mut seasons: Vec<u32> = one.iter().chain(other).copied().collect();
    seasons.sort_unstable();
    seasons.dedup();
    seasons
}

/// One ask to keep until it is ruled on, as it was made.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Asking<'a> {
    /// The kind of title it asks for.
    pub kind: Kind,
    /// The request service's id for the title.
    pub title: &'a str,
    /// The seasons it asks for.
    pub seasons: &'a [u32],
    /// What the title is called.
    pub called: Option<&'a str>,
    /// The year it came out.
    pub year: Option<u16>,
    /// The member it is for.
    pub member: &'a Member,
    /// How much of their allowance it uses.
    pub counts: u32,
    /// When it was asked for.
    pub at: Instant,
}

impl Kept {
    /// Keep `asking` on the ask already waiting for its title, or as a new one under the
    /// next number of its kind, and say what that ask is named.
    pub(super) fn joining(&mut self, asking: Asking<'_>) -> Reference {
        let asker = Asker {
            member: asking.member.id.clone(),
            name: asking.member.name.clone(),
            seasons: asking.seasons.to_vec(),
            counts: asking.counts,
            asked_at: asking.at,
        };
        if let Some(ask) = self
            .waiting
            .iter_mut()
            .find(|ask| ask.waits() && ask.kind == asking.kind && ask.title == asking.title)
        {
            ask.seasons = together(&ask.seasons, asking.seasons);
            ask.askers.push(asker);
            return ask.named();
        }
        let number = self
            .waiting
            .iter()
            .filter(|ask| ask.kind == asking.kind)
            .map(|ask| ask.number)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.waiting.push(Waiting {
            number,
            kind: asking.kind,
            title: asking.title.to_owned(),
            seasons: asking.seasons.to_vec(),
            called: asking.called.map(str::to_owned),
            year: asking.year,
            askers: vec![asker],
            asked_at: asking.at,
            standing: Standing::Waiting,
            settled_at: None,
        });
        Reference::AsOwner(asking.kind, number)
    }

    /// Move the waiting ask `reference` names to `standing` at `at`, and say what it was.
    pub(super) fn settling(
        &mut self,
        reference: Reference,
        standing: Standing,
        at: Instant,
    ) -> Option<Waiting> {
        let ask = self
            .waiting
            .iter_mut()
            .find(|ask| ask.named() == reference && ask.waits())?;
        let was = ask.clone();
        ask.standing = standing;
        ask.settled_at = Some(at);
        Some(was)
    }
}

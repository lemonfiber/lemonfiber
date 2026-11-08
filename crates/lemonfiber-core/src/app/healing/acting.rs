//! The three acts a run takes on an item that struck out: its curator drops it from its
//! client, blocklisting its release, then searches for its title again; each one said as
//! it came out, and the drop journalled so it can be put back.

use crate::app::Ctx;
use crate::journal::{Change, Kind};
use crate::model::{Act, ActOutcome, ActTaken, Healed};
use crate::ports::service::Failure;

use super::gathering::{Curator, Gathered};
use super::judging::Found;

/// The operation each drop is journalled under.
pub(crate) const OPERATION: &str = "healing a wedged download";

/// Why nothing is done to an item no curator holds.
const NO_CURATOR: &str = "no curator holds it, and lemonfiber drops a download only \
                          through the curator that asked for it";

/// Why a run that was a rehearsal did nothing.
const REHEARSED: &str = "this run only said what it would do";

/// What a run did to `found`, or would have where it is a rehearsal.
pub(super) async fn acted(ctx: &Ctx, gathered: &Gathered, found: &Found) -> Healed {
    let curator = found.curator.and_then(|at| gathered.curators.get(at));
    let title = found
        .entry
        .as_ref()
        .zip(curator)
        .and_then(|(entry, curator)| curator.titles.get(&entry.title_id))
        .cloned();
    let acts = match (curator, found.entry.as_ref()) {
        (Some(curator), Some(entry)) => match (&curator.held_back, entry.queued) {
            (Some(why), _) => refused_all(why),
            (None, None) => refused_all("its curator names no queue entry for it"),
            (None, Some(_)) if ctx.dry_run => would(),
            (None, Some(queued)) => dropped(ctx, curator, entry.title_id, queued).await,
        },
        (Some(_) | None, _) => refused_all(NO_CURATOR),
    };
    Healed {
        title,
        service: curator
            .map(|curator| curator.id.clone())
            .or_else(|| found.client.clone())
            .unwrap_or_default(),
        release: found.release(),
        why: found.why.clone(),
        acts,
    }
}

/// Whether a run did something to `healed`: dropped it, or searched again in its place.
pub(super) fn took(healed: &Healed) -> bool {
    healed.acts.iter().any(|taken| {
        taken.act == Act::Remove && matches!(taken.outcome, ActOutcome::Done | ActOutcome::FellBack)
    })
}

/// What a run says it did to `healed`, in one sentence.
pub(super) fn said(healed: &Healed, rehearsed: bool) -> String {
    let named = healed.title.as_deref().unwrap_or(&healed.release);
    let acts: Vec<String> = healed
        .acts
        .iter()
        .map(|taken| {
            let act = match taken.act {
                Act::Remove => "drop",
                Act::Blocklist => "blocklist",
                Act::Search => "search again",
            };
            match (taken.outcome, &taken.why) {
                (ActOutcome::Done, _) => format!("{act}: done"),
                (ActOutcome::Would, _) => format!("{act}: would"),
                (ActOutcome::Refused | ActOutcome::FellBack, Some(why)) => {
                    format!("{act}: not done, {why}")
                }
                (ActOutcome::Refused | ActOutcome::FellBack, None) => format!("{act}: not done"),
            }
        })
        .collect();
    format!(
        "{}{} {named}, because {}: {}",
        if rehearsed { "would heal" } else { "healed" },
        if healed.service.is_empty() {
            String::new()
        } else {
            format!(" at {}", healed.service)
        },
        healed.why,
        acts.join(", ")
    )
}

/// Each of the three acts, refused for `why`.
fn refused_all(why: &str) -> Vec<ActTaken> {
    [Act::Remove, Act::Blocklist, Act::Search]
        .into_iter()
        .map(|act| ActTaken {
            act,
            outcome: ActOutcome::Refused,
            instead: None,
            why: Some(why.to_owned()),
        })
        .collect()
}

/// Each of the three acts, as a rehearsal would have taken it.
fn would() -> Vec<ActTaken> {
    [Act::Remove, Act::Blocklist, Act::Search]
        .into_iter()
        .map(|act| ActTaken {
            act,
            outcome: ActOutcome::Would,
            instead: None,
            why: Some(REHEARSED.to_owned()),
        })
        .collect()
}

/// One act, done where nothing refused it and refused for `refused` where something did.
fn taken(act: Act, refused: Option<&Failure>) -> ActTaken {
    ActTaken {
        act,
        outcome: if refused.is_some() {
            ActOutcome::Refused
        } else {
            ActOutcome::Done
        },
        instead: None,
        why: refused.map(ToString::to_string),
    }
}

/// Drop the queue entry `queued` from `curator`'s client, blocklisting its release, then
/// search for the title `title` again.
///
/// A curator that cannot drop a download is asked to search again alone, and the drop is
/// said to have fallen back to that search, not done.
async fn dropped(ctx: &Ctx, curator: &Curator, title: i64, queued: i64) -> Vec<ActTaken> {
    let dropping = curator.asked.unqueue(curator.kind, queued, true).await;
    let mut acts = match &dropping {
        Ok(release) => {
            journalled(ctx, curator, title, release);
            vec![taken(Act::Remove, None), taken(Act::Blocklist, None)]
        }
        Err(failure @ Failure::Unsupported { .. }) => vec![
            ActTaken {
                act: Act::Remove,
                outcome: ActOutcome::FellBack,
                instead: Some(Act::Search),
                why: Some(failure.to_string()),
            },
            taken(Act::Blocklist, Some(failure)),
        ],
        Err(failure) => return refused_all(&failure.to_string()),
    };
    let searching = curator.asked.search(curator.kind, title).await;
    acts.push(taken(Act::Search, searching.as_ref().err()));
    acts
}

/// Journal the drop of `release` from `curator`, best effort as every act at the watch's
/// pace is.
fn journalled(ctx: &Ctx, curator: &Curator, title: i64, release: &str) {
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: curator.id.clone(),
        kind: Kind::Unqueued {
            kind: curator.kind,
            title,
            release: release.to_owned(),
        },
    };
    if let Some(paths) = crate::app::targets::layout(ctx) {
        let _ = crate::app::recover::journalled(&paths.journal(), std::slice::from_ref(&change));
    }
}

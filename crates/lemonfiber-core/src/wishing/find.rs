//! One page of a member's search, under their limits.

use futures_util::future::join_all;

use super::place::Place;
use super::{capitalised, not_working, reached, refused, Reached};
use crate::app::Ctx;
use crate::error::codes::wish::{CANNOT_FETCH, NOT_A_SEARCH};
use crate::error::Problem;
use crate::model::{Askable, AskableReport, Stands, TitleKind};
use crate::ports::service::{Found, MediaStatus};

/// The fewest characters a search is made with.
const TERM_SHORTEST: usize = 2;

/// The most characters a search is made with.
const TERM_LONGEST: usize = 100;

/// How many of the request service's pages one page of a search reads, at most, looking
/// for a title the member may be shown.
const READ_PER_PAGE: usize = 3;

/// One page of what `term` finds for the member `named`, from `after` on.
///
/// # Errors
///
/// A [`Problem`] where the words are too few or too many, where `after` is not a place a
/// page gave, where the house cannot fetch anything now, and wherever [`reached`]
/// refuses or what answers does not.
pub(super) async fn found(
    ctx: &Ctx,
    named: &str,
    term: &str,
    after: Option<&str>,
) -> Result<AskableReport, Box<Problem>> {
    let term = term.trim();
    if !(TERM_SHORTEST..=TERM_LONGEST).contains(&term.chars().count()) {
        return Err(refused(
            NOT_A_SEARCH,
            "Type at least two letters, and no more than a hundred.",
        ));
    }
    let mut place = Place::read(after)?;
    let reached = reached(ctx, named).await?;
    if reached.kinds.is_empty() {
        return Err(refused(
            CANNOT_FETCH,
            "This house can't fetch anything new right now.",
        ));
    }
    let theirs = crate::household::run::allowance_of(
        reached.requests.as_ref(),
        &reached.member,
        ctx.seams.clock.now(),
    )
    .await
    .map_err(|_| not_working())?;
    let mut titles = Vec::new();
    let mut next = Some(place.page);
    for _ in 0..READ_PER_PAGE {
        let Some(page) = next else {
            break;
        };
        let read = reached
            .requests
            .search(term, &reached.kinds, page)
            .await
            .map_err(|_| not_working())?;
        next = read.next.filter(|after| *after > page);
        for title in shown(&reached, read.titles).await {
            if !place.shown.contains(&title.id) {
                place.shown.push(title.id.clone());
                titles.push(askable(title));
            }
        }
        if !titles.is_empty() {
            break;
        }
    }
    Ok(AskableReport {
        member: reached.member.name,
        term: term.to_owned(),
        kinds: reached.kinds.into_iter().map(TitleKind::from).collect(),
        titles,
        next: next.and_then(|page| Place { page, ..place }.written()),
        approval: format!("{}.", capitalised(theirs.asking.policy.household())),
        asking: theirs.asking,
    })
}

/// The titles of `found` the member may be shown, in the order found.
///
/// Each of a kind the house can fetch, and for a member held to anything, each looked up
/// and held against their limits; one that cannot be looked up is left out.
async fn shown(reached: &Reached, found: Vec<Found>) -> Vec<Found> {
    let fetched = found
        .into_iter()
        .filter(|title| reached.kinds.contains(&title.kind));
    if !reached.limits.restricted() {
        return fetched.collect();
    }
    let looked_up = join_all(fetched.map(|title| async move {
        let detail = reached
            .requests
            .detail(title.kind, &title.id, reached.limits.region())
            .await;
        let admitted = matches!(
            detail,
            Ok(Some(ref detail)) if reached.limits.admits(detail.certification.as_deref())
        );
        admitted.then_some(title)
    }))
    .await;
    looked_up.into_iter().flatten().collect()
}

/// A title as a member's search shows it.
fn askable(found: Found) -> Askable {
    Askable {
        stands: stands(found.status),
        id: found.id,
        kind: found.kind.into(),
        title: found.title,
        year: found.year,
    }
}

/// Where a title stands in the house, as a member is told.
const fn stands(status: MediaStatus) -> Stands {
    match status {
        MediaStatus::Available => Stands::Here,
        MediaStatus::PartlyAvailable => Stands::PartlyHere,
        MediaStatus::Pending | MediaStatus::Processing => Stands::OnItsWay,
        MediaStatus::Unknown | MediaStatus::Deleted => Stands::Askable,
    }
}

#[cfg(test)]
mod tests;

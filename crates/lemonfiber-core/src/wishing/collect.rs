//! A member's search for music or books, answered from the curator of each by its own
//! lookup, under the household's policy.

use super::ask::{nothing_left, said, ON_ITS_WAY};
use super::{capitalised, not_working, reached, refused, searched};
use crate::app::Ctx;
use crate::collected::store::{Ask, Asking, Standing};
use crate::collected::{Collector, Medium, Unit};
use crate::error::codes::wish::{ALREADY_HERE, CANNOT_FETCH, NOT_A_PLACE, NO_ROOM};
use crate::error::Problem;
use crate::model::{Askable, AskableReport, AskedReport, Became, LeftAfter, Stands, TitleKind};
use crate::ports::service::{Candidate, CandidateState, Left};

/// The longest a place a curator's page gave is taken back.
const PLACE_LONGEST: usize = 1024;

/// One page of the `unit`s `term` finds for the member `named`, from `after` on.
///
/// # Errors
///
/// A [`Problem`] where the words are too few or too many, where `after` is not a place a
/// page could have given, where no curator here answers for the unit's medium, and
/// wherever [`reached`] refuses or what answers does not.
pub(super) async fn found(
    ctx: &Ctx,
    named: &str,
    term: &str,
    after: Option<&str>,
    unit: Unit,
) -> Result<AskableReport, Box<Problem>> {
    let term = searched(term)?;
    if after.is_some_and(|after| after.is_empty() || after.len() > PLACE_LONGEST) {
        return Err(refused(NOT_A_PLACE, "That search can't go on from there."));
    }
    let reached = reached(ctx, named).await?;
    let curator = curator_of(ctx, unit).await?;
    let theirs = super::allowance(ctx, &reached)
        .await
        .map_err(|_| not_working())?;
    let page = curator
        .lookup(unit, term, after)
        .await
        .map_err(|_| not_working())?;
    let kept = crate::collected::run::every(ctx);
    Ok(AskableReport {
        member: reached.member.name,
        term: term.to_owned(),
        kinds: vec![TitleKind::from(unit.medium())],
        titles: page
            .found
            .into_iter()
            .map(|found| askable(unit, found, &kept))
            .collect(),
        next: page.next,
        approval: format!("{}.", capitalised(theirs.asking.policy.household())),
        asking: theirs.asking,
    })
}

/// Ask for the candidate `reference` names, as a `unit`, on behalf of the member `named`,
/// or under a rehearsal say what asking would come to.
///
/// A whole artist, author or series waits for the operator whatever the household's
/// policy; an album or a book follows it. Either is kept by the core under the next
/// reference of its medium, and one that does not wait is asked of the curator first.
///
/// # Errors
///
/// A [`Problem`] where no curator here answers for the unit's medium, where it holds no
/// such candidate, where it is already here, where the house has no room, where the
/// member's allowance has none left, and wherever [`reached`] refuses or what answers
/// does not.
pub(super) async fn asked(
    ctx: &Ctx,
    named: &str,
    unit: Unit,
    reference: &str,
) -> Result<AskedReport, Box<Problem>> {
    let path = crate::collected::run::kept_at(ctx).map_err(|_| not_working())?;
    let reached = reached(ctx, named).await?;
    let curator = curator_of(ctx, unit).await?;
    let candidate = curator
        .candidate(unit, reference)
        .await
        .map_err(|_| not_working())?
        .ok_or_else(super::no_such_title)?;
    let theirs = super::allowance(ctx, &reached)
        .await
        .map_err(|_| not_working())?;
    let kept = crate::collected::run::every(ctx);
    let left = crate::collected::run::left(
        &kept,
        theirs.headroom.films,
        &reached.member.id,
        unit.medium(),
        ctx.seams.clock.now(),
    );
    let report = |outcome: Became, cost: u32, request: Option<String>| {
        let leaves = leaves_of(unit, left, cost);
        AskedReport {
            member: reached.member.name.clone(),
            title: reference.to_owned(),
            kind: TitleKind::from(unit.medium()),
            unit: Some(unit.into()),
            seasons: Vec::new(),
            request,
            said: match outcome {
                Became::OnItsWay => ON_ITS_WAY.to_owned(),
                _ => said(outcome, true, &leaves, ctx.dry_run),
            },
            outcome,
            leaves,
            released: true,
            rehearsed: ctx.dry_run,
        }
    };
    let waiting = waits_here(&kept, unit, &candidate.reference);
    match candidate.state {
        CandidateState::Here => return Err(refused(ALREADY_HERE, "It's already in the house.")),
        CandidateState::Asked => return Ok(report(Became::OnItsWay, 0, None)),
        CandidateState::PartlyHere | CandidateState::MayAsk if waiting => {
            return Ok(report(Became::OnItsWay, 0, None))
        }
        CandidateState::PartlyHere | CandidateState::MayAsk => {}
    }
    if crate::space::run::admits(ctx).await.is_err() {
        return Err(refused(
            NO_ROOM,
            "The house has no room for new things right now.",
        ));
    }
    if left.remaining().is_some_and(|remaining| remaining < ONE) {
        return Err(nothing_left(&theirs));
    }
    let waits = unit.always_waits() || !theirs.approves_own;
    let outcome = if waits {
        Became::Waiting
    } else {
        Became::Asked
    };
    if ctx.dry_run {
        return Ok(report(outcome, ONE, None));
    }
    if !waits {
        crate::collected::run::sent(&curator, unit, &candidate.reference)
            .await
            .map_err(|_| not_working())?;
    }
    let filed = crate::collected::store::filed(
        &path,
        &Asking {
            unit,
            reference: candidate.reference,
            name: candidate.name,
            by: candidate.by,
            year: candidate.year,
            cover: candidate.cover,
            member: reached.member.id.clone(),
            at: crate::instant::written(ctx.seams.clock.now()).unwrap_or_default(),
            standing: if waits {
                Standing::Waiting
            } else {
                Standing::Sent
            },
        },
    )
    .map_err(|_| not_working())?;
    Ok(report(outcome, ONE, Some(filed.to_string())))
}

/// What one ask for music or books counts against the member's limit.
const ONE: u32 = 1;

/// What is left once `cost` of `unit`'s medium is counted, for that medium alone.
fn leaves_of(unit: Unit, left: Left, cost: u32) -> LeftAfter {
    let after = left
        .remaining()
        .map(|remaining| remaining.saturating_sub(cost));
    match unit.medium() {
        Medium::Music => LeftAfter {
            music: after,
            ..LeftAfter::default()
        },
        Medium::Books => LeftAfter {
            books: after,
            ..LeftAfter::default()
        },
    }
}

/// The curator answering for `unit`'s medium.
///
/// # Errors
///
/// The refusal for a house with none that can be asked now.
pub(super) async fn curator_of(ctx: &Ctx, unit: Unit) -> Result<Collector, Box<Problem>> {
    crate::collected::run::collector(ctx, unit.medium())
        .await
        .ok_or_else(|| {
            refused(
                CANNOT_FETCH,
                &format!("This house can't fetch {} right now.", unit.medium().word()),
            )
        })
}

/// A candidate as a member's search shows it: on its way where the curator already wants
/// it or the core keeps an ask for it waiting.
fn askable(unit: Unit, found: Candidate, kept: &[Ask]) -> Askable {
    let waiting = waits_here(kept, unit, &found.reference);
    Askable {
        stands: match found.state {
            CandidateState::Here => Stands::Here,
            CandidateState::Asked => Stands::OnItsWay,
            CandidateState::PartlyHere | CandidateState::MayAsk if waiting => Stands::OnItsWay,
            CandidateState::PartlyHere => Stands::PartlyHere,
            CandidateState::MayAsk => Stands::Askable,
        },
        id: found.reference,
        kind: TitleKind::from(unit.medium()),
        unit: Some(unit.into()),
        title: found.name,
        by: found.by,
        year: found.year,
        cover: found.cover,
    }
}

/// Whether the core keeps an ask for the `unit` `reference` names waiting for the operator.
fn waits_here(kept: &[Ask], unit: Unit, reference: &str) -> bool {
    kept.iter().any(|ask| {
        ask.unit == unit && ask.reference == reference && ask.standing == Standing::Waiting
    })
}

#[cfg(test)]
mod tests;

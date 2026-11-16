//! A member's ask for one title, checked by the core before anything is asked.
//!
//! The order is the requirement's: the member's limits and the house's own room and the
//! member's allowance are all settled before the request service is asked to file
//! anything, and a rehearsal settles all of them and files nothing.

mod telling;

pub(super) use telling::said;

use lemonfiber_contract::capabilities::request::intake::Fills;

use super::{not_working, reached, refused, refused_with, titled, Reached};
use crate::app::Ctx;
use crate::error::codes::wish::{ALREADY_HERE, NOTHING_LEFT, NOT_ITS_SEASON};
use crate::error::Problem;
use crate::household::run::as_owner::{self, HeldBack};
use crate::household::run::OwnAllowance;
use crate::instant::Instant;
use crate::model::{AskedReport, Became, LeftAfter};
use crate::ports::media::Kind;
use crate::ports::service::{
    Detail, Failure, HouseholdRequest, Left, MediaStatus, RequestStatus, Season, Wish,
};

/// What an ask names comes to before anything is filed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Choice {
    /// Ask for these seasons, or for the film where there are none.
    Ask(Vec<u32>),
    /// Somebody already asked for all of it.
    OnItsWay,
    /// All of it is here.
    Here,
}

/// Ask for `wish` on behalf of the member `named`, or under a rehearsal say what asking
/// would come to.
///
/// # Errors
///
/// A [`Problem`] where the house cannot fetch that kind, where no title the member may
/// ask for goes by that id, where a season named is not the title's, where it is already
/// here, where the house has no room, where the member's allowance has none left, and
/// wherever [`reached`] refuses or what answers does not.
pub(super) async fn asked(ctx: &Ctx, named: &str, wish: Wish) -> Result<AskedReport, Box<Problem>> {
    let reached = reached(ctx, named).await?;
    let detail = titled(&reached, wish.kind, &wish.id).await?;
    let theirs = super::allowance(ctx, &reached)
        .await
        .map_err(|_| not_working())?;
    let unchanged = |outcome, said: &str| AskedReport {
        member: reached.member.name.clone(),
        title: wish.id.clone(),
        kind: wish.kind.into(),
        unit: None,
        seasons: wish.seasons.clone(),
        request: None,
        outcome,
        leaves: leaves(&theirs, wish.kind, 0),
        released: detail.released,
        said: said.to_owned(),
        rehearsed: ctx.dry_run,
    };
    let held = as_owner::kept(ctx).held_back(wish.kind, &wish.id, &reached.member.id);
    let plan = match planned(
        &wish,
        &detail,
        &theirs.standing,
        &reached.member,
        held.as_ref(),
    )? {
        Planned::Here => return Err(refused(ALREADY_HERE, "It's already in the house.")),
        Planned::OnItsWay => return Ok(unchanged(Became::OnItsWay, ON_ITS_WAY)),
        Planned::AlreadyAsked => return Ok(unchanged(Became::Waiting, ALREADY_ASKED)),
        Planned::Ask(plan) => plan,
    };
    if crate::space::run::admits(ctx).await.is_err() {
        return Err(super::no_room());
    }
    let cost = plan.cost(wish.kind);
    let left = of_kind(&theirs, wish.kind);
    if left.remaining().is_some_and(|remaining| remaining < cost) {
        return Err(nothing_left(&theirs));
    }
    let leaves = leaves(&theirs, wish.kind, cost);
    let waiting = if ctx.dry_run {
        plan.beside.is_some() || !theirs.approves_own
    } else {
        carried_out(ctx, &reached, &theirs, (&wish, &detail), &plan).await?
    };
    let outcome = if waiting {
        Became::Waiting
    } else {
        Became::Asked
    };
    Ok(AskedReport {
        member: reached.member.name,
        title: wish.id,
        kind: wish.kind.into(),
        unit: None,
        said: said(outcome, detail.released, &leaves, ctx.dry_run),
        seasons: plan.seasons(),
        request: None,
        outcome,
        leaves,
        released: detail.released,
        rehearsed: ctx.dry_run,
    })
}

/// What an ask comes to once where the title stands, the requests the service holds and
/// the member's own asks kept are read.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Planned {
    /// All of it is here.
    Here,
    /// Somebody's request for all of it was approved, and it is on its way.
    OnItsWay,
    /// The member's own asks already wait for all of it.
    AlreadyAsked,
    /// Ask for what is left.
    Ask(Plan),
}

/// What to ask for, and what to keep beside other members' requests waiting for approval.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
    /// The seasons to ask the request service for, or none for the film or a series
    /// asked for whole; nothing where there is nothing to ask it for.
    ask: Option<Vec<u32>>,
    /// The seasons to keep beside other members' requests waiting for approval, and those
    /// requests; nothing where none waits.
    beside: Option<(Vec<u32>, Vec<i64>)>,
}

impl Plan {
    /// How much of the member's allowance it uses.
    fn cost(&self, kind: Kind) -> u32 {
        let asked = self
            .ask
            .as_ref()
            .map_or(0, |seasons| counted(kind, seasons));
        let beside = self
            .beside
            .as_ref()
            .map_or(0, |(seasons, _)| counted(kind, seasons));
        asked.saturating_add(beside)
    }

    /// Every season it asks for, kept or asked.
    fn seasons(&self) -> Vec<u32> {
        let mut seasons: Vec<u32> = self
            .ask
            .iter()
            .flatten()
            .chain(self.beside.iter().flat_map(|(seasons, _)| seasons))
            .copied()
            .collect();
        seasons.sort_unstable();
        seasons
    }
}

/// What asking for `wish` comes to for `member`, with `standing` the requests the service
/// holds and `held` what the member's own asks kept already wait for.
///
/// What only another member's request waiting for approval asks for is kept beside that
/// request rather than read as on its way; what the member's own asks already wait for is
/// left out.
///
/// # Errors
///
/// The refusal [`chosen`] answers with.
fn planned(
    wish: &Wish,
    detail: &Detail,
    standing: &[HouseholdRequest],
    member: &crate::ports::service::Member,
    held: Option<&HeldBack>,
) -> Result<Planned, Box<Problem>> {
    let choice = chosen(wish, detail)?;
    if choice == Choice::Here {
        return Ok(Planned::Here);
    }
    let pending = waiting_for_approval(wish, detail);
    let (others, own): (Vec<&HouseholdRequest>, Vec<&HouseholdRequest>) = standing
        .iter()
        .filter(|request| {
            request.kind == Some(wish.kind)
                && request.title_id.as_deref() == Some(wish.id.as_str())
                && request.request_status == Some(RequestStatus::Pending)
        })
        .partition(|request| !crate::household::run::made_by(request, member));
    let asking = match choice {
        Choice::Ask(seasons) => Some(seasons),
        Choice::OnItsWay | Choice::Here => None,
    };
    let own_waiting =
        asking.is_some() || (pending.is_some() && !(others.is_empty() && own.is_empty()));
    let beside = pending
        .filter(|_| !others.is_empty())
        .map(|seasons| (seasons, others.iter().map(|request| request.id).collect()));
    let plan = Plan {
        ask: asking.and_then(|seasons| unheld(seasons, held)),
        beside: beside.and_then(|(seasons, requests)| {
            unheld(seasons, held).map(|seasons| (seasons, requests))
        }),
    };
    if plan.ask.is_some() || plan.beside.is_some() {
        return Ok(Planned::Ask(plan));
    }
    Ok(if own_waiting {
        Planned::AlreadyAsked
    } else {
        Planned::OnItsWay
    })
}

/// The seasons of what `wish` names that wait for approval at the request service, none
/// for a film or a series with no seasons listed; nothing where none does.
fn waiting_for_approval(wish: &Wish, detail: &Detail) -> Option<Vec<u32>> {
    let considered: Vec<&Season> = detail
        .seasons
        .iter()
        .filter(|season| wish.seasons.is_empty() || wish.seasons.contains(&season.number))
        .collect();
    if considered.is_empty() {
        return (detail.status == MediaStatus::Pending).then(Vec::new);
    }
    let pending: Vec<u32> = considered
        .iter()
        .filter(|season| season.status == MediaStatus::Pending)
        .map(|season| season.number)
        .collect();
    (!pending.is_empty()).then_some(pending)
}

/// Ask for what `plan` asks the request service for, then keep what it keeps beside
/// other members' requests, and say whether any of it waits for approval.
///
/// # Errors
///
/// What [`filed`] refuses with, and the refusal for an ask that cannot be kept.
async fn carried_out(
    ctx: &Ctx,
    reached: &Reached,
    theirs: &OwnAllowance,
    (wish, detail): (&Wish, &Detail),
    plan: &Plan,
) -> Result<bool, Box<Problem>> {
    let filer = Filer {
        requests: reached.requests.as_ref(),
        member: &reached.member,
        kept: &reached.kept,
    };
    let named = Named {
        called: detail.title.as_deref(),
        year: detail.year,
    };
    let mut waiting = false;
    if let Some(seasons) = &plan.ask {
        waiting = filed(ctx, &filer, theirs, (wish, named), seasons).await?;
    }
    if let Some((seasons, requests)) = &plan.beside {
        let keeping = as_owner::beside::Keeping {
            requests,
            kind: wish.kind,
            title: &wish.id,
            seasons,
            called: named.called,
            year: named.year,
            member: &reached.member,
            counts: counted(wish.kind, seasons),
            at: ctx.stamp(),
        };
        as_owner::beside::kept(&reached.kept, keeping).map_err(|_| not_working())?;
        waiting = true;
    }
    Ok(waiting)
}

/// What asking for `wish` comes to, from where each part of the title stands.
///
/// No season named asks for every season not already here or on its way, and a season
/// named that is either is left out rather than asked for again.
///
/// # Errors
///
/// The refusal for seasons named for a film, and for a season the series does not have.
fn chosen(wish: &Wish, detail: &Detail) -> Result<Choice, Box<Problem>> {
    let not_a_season = || refused(NOT_ITS_SEASON, "That isn't a season this title has.");
    if wish.kind == Kind::Movies && !wish.seasons.is_empty() {
        return Err(not_a_season());
    }
    if wish
        .seasons
        .iter()
        .any(|number| !detail.seasons.iter().any(|season| season.number == *number))
    {
        return Err(not_a_season());
    }
    let considered: Vec<&Season> = detail
        .seasons
        .iter()
        .filter(|season| wish.seasons.is_empty() || wish.seasons.contains(&season.number))
        .collect();
    if considered.is_empty() {
        return Ok(match detail.status {
            MediaStatus::Available => Choice::Here,
            MediaStatus::Pending | MediaStatus::Processing | MediaStatus::PartlyAvailable => {
                Choice::OnItsWay
            }
            MediaStatus::Unknown | MediaStatus::Deleted => Choice::Ask(Vec::new()),
        });
    }
    let askable: Vec<u32> = considered
        .iter()
        .filter(|season| matches!(season.status, MediaStatus::Unknown | MediaStatus::Deleted))
        .map(|season| season.number)
        .collect();
    if !askable.is_empty() {
        return Ok(Choice::Ask(askable));
    }
    let coming = considered.iter().any(|season| {
        matches!(
            season.status,
            MediaStatus::Pending | MediaStatus::Processing
        )
    });
    Ok(if coming {
        Choice::OnItsWay
    } else {
        Choice::Here
    })
}

/// Whom an ask is filed for, and where.
pub(crate) struct Filer<'a> {
    /// Whatever fills `request.intake`, asked as its owner.
    pub requests: &'a dyn Fills,
    /// The member, as the media server holds their account.
    pub member: &'a crate::ports::service::Member,
    /// Where asks filed or kept as the request service's owner are kept.
    pub kept: &'a std::path::Path,
}

/// What a title is called, as the request service named it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Named<'a> {
    /// What it is called.
    pub called: Option<&'a str>,
    /// The year it came out.
    pub year: Option<u16>,
}

/// File the ask, linking the member to an account on the request service first where it
/// holds none, and say whether it waits for approval. Where it still holds none, the ask is
/// filed as the request service's owner and kept beside the member, so it counts as
/// theirs; for a member whose asks wait for approval it is kept instead, joining the ask
/// already kept for the title where one waits, and filed as the owner only once it is
/// approved.
///
/// # Errors
///
/// The refusal for what answers not answering, for an ask that cannot be kept, and for a
/// service that let through an ask from a member whose asks wait for approval: what it
/// filed is not reported as granted.
pub(crate) async fn filed(
    ctx: &Ctx,
    filer: &Filer<'_>,
    theirs: &OwnAllowance,
    (wish, named): (&Wish, Named<'_>),
    seasons: &[u32],
) -> Result<bool, Box<Problem>> {
    let requests = filer.requests;
    let account = match &theirs.id {
        Some(id) => Some(id.clone()),
        None => linked(requests, &filer.member.id).await?,
    };
    let keeping = account.is_none().then_some(filer.kept);
    if let (Some(path), false) = (keeping, theirs.approves_own) {
        let asking = as_owner::Asking {
            kind: wish.kind,
            title: &wish.id,
            seasons,
            called: named.called,
            year: named.year,
            member: filer.member,
            counts: counted(wish.kind, seasons),
            at: ctx.stamp(),
        };
        as_owner::waits(path, asking).map_err(|_| not_working())?;
        return Ok(true);
    }
    let asked = requests
        .ask(
            account.as_deref(),
            &Wish {
                id: wish.id.clone(),
                kind: wish.kind,
                seasons: seasons.to_vec(),
            },
        )
        .await
        .map_err(|_| not_working())?;
    if let Some(path) = keeping {
        let filing = as_owner::Filing {
            request: asked.request,
            title: &wish.id,
            member: filer.member,
            kind: wish.kind,
            counts: counted(wish.kind, seasons),
            now: ctx.stamp(),
            standing: &theirs.standing,
            read: theirs.read,
        };
        as_owner::filed(path, filing).map_err(|_| not_working())?;
    }
    if !theirs.approves_own && !asked.waiting {
        return Err(not_working());
    }
    Ok(asked.waiting)
}

/// `seasons` less what `held`, the member's own asks kept for approval, already waits
/// for; nothing where that leaves nothing more to ask for.
fn unheld(seasons: Vec<u32>, held: Option<&HeldBack>) -> Option<Vec<u32>> {
    match held {
        None => Some(seasons),
        Some(HeldBack::Whole) => None,
        Some(HeldBack::Seasons(_)) if seasons.is_empty() => Some(seasons),
        Some(HeldBack::Seasons(numbers)) => {
            let left: Vec<u32> = seasons
                .into_iter()
                .filter(|season| !numbers.contains(season))
                .collect();
            (!left.is_empty()).then_some(left)
        }
    }
}

/// The request service's account for the member `id` names, once asked to link them: none
/// where it holds none, as where it links nobody it could not find by their id.
///
/// # Errors
///
/// The refusal for what answers not answering.
async fn linked(requests: &dyn Fills, id: &str) -> Result<Option<String>, Box<Problem>> {
    match requests.link_members(&[id.to_owned()]).await {
        Ok(()) | Err(Failure::Unsupported { .. }) => {}
        Err(_) => return Err(not_working()),
    }
    let held = requests.requesting(id).await.map_err(|_| not_working())?;
    Ok(held.map(|held| held.id))
}

/// How much of a member's allowance asking for `seasons` of a title of `kind` uses: one
/// for a film, and one for each season of a series, a series asked for whole counting as
/// one where it names none.
fn counted(kind: Kind, seasons: &[u32]) -> u32 {
    match kind {
        Kind::Movies => 1,
        Kind::Tv => u32::try_from(seasons.len()).unwrap_or(u32::MAX).max(1),
    }
}

/// What a member's period counts and allows of `kind`.
const fn of_kind(theirs: &OwnAllowance, kind: Kind) -> Left {
    match kind {
        Kind::Movies => theirs.headroom.films,
        Kind::Tv => theirs.headroom.television,
    }
}

/// What is left of each kind once `cost` of `kind` is counted.
fn leaves(theirs: &OwnAllowance, kind: Kind, cost: u32) -> LeftAfter {
    let after = |of: Kind| {
        let remaining = of_kind(theirs, of).remaining()?;
        Some(if of == kind {
            remaining.saturating_sub(cost)
        } else {
            remaining
        })
    };
    LeftAfter {
        films: after(Kind::Movies),
        television: after(Kind::Tv),
        music: None,
        books: None,
    }
}

/// The refusal for an allowance with nothing left, saying when it frees up where that is
/// known.
pub(super) fn nothing_left(theirs: &OwnAllowance) -> Box<Problem> {
    let summary = "You've asked for all you can for now.";
    match theirs.asking.frees_up.and_then(Instant::day) {
        Some(day) => refused_with(
            NOTHING_LEFT,
            summary,
            &format!("You can ask again from {day}."),
        ),
        None => refused(NOTHING_LEFT, summary),
    }
}

/// What is said where somebody already asked for all of it.
pub(super) const ON_ITS_WAY: &str = "Somebody already asked for this, and it's on its way.";

/// What is said where the member's own ask for all of it still waits for approval.
const ALREADY_ASKED: &str =
    "You've already asked for this. Whoever runs the house will say yes or no.";

#[cfg(test)]
mod tests;

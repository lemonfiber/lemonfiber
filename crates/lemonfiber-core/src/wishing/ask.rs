//! A member's ask for one title, checked by the core before anything is asked.
//!
//! The order is the requirement's: the member's limits and the house's own room and the
//! member's allowance are all settled before the request service is asked to file
//! anything, and a rehearsal settles all of them and files nothing.

use super::{not_working, reached, refused, refused_with, Reached};
use crate::app::Ctx;
use crate::error::codes::wish::{
    ALREADY_HERE, CANNOT_FETCH, NOTHING_LEFT, NOT_ASKABLE, NOT_ITS_SEASON, NO_ROOM,
};
use crate::error::Problem;
use crate::household::run::OwnAllowance;
use crate::model::{AskedReport, Became, LeftAfter};
use crate::ports::media::Kind;
use crate::ports::service::{Detail, Left, MediaStatus, Season, Wish};

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
    if !reached.kinds.contains(&wish.kind) {
        return Err(refused(
            CANNOT_FETCH,
            &format!(
                "This house can't fetch {} right now.",
                plural_noun(wish.kind)
            ),
        ));
    }
    let detail = admitted(&reached, &wish).await?;
    let theirs = crate::household::run::allowance_of(
        reached.requests.as_ref(),
        &reached.member,
        ctx.seams.clock.now(),
    )
    .await
    .map_err(|_| not_working())?;
    let seasons = match chosen(&wish, &detail)? {
        Choice::Here => {
            return Err(refused(ALREADY_HERE, "It's already in the house."));
        }
        Choice::OnItsWay => {
            return Ok(AskedReport {
                member: reached.member.name,
                title: wish.id,
                kind: wish.kind.into(),
                seasons: wish.seasons,
                outcome: Became::OnItsWay,
                leaves: leaves(&theirs, wish.kind, 0),
                released: detail.released,
                said: ON_ITS_WAY.to_owned(),
                rehearsed: ctx.dry_run,
            });
        }
        Choice::Ask(seasons) => seasons,
    };
    if crate::space::run::admits(ctx).await.is_err() {
        return Err(refused(
            NO_ROOM,
            "The house has no room for new things right now.",
        ));
    }
    let cost = counted(wish.kind, &seasons);
    let left = of_kind(&theirs, wish.kind);
    if left.remaining().is_some_and(|remaining| remaining < cost) {
        return Err(nothing_left(&theirs));
    }
    let leaves = leaves(&theirs, wish.kind, cost);
    let waiting = if ctx.dry_run {
        !theirs.approves_own
    } else {
        filed(&reached, &theirs, &wish, &seasons).await?
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
        said: said(outcome, detail.released, &leaves, ctx.dry_run),
        seasons,
        outcome,
        leaves,
        released: detail.released,
        rehearsed: ctx.dry_run,
    })
}

/// What `wish` names, where it is a title the member may ask for.
///
/// # Errors
///
/// The refusal for a title nobody knows and for one outside the member's limits, which
/// are the same answer, and for what answers not answering.
async fn admitted(reached: &Reached, wish: &Wish) -> Result<Detail, Box<Problem>> {
    let unknown = || refused(NOT_ASKABLE, "There's no such title to ask for.");
    if wish.id.trim().is_empty() {
        return Err(unknown());
    }
    let detail = reached
        .requests
        .detail(wish.kind, &wish.id, reached.limits.region())
        .await
        .map_err(|_| not_working())?
        .ok_or_else(unknown)?;
    if reached.limits.restricted() && !reached.limits.admits(detail.certification.as_deref()) {
        return Err(unknown());
    }
    Ok(detail)
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

/// File the ask, making the member an account on the request service first where it
/// holds none, and say whether it waits for approval.
///
/// # Errors
///
/// The refusal for what answers not answering, and for a service that let through an
/// ask from a member whose asks wait for approval: what it filed is not reported as
/// granted.
async fn filed(
    reached: &Reached,
    theirs: &OwnAllowance,
    wish: &Wish,
    seasons: &[u32],
) -> Result<bool, Box<Problem>> {
    let requests = reached.requests.as_ref();
    let account = if let Some(id) = &theirs.id {
        id.clone()
    } else {
        requests
            .link_members(std::slice::from_ref(&reached.member.id))
            .await
            .map_err(|_| not_working())?;
        requests
            .requesting(&reached.member.id)
            .await
            .map_err(|_| not_working())?
            .ok_or_else(not_working)?
            .id
    };
    let asked = requests
        .ask(
            &account,
            &Wish {
                id: wish.id.clone(),
                kind: wish.kind,
                seasons: seasons.to_vec(),
            },
        )
        .await
        .map_err(|_| not_working())?;
    if !theirs.approves_own && !asked.waiting {
        return Err(not_working());
    }
    Ok(asked.waiting)
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
    }
}

/// The refusal for an allowance with nothing left, saying when it frees up where that is
/// known.
fn nothing_left(theirs: &OwnAllowance) -> Box<Problem> {
    let summary = "You've asked for all you can for now.";
    match &theirs.asking.frees_up {
        Some(at) => refused_with(
            NOTHING_LEFT,
            summary,
            &format!(
                "You can ask again from {}.",
                at.split('T').next().unwrap_or(at)
            ),
        ),
        None => refused(NOTHING_LEFT, summary),
    }
}

/// What is said where somebody already asked for all of it.
const ON_ITS_WAY: &str = "Somebody already asked for this, and it's on its way.";

/// What an ask came to, or would come to, in one household sentence.
fn said(outcome: Became, released: bool, leaves: &LeftAfter, rehearsed: bool) -> String {
    let mut said = match (outcome, rehearsed) {
        (Became::Waiting, true) => {
            "Asking for this waits for whoever runs the house to say yes.".to_owned()
        }
        (_, true) => "Asking for this brings it without anybody saying yes first.".to_owned(),
        (Became::Waiting, false) => "Asked. Whoever runs the house will say yes or no.".to_owned(),
        (_, false) => "Asked. It's on its way.".to_owned(),
    };
    if !released {
        said.push_str(" It isn't out yet, so it arrives once it is.");
    }
    if let Some(room) = room(leaves) {
        said.push(' ');
        said.push_str(&room);
    }
    said
}

/// What is left afterwards, in titles of each kind and never as a size; nothing where no
/// limit holds the member.
fn room(leaves: &LeftAfter) -> Option<String> {
    let films = leaves.films.map(|films| more(films, "film", ""));
    let television = leaves
        .television
        .map(|seasons| more(seasons, "season", " of television"));
    let parts: Vec<String> = films.into_iter().chain(television).collect();
    (!parts.is_empty()).then(|| format!("After this, there's room for {}.", parts.join(" and ")))
}

/// `count` more of `noun`, as a household says it: none is "no more".
fn more(count: u32, noun: &str, after: &str) -> String {
    let many = if count == 0 {
        "no".to_owned()
    } else {
        count.to_string()
    };
    format!("{many} more {noun}{}{after}", plural(count))
}

/// The ending a count of `many` takes.
fn plural(many: u32) -> &'static str {
    crate::plural::s(usize::try_from(many).unwrap_or(usize::MAX))
}

/// What the household calls titles of `kind`, many of them.
const fn plural_noun(kind: Kind) -> &'static str {
    match kind {
        Kind::Movies => "films",
        Kind::Tv => "series",
    }
}

#[cfg(test)]
mod tests;

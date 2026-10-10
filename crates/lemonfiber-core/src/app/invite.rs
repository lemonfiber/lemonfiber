//! Offering somebody an account, taking back the ones nobody took up, and putting one
//! back to being claimable.
//!
//! All three are the same errand seen from different sides, which is why they are one
//! file: an invitation is an account with **no password on it**, so making one, sweeping
//! one away and returning one to that state are three ways of moving the same line. Both
//! commands here end in an [`Invitation`] for the operator to pass on, and both read
//! where to send it through [`reaching`] — an address derived twice is an address the two
//! can disagree about.
//!
//! What the operator sends is an address the stack already serves. There is no page
//! of lemonfiber's for anybody to open: it runs nothing between commands, so a link
//! only it could answer would stop working the moment the operator closed the
//! terminal — which is exactly when somebody reads the message they were sent.
//!
//! **Expiry happens here rather than on a clock**, for the same reason. Nothing runs
//! in the background to sweep at the moment an invitation runs out, so the sweep is
//! done when the operator next offers one. An invitation therefore stands past its
//! window on a quiet stack, until the next offer is made.
//!
//! **Every offer is written down before it is handed back**, with when it runs out.
//! The media server's own record of when an account was made is bounded and can be
//! pushed along, and an offer nothing can date is one the sweep treats as expired —
//! see [`crate::invitation`].

mod allowing;
pub(crate) mod declining;
mod offering;
mod refusals;
mod reissuing;
pub(crate) mod standing;

pub(crate) use reissuing::reissue;

use crate::app::{Allowance, Ctx};
use crate::invitation::{Spent, HOURS_TO_CLAIM};
use crate::model::{Applied, Invitation, InvitationStanding, Linked};
use crate::ports::service::{Allowed, Household as _, Member};

use allowing::{allowing, would_not_allow};
use refusals::{
    no_credential, no_media_server, nobody_named, nowhere_to_send, runs_the_server, would_not_renew,
};
use standing::{already_here, held, renews, standing_of, take_back, Held};

/// Offer somebody an account, and withdraw any nobody claimed in time.
///
/// What they may watch is chosen here rather than left for somebody to go and set in
/// the media server afterwards: an account made open and narrowed later is open for as
/// long as it takes anybody to remember, and the person most likely to be given a limit
/// is a child who has been handed the address already.
///
/// Unconfirmed, it is the rehearsal: what the invitation would grant and for how long,
/// with nothing made and nothing taken back. What is being decided is what the person
/// will be able to see, so it is shown before the account exists rather than found
/// out afterwards from what they can see.
///
/// # Errors
///
/// Returns a [`Problem`](crate::error::Problem) where the stack has no media server
/// to hold the account, where it will not answer, where the name is the account that
/// administers it, or where no library goes by a name that was given.
pub(crate) async fn offer(
    ctx: &Ctx,
    name: String,
    allowance: Allowance,
    confirm: bool,
) -> Result<Invitation, Box<crate::error::Problem>> {
    // Trimmed, because the media server keeps the spaces and treats the result as a
    // different person: offering `ana ` beside `ana` makes a second account that
    // reads identically in every list either of them appears in.
    let name = name.trim().to_owned();
    let Reaching {
        server,
        reachable,
        manifest,
    } = reaching(ctx, &name).await?;

    let held = held(ctx, &server).await;
    let already = already_here(&held, &name).cloned();
    // Refused before anything else, a rehearsal included. This is the account the program
    // signs in as, and an offer would write a household member's limits on it.
    if let Some(member) = already
        .as_ref()
        .filter(|member| member.access.administrator)
    {
        return Err(Box::new(runs_the_server(&member.name)));
    }
    let renewing = already.as_ref().is_some_and(|member| renews(&held, member));
    // The one being offered again is not among the ones taken back. This is the account
    // the offer is *for* — so the sweep goes around it, and everybody else's expired
    // invitation is taken as before.
    let sweeping = around(&held.spent, already.as_ref());
    let standing = standing_after(already.as_ref(), renewing);
    // Resolved before the account is made, and in a rehearsal too. A library named
    // wrong is a refusal the operator is owed instead of an account, not after one —
    // and a rehearsal that skipped the check would say an invitation would be made
    // that the real run then refuses.
    let allowed = allowing(&server, &allowance).await?;

    // A rehearsal makes no account and takes none back. Both halves of this command
    // change the household, and the one that removes accounts is the half nobody
    // would want rehearsed by doing it. What it can still do is say exactly what
    // would happen, because every part of the answer is known before anything is
    // written: who it is for, the address, what has run out, and what is already
    // there under that name.
    if ctx.dry_run || !confirm {
        return Ok(Invitation {
            name: already.map_or(name, |member| member.name),
            address: reachable.url,
            // A rehearsal mints no token, so there is no address yet that would decline.
            decline: None,
            caution: reachable.caution,
            hours: HOURS_TO_CLAIM,
            withdrawn: names(&sweeping.withdrawn),
            suspended: names(&sweeping.suspended),
            rehearsed: true,
            standing,
            linked: Linked::NotTried,
            // Said in full on a rehearsal, because every part of it is known without
            // writing anything: the certificates are a read, and what would be written
            // has already been decided.
            applied: applied(&server, &allowance, allowed.as_ref(), Linked::NotTried).await,
        });
    }

    let taken = take_back(&server, &sweeping).await;

    // The account comes back from whichever this is about, so the operator is told
    // the name somebody signs in as rather than the one they typed — those differ by
    // case whenever an account was already here.
    let member = match already {
        Some(member) => {
            again(ctx, &server, &held, &member, renewing, allowed.as_ref()).await?;
            member
        }
        None => made(ctx, &server, &held, &name, allowed.as_ref()).await?,
    };

    // The account being narrowed is named only where something was written on it: an
    // offer that says nothing about access must not quietly take a permission off
    // somebody's account one service along.
    let decline = if standing == InvitationStanding::Joined {
        None
    } else {
        declining::issued(ctx, &manifest.services, &held.household, &member).await
    };

    let narrowed = allowed.as_ref().map(|_| member.id.as_str());
    let Told { linked, requesting } =
        told(ctx, &manifest, &to_link(&held, &member), narrowed).await;
    let applied = applied(&server, &allowance, allowed.as_ref(), requesting).await;

    Ok(Invitation {
        name: member.name,
        address: reachable.url,
        decline,
        caution: reachable.caution,
        hours: HOURS_TO_CLAIM,
        withdrawn: taken.withdrawn,
        suspended: taken.suspended,
        rehearsed: false,
        standing,
        linked,
        applied,
    })
}

/// What has run out, leaving out the account this offer is for.
fn around(spent: &Spent, already: Option<&Member>) -> Spent {
    let not_this = |gone: &&crate::invitation::Offered| {
        already.is_none_or(|member| member.id != gone.member.id)
    };
    Spent {
        withdrawn: spent.withdrawn.iter().filter(not_this).cloned().collect(),
        suspended: spent.suspended.iter().filter(not_this).cloned().collect(),
    }
}

/// The names of the accounts a sweep reaches.
fn names(spent: &[crate::invitation::Offered]) -> Vec<String> {
    spent.iter().map(|gone| gone.member.name.clone()).collect()
}

/// What was found where the invitation was going, once this offer has been made.
///
/// An invitation that ran out does not still stand, so offering it again is not
/// `Waiting`: one is being made now, on an account they already had — `Made` for an
/// account nobody has been in, and `Reset` for one somebody has, because what they need
/// to hear is that a password they had has stopped working.
fn standing_after(already: Option<&Member>, renewing: bool) -> InvitationStanding {
    match already {
        Some(member) if renewing && member.last_seen.is_some() => InvitationStanding::Reset,
        Some(_) if renewing => InvitationStanding::Made,
        _ => standing_of(already),
    }
}

/// Offer an account that is already here: dated again where its window had closed, and
/// narrowed where anything was chosen.
///
/// Nothing is taken back on the way out of this. The account was here before the offer
/// and is still theirs; what a failure leaves is the account as it stood.
async fn again(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    held: &Held,
    member: &Member,
    renewing: bool,
    allowed: Option<&Allowed>,
) -> Result<(), Box<crate::error::Problem>> {
    // Dated first, then switched on: the other way round, a record that would not write
    // leaves an account open with nothing to date it by.
    if renewing {
        if !offering::recorded_now(ctx, held, member) {
            return Err(Box::new(would_not_renew(&member.name)));
        }
        return offering::guarded(server, member, allowed, false).await;
    }
    // Written only where something was chosen: an offer that named neither must leave
    // what an account already here is allowed exactly as its household set it.
    let Some(allowed) = allowed else {
        return Ok(());
    };
    server
        .allow(&member.id, allowed)
        .await
        .map_err(|_| Box::new(would_not_allow(&member.name, false)))
}

/// Make the account, date it, and ready it — or none of it.
///
/// **An account that fails either is taken back.** Both are part of what the offer
/// promises: a window, and an account narrowed as the operator chose and bounded against
/// a stranger guessing at it. The media server makes an account open to every library and
/// bounded by nothing, so one left behind by a failure half way is the least guarded
/// account the household holds.
async fn made(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    held: &Held,
    name: &str,
    allowed: Option<&Allowed>,
) -> Result<Member, Box<crate::error::Problem>> {
    let member = server
        .invite(name)
        .await
        .map_err(|failure| Box::new(crate::error::Diagnose::problem(&failure)))?;
    let finished = if offering::recorded_now(ctx, held, &member) {
        offering::guarded(server, &member, allowed, true).await
    } else {
        Err(Box::new(offering::unrecorded(&member.name)))
    };
    let Err(refusal) = finished else {
        return Ok(member);
    };
    Err(Box::new(undone(server, &member, *refusal).await))
}

/// Take back an account a refused offer made, saying so on the refusal.
///
/// Where the media server will not take it back either, the refusal says which account
/// was left and what it is, because that is now the operator's to remove.
async fn undone(
    server: &crate::jellyfin::Jellyfin,
    member: &Member,
    refusal: crate::error::Problem,
) -> crate::error::Problem {
    if server.withdraw(&member.id).await.is_ok() {
        return refusal;
    }
    refusal.or_try(crate::error::Remedy::new(format!(
        "The account {} was made and could not be taken back: it has no password and \
         nothing held back. Remove it in the media server's own settings",
        member.name
    )))
}

/// Hold what this person may ask for to the same decision as what they may watch.
///
/// **This is the hole the setting exists to close.** A limit on the media server decides
/// what an account is offered; it says nothing at all about what that account may ask
/// the request service to fetch, and a child who cannot watch something but can pull it
/// into the library has been given half a limit. The request service has no notion of a
/// content rating, so what it can be told instead is the difference that matters: what
/// this person asks for waits for somebody to see it.
///
/// Reached with the service already signed in, because telling it about the household
/// and holding one of them are one errand: see [`told`].
async fn holding(access: &crate::app::targets::HouseholdAccess, member: &str) -> Linked {
    match access.requests.requesting(member).await {
        // Nothing to hold rather than a failure to hold something: a member this
        // service has never heard of has no second permission to disagree with the
        // first, and the next run makes the account and holds it then.
        Ok(None) => Linked::NotTried,
        Ok(Some(requesting)) if !requesting.approves_own => Linked::Made,
        Ok(Some(requesting)) => match access.requests.approval_first(&requesting.id).await {
            Ok(()) => Linked::Made,
            Err(_) => Linked::NotYet,
        },
        Err(_) => Linked::NotYet,
    }
}

/// What was written on the account, said back in the household's own words.
///
/// **Said so an absence later is explicable.** A restricted member who cannot find half
/// the library is either this setting working or a defect, and an operator with nothing
/// on record cannot tell which — so what was applied travels back on the answer that
/// applied it, including what happened to content the server has no rating for.
///
/// The certificates come off the media server, because the table is the operator's
/// country's rather than this product's. A server that will not answer costs the names
/// and not the limit: the words for the number still read, and what stands in for the
/// names says it stood in.
async fn applied(
    server: &crate::jellyfin::Jellyfin,
    allowance: &Allowance,
    allowed: Option<&crate::ports::service::Allowed>,
    requesting: Linked,
) -> Option<Applied> {
    let allowed = allowed?;
    let certificates = server.ratings().await.unwrap_or_default();
    Some(Applied {
        limit: allowance
            .age_limit
            .map(|age| crate::rating::reading(&certificates, Some(age))),
        libraries: allowance.libraries.clone(),
        unrated: allowed.unrated.unwrap_or_default(),
        requesting,
        filtering: crate::age_limit::A_FILTER_NOT_A_LOCK.to_owned(),
    })
}

/// Everybody the media server holds now, as the identifiers the request service reads.
///
/// **Everybody, not only the person just invited.** The request service skips anybody
/// it already holds, so sending the whole household is what completes a link an earlier
/// run could not make — and it completes it without anything having been written down
/// in between, which is the only kind of "later" that survives this program being
/// closed, reinstalled, or run from somewhere else.
///
/// The invitations just removed are left out: they no longer have an account.
fn to_link(held: &Held, made: &Member) -> Vec<String> {
    let mut linking: Vec<String> = held
        .household
        .iter()
        .filter(|member| {
            !held
                .spent
                .withdrawn
                .iter()
                .any(|gone| gone.member.id == member.id)
        })
        .map(|member| member.id.clone())
        .collect();
    // The account just made was not in the household when it was read.
    if !linking.contains(&made.id) {
        linking.push(made.id.clone());
    }
    linking
}

/// What the request service was told, in the two things there are to tell it.
struct Told {
    /// Whether it holds an account for everybody the media server does.
    linked: Linked,
    /// Whether what the one being narrowed asks for was held to the same decision.
    requesting: Linked,
}

/// Tell the request service about them, and hold the narrowed one to what they may
/// watch — where there is a service and it can be reached.
///
/// **One errand rather than two**, because it is one sign-in and one set of reasons it
/// could not be made: a second reach would be a second chance to disagree about whether
/// the service answered at all.
///
/// **Best-effort by design.** The account on the media server is what an invitation
/// *is*, and it stands whether or not a second service is up — so a request service
/// that will not answer is reported rather than allowed to refuse the invitation. What
/// the person cannot do yet is worth a line; it is not worth the account.
async fn told(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    members: &[String],
    narrowed: Option<&str>,
) -> Told {
    let Some(access) = crate::app::targets::seerr_reader(ctx, manifest).await else {
        return Told {
            linked: Linked::NotTried,
            requesting: Linked::NotTried,
        };
    };
    if access.requests.answers().await.is_err() {
        return Told {
            linked: Linked::NotYet,
            requesting: Linked::NotYet,
        };
    }
    let linked = if access.requests.link_members(members).await.is_err() {
        Linked::NotYet
    } else {
        Linked::Made
    };
    Told {
        linked,
        // After the link, because there has to be an account over there to hold.
        requesting: match narrowed {
            None => Linked::NotTried,
            Some(member) => holding(&access, member).await,
        },
    }
}

/// What both halves of this errand need before either can act: a way to reach the media
/// server, and the address a person reaches it at.
struct Reaching {
    /// The media server, signed in as this program.
    server: crate::jellyfin::Jellyfin,
    /// Where a *person* opens it.
    reachable: crate::door::Address,
    /// The stack, whose request service and media server are found in it — boxed,
    /// because it is carried across every await of an invitation.
    manifest: Box<lemonfiber_manifest::Manifest>,
}

/// Everything an invitation or a reissue needs before it touches anything.
///
/// Shared because both send the same message in the end, and an address derived twice is
/// an address that can differ between the two.
async fn reaching(ctx: &Ctx, name: &str) -> Result<Reaching, Box<crate::error::Problem>> {
    if name.is_empty() {
        return Err(Box::new(nobody_named()));
    }
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(crate::error::Diagnose::problem(&err)))?;
    // Only the client and the port are carried on: the server as the lookup resolved
    // it is not held across what follows.
    let (server, port) = {
        let Some(media) = super::targets::hosted(ctx, &manifest) else {
            return Err(Box::new(no_media_server()));
        };
        let Some(server) = media.administered(ctx) else {
            return Err(Box::new(no_credential()));
        };
        (
            server,
            crate::screening::door::household_port(&manifest, media.port),
        )
    };
    let Some(reachable) = household_address(ctx, port).await else {
        return Err(Box::new(nowhere_to_send()));
    };
    Ok(Reaching {
        server,
        reachable,
        manifest: Box::new(manifest),
    })
}

/// Where a *person* reaches the media server on `port`.
///
/// Neither of the URLs the stack wires itself with: those name a host only this machine or
/// this stack can resolve, and a message carrying one sends somebody an address that cannot
/// open. Asked now rather than remembered, so a machine renamed since the last look answers
/// as it is. Where there is no name and nothing recorded there is no address rather than a
/// guess — an invented one is the one thing that gets sent on, and what gets sent on has to
/// be true.
///
/// Shared by everything that hands a person the way in, because an address derived twice is
/// an address that can differ between the two.
pub(crate) async fn household_address(ctx: &Ctx, port: u16) -> Option<crate::door::Address> {
    let named = ctx.site.name().await;
    crate::door::address(
        named.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.environment,
        port,
    )
}

#[cfg(test)]
mod tests;

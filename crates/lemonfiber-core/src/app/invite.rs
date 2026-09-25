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
//! done when the operator next offers one. An invitation therefore stands a little
//! past its window on a quiet stack, which is the direction to err in: it is an
//! account nobody has claimed, reachable only by somebody who was told about it.
//!
//! **An account offered again is dated from when it was offered again.** Once a password
//! can be taken off an existing account, "unclaimed" stops meaning "new": the record that
//! an account was *made* is months old for a household member, so a reset read that way
//! would be expired before anybody was told about it, and the next offer to anybody would
//! withdraw it. The media server records the reset too, and
//! [`offered`](crate::invitation::offered) takes the later of the two.

mod allowing;
mod refusals;
mod reissuing;
mod standing;

pub(crate) use reissuing::reissue;

use crate::app::{Allowance, Ctx};
use crate::invitation::{Offered, HOURS_TO_CLAIM};
use crate::model::{Applied, Invitation, InvitationStanding, Linked};
use crate::ports::service::{Household as _, Member, Requests as _};

use allowing::{allowing, would_not_allow};
use refusals::{no_credential, no_media_server, nobody_named, nowhere_to_send, would_not_renew};
use standing::{already_here, has_run_out, held, standing_of, take_back, Held};

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
/// to hold the account, where it will not answer, or where no library goes by a name
/// that was given.
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
        services,
    } = reaching(ctx, &name).await?;

    let held = held(ctx, &server).await;
    let already = already_here(&held, &name).cloned();
    // The one being offered again is not among the ones taken back. Withdrawing means
    // removing the account, and this is the account the offer is *for* — so the sweep
    // goes around it, and everybody else's expired invitation is taken as before.
    let renewing = already
        .as_ref()
        .is_some_and(|member| has_run_out(&held, member));
    let sweeping: Vec<Offered> = held
        .spent
        .iter()
        .filter(|gone| {
            already
                .as_ref()
                .is_none_or(|member| member.id != gone.member.id)
        })
        .cloned()
        .collect();
    // `Made` rather than `Waiting`, because an invitation that ran out does not still
    // stand — one is being made now, on an account they already had. That the account
    // is not new is the requirement being met and not something to report: what the
    // operator sends is the same either way.
    let standing = if renewing {
        InvitationStanding::Made
    } else {
        standing_of(already.as_ref())
    };
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
            caution: reachable.caution,
            hours: HOURS_TO_CLAIM,
            withdrawn: sweeping.into_iter().map(|it| it.member.name).collect(),
            rehearsed: true,
            standing,
            linked: Linked::NotTried,
            // Said in full on a rehearsal, because every part of it is known without
            // writing anything: the certificates are a read, and what would be written
            // has already been decided.
            applied: applied(&server, &allowance, allowed.as_ref(), Linked::NotTried).await,
        });
    }

    let withdrawn = take_back(&server, &sweeping).await;

    // The account comes back from whichever this is about, so the operator is told
    // the name somebody signs in as rather than the one they typed — those differ by
    // case whenever an account was already here.
    let member = if let Some(member) = already {
        // An invitation that ran out is offered again by dating it again. The account
        // already has no password, so taking one off changes nothing about it — what it
        // does is write the record that says when it was offered, which is what the
        // window is counted from. Refused rather than glossed: the message about to be
        // sent promises a window, and one that will not be honoured is worse than none.
        if renewing {
            server
                .unclaim(&member.id)
                .await
                .map_err(|_| Box::new(would_not_renew(&member.name)))?;
        }
        member
    } else {
        server
            .invite(&name)
            .await
            .map_err(|failure| Box::new(crate::error::Diagnose::problem(&failure)))?
    };

    // Written on the account, which is why it happens after there is one. Nothing is
    // written where nothing was chosen: an offer that named neither must leave what an
    // account already here is allowed exactly as its household set it.
    if let Some(allowed) = &allowed {
        server
            .allow(&member.id, allowed)
            .await
            .map_err(|_| Box::new(would_not_allow(&member.name)))?;
    }

    // The account being narrowed is named only where something was written on it: an
    // offer that says nothing about access must not quietly take a permission off
    // somebody's account one service along.
    let narrowed = allowed.as_ref().map(|_| member.id.as_str());
    let Told { linked, requesting } =
        told(ctx, &services, &to_link(&held, &member), narrowed).await;
    let applied = applied(&server, &allowance, allowed.as_ref(), requesting).await;

    Ok(Invitation {
        name: member.name,
        address: reachable.url,
        caution: reachable.caution,
        hours: HOURS_TO_CLAIM,
        withdrawn,
        rehearsed: false,
        standing,
        linked,
        applied,
    })
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
    match access.seerr.requesting(member).await {
        // Nothing to hold rather than a failure to hold something: a member this
        // service has never heard of has no second permission to disagree with the
        // first, and the next run makes the account and holds it then.
        Ok(None) => Linked::NotTried,
        Ok(Some(requesting)) if !requesting.approves_own => Linked::Made,
        Ok(Some(requesting)) => match access.seerr.approval_first(&requesting.id).await {
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
/// The invitations just taken back are left out: they no longer have an account.
fn to_link(held: &Held, made: &Member) -> Vec<String> {
    let mut linking: Vec<String> = held
        .household
        .iter()
        .filter(|member| !held.spent.iter().any(|gone| gone.member.id == member.id))
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
    services: &[lemonfiber_manifest::Service],
    members: &[String],
    narrowed: Option<&str>,
) -> Told {
    let Some(access) = crate::app::targets::seerr_reader(ctx, services) else {
        return Told {
            linked: Linked::NotTried,
            requesting: Linked::NotTried,
        };
    };
    if access
        .seerr
        .sign_in(crate::config::JELLYFIN_ADMIN_USER, &access.password)
        .await
        .is_err()
    {
        return Told {
            linked: Linked::NotYet,
            requesting: Linked::NotYet,
        };
    }
    let linked = if access.seerr.link_members(members).await.is_err() {
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
    /// The stack's services, which the request service is found among.
    services: Vec<lemonfiber_manifest::Service>,
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
    let Some(jellyfin) = super::seed::identity::jellyfin_service(&manifest.services) else {
        return Err(Box::new(no_media_server()));
    };
    let Some(password) = super::seed::identity::recorded_jellyfin_password(ctx) else {
        return Err(Box::new(no_credential()));
    };
    // Where a *person* reaches the media server, which is neither of the URLs the
    // stack wires itself with: those name a host only this machine or this stack can
    // resolve, and an invitation carrying one sends somebody an address that cannot
    // open. Asked now rather than remembered, so a machine renamed since the last
    // look answers as it is. Where there is no name and nothing recorded there is no
    // address rather than a guess — an invented one is the one thing that gets sent
    // on, and what gets sent on has to be true.
    let named = ctx.site.name().await;
    let Some(reachable) = crate::door::address(
        named.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.environment,
        jellyfin.port,
    ) else {
        return Err(Box::new(nowhere_to_send()));
    };
    Ok(Reaching {
        server: crate::jellyfin::Jellyfin::authenticated(
            ctx.seams.http.clone(),
            &jellyfin.loopback,
            "jellyfin",
            crate::config::JELLYFIN_ADMIN_USER,
            &password,
        ),
        reachable,
        services: manifest.services,
    })
}

#[cfg(test)]
mod tests;

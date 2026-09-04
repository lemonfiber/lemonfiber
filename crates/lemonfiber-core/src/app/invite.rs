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
mod reissuing;

pub(super) use reissuing::reissued;

use crate::app::{Allowance, Ctx};
use crate::invitation::{offered, run_out, Offered, HOURS_OF_RECORD, HOURS_TO_CLAIM};
use crate::model::{Applied, Invitation, InvitationStanding, Linked};
use crate::ports::service::{Household as _, Member, Requests as _};

use allowing::{allowing, would_not_allow};

/// Offer somebody an account, and withdraw any nobody claimed in time.
///
/// What they may watch is chosen here rather than left for somebody to go and set in
/// the media server afterwards: an account made open and narrowed later is open for as
/// long as it takes anybody to remember, and the person most likely to be given a limit
/// is a child who has been handed the address already.
///
/// # Errors
///
/// Returns a [`Problem`](crate::error::Problem) where the stack has no media server
/// to hold the account, where it will not answer, or where no library goes by a name
/// that was given.
pub(super) async fn offer(
    ctx: &Ctx,
    name: String,
    allowance: Allowance,
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
    if ctx.dry_run {
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

    let linked = link(ctx, &services, &to_link(&held, &member)).await;
    // After the link, because there has to be an account over there to hold. A member
    // the request service has never heard of has no second permission to disagree with
    // the first, which is not the same as one whose permissions could not be read.
    let requesting = if allowed.is_some() {
        holding(ctx, &services, &member.id).await
    } else {
        Linked::NotTried
    };
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
/// Best-effort for the reason the link is: the account on the media server is what an
/// invitation *is*, and it stands whether or not a second service is up. What could not
/// be held is reported rather than allowed to refuse the invitation.
async fn holding(ctx: &Ctx, services: &[lemonfiber_manifest::Service], member: &str) -> Linked {
    let Some(access) = crate::app::targets::seerr_reader(ctx, services) else {
        return Linked::NotTried;
    };
    if access
        .seerr
        .sign_in(crate::config::JELLYFIN_ADMIN_USER, &access.password)
        .await
        .is_err()
    {
        return Linked::NotYet;
    }
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

/// Tell the request service about them, where there is one and it can be reached.
///
/// **Best-effort by design.** The account on the media server is what an invitation
/// *is*, and it stands whether or not a second service is up — so a request service
/// that will not answer is reported rather than allowed to refuse the invitation. What
/// the person cannot do yet is worth a line; it is not worth the account.
async fn link(ctx: &Ctx, services: &[lemonfiber_manifest::Service], members: &[String]) -> Linked {
    let Some(access) = crate::app::targets::seerr_reader(ctx, services) else {
        return Linked::NotTried;
    };
    if access
        .seerr
        .sign_in(crate::config::JELLYFIN_ADMIN_USER, &access.password)
        .await
        .is_err()
    {
        return Linked::NotYet;
    }
    if access.seerr.link_members(members).await.is_err() {
        return Linked::NotYet;
    }
    Linked::Made
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
            ctx.http.clone(),
            &jellyfin.loopback,
            "jellyfin",
            crate::config::JELLYFIN_ADMIN_USER,
            &password,
        ),
        reachable,
        services: manifest.services,
    })
}

/// What was found where the invitation was going.
fn standing_of(already: Option<&Member>) -> InvitationStanding {
    match already {
        Some(member) if member.claimed => InvitationStanding::Joined,
        // Unclaimed, but somebody has been in it: their password was taken off rather
        // than an offer they never took up. Told apart because the message differs —
        // nobody is being invited, and what they need to hear is that a password they
        // had has stopped working.
        Some(member) if member.last_seen.is_some() => InvitationStanding::Reset,
        Some(_) => InvitationStanding::Waiting,
        None => InvitationStanding::Made,
    }
}

/// The account this invitation is for, where the household already holds one.
///
/// **Matched without regard to case**, because the media server refuses a second
/// account whose name differs from an existing one only in case — so a match missed
/// here walks straight into the refusal this exists to prevent, and the operator is
/// handed the server's own word for it, which is `400`.
///
/// **The ones that have run out are counted too**, and that is the whole of offering
/// an expired invitation again without making somebody a second time. The account is
/// already theirs; what has run out is the window on it, and a window is restarted by
/// dating the invitation again rather than by building a new account to carry it. Left
/// out, this run would withdraw the account — which is to say delete it — and then make
/// another under the same name with a different identifier, so anything already linked
/// to them would be linked to somebody who no longer exists.
fn already_here<'a>(held: &'a Held, name: &str) -> Option<&'a Member> {
    let asked = name.to_lowercase();
    held.household
        .iter()
        .find(|member| member.name.to_lowercase() == asked)
}

/// Whether this account is one the sweep was about to take back.
fn has_run_out(held: &Held, member: &Member) -> bool {
    held.spent.iter().any(|gone| gone.member.id == member.id)
}

/// What the media server holds right now, as this command needs to see it.
struct Held {
    /// Every account it has, claimed or not.
    household: Vec<Member>,
    /// The invitations among them that have run out.
    spent: Vec<Offered>,
}

/// The invitations nobody claimed in time, as the media server holds them now.
///
/// Best-effort: a media server that will not answer is not a reason to refuse the
/// invitation the operator asked for. The sweep runs again next time.
///
/// **Reading and acting are separate** so that a rehearsal can do the first without
/// the second — the whole of what `--dry-run` promises is that the second does not
/// happen, and a sweep that removed accounts on the way to saying what it would do
/// would be the flag doing the damage it exists to prevent.
async fn held(ctx: &Ctx, server: &crate::jellyfin::Jellyfin) -> Held {
    let cutoff = ctx.hours_ago(HOURS_TO_CLAIM);
    let since = ctx.hours_ago(HOURS_OF_RECORD);
    let (Ok(household), Ok(records)) =
        (server.household().await, server.when_invited(&since).await)
    else {
        return Held {
            household: Vec::new(),
            spent: Vec::new(),
        };
    };
    let waiting = offered(household.clone(), &records);
    Held {
        household,
        spent: run_out(&waiting, &cutoff).into_iter().cloned().collect(),
    }
}

/// Take back the invitations that have run out, naming the ones actually taken.
///
/// A server that refuses one is not reported as having given it back: the operator
/// reads this list as what is gone.
async fn take_back(server: &crate::jellyfin::Jellyfin, spent: &[Offered]) -> Vec<String> {
    let mut taken = Vec::new();
    for invitation in spent {
        if server.withdraw(&invitation.member.id).await.is_ok() {
            taken.push(invitation.member.name.clone());
        }
    }
    taken
}

/// Said where the stack holds no media server: there is nothing to make an account on.
fn no_media_server() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-1"),
        crate::error::Severity::Error,
        "this stack has no media server, so there is no account to offer",
        "An invitation is an account on the media server; without one there is nothing \
         for somebody to sign in to",
        crate::error::Remedy::new("Add a media server to the stack and run setup"),
    )
}

/// Said where the invitation is for nobody: the name is blank, or only spaces.
///
/// The media server refuses this too, in its own words, which are `400` and a link
/// to the specification of that status. The operator asked for something reasonable
/// and mistyped it, and is owed a sentence about the name rather than about HTTP.
fn nobody_named() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-4"),
        crate::error::Severity::Error,
        "an invitation needs somebody to be for",
        "The name is what they will sign in as, so a blank one is an account nobody \
         could use",
        crate::error::Remedy::new("Give the name they will sign in as")
            .with_detail("lemonfiber invite ana"),
    )
}

/// Said where this machine has no address the household could arrive at.
///
/// An invitation is an address somebody else types. Sending one built from a default
/// would be sending a link that opens nothing, which is worse than saying there is
/// none: the operator would learn it had failed from whoever they invited.
fn nowhere_to_send() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-3"),
        crate::error::Severity::Error,
        "this machine has no address the household could arrive at",
        "An invitation is an address somebody else opens, and this machine answers to \
         no name on the network and has none written down",
        crate::error::Remedy::new("Record the address the household should use")
            .with_detail("lemonfiber config set HOUSEHOLD_HOST <address>"),
    )
}

/// Said where an expired invitation could not be dated again, so its window is not real.
///
/// The account is untouched and still theirs — what failed is the write that says when it
/// was offered. Reported rather than glossed over because the message the operator is
/// about to send promises a window, and this one would be counted from whenever the
/// invitation was first made, which has already passed.
fn would_not_renew(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-5"),
        crate::error::Severity::Error,
        format!("the media server would not offer {name}'s invitation again"),
        "Their account is still there and still has no password on it; what could not be \
         written is when it was offered, which is what the window is counted from",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}

/// Said where the admin credential was never recorded: nothing can be asked of the server.
fn no_credential() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-2"),
        crate::error::Severity::Error,
        "the media server's own account has not been set up yet",
        "Making somebody else an account is done as the administrator, and this machine \
         has not recorded one",
        crate::error::Remedy::new("Run setup so the media server's account is made and recorded")
            .with_detail("lemonfiber setup"),
    )
}

#[cfg(test)]
mod tests {
    use super::{link, Linked};
    use crate::test_support::a_context;

    /// A stack with nothing to reach the request service with tells it nothing, and
    /// says so as a thing not tried rather than a thing that failed.
    ///
    /// Driven at `link` directly: reached through the whole command, the media
    /// server's own reader refuses first for the same missing password, so the branch
    /// this is about is never the one that answers.
    #[tokio::test]
    async fn with_no_request_service_to_reach_nothing_is_tried() {
        let ctx = a_context().build();
        let services = ctx
            .stack
            .checked_manifest(ctx.today())
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        assert!(
            !services.is_empty(),
            "the shipped stack declared no services, so this asserts nothing"
        );

        assert_eq!(
            link(&ctx, &services, &["1".to_owned()]).await,
            Linked::NotTried,
            "a stack with nothing to sign in with reported a link that failed rather \
             than one nothing was tried on"
        );
    }
}

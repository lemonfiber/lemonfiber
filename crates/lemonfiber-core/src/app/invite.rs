//! Offering somebody an account, and taking back the ones nobody took up.
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

use crate::app::Ctx;
use crate::invitation::{offered, run_out, Offered, HOURS_OF_RECORD, HOURS_TO_CLAIM};
use crate::model::{Invitation, InvitationStanding};
use crate::ports::service::{Household as _, Member};

/// Offer somebody an account, and withdraw any nobody claimed in time.
///
/// # Errors
///
/// Returns a [`Problem`](crate::error::Problem) where the stack has no media server
/// to hold the account, or where it will not answer.
pub(super) async fn offer(
    ctx: &Ctx,
    name: String,
) -> Result<Invitation, Box<crate::error::Problem>> {
    // Trimmed, because the media server keeps the spaces and treats the result as a
    // different person: offering `ana ` beside `ana` makes a second account that
    // reads identically in every list either of them appears in.
    let name = name.trim().to_owned();
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

    let server = crate::jellyfin::Jellyfin::authenticated(
        ctx.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        &password,
    );

    let held = held(ctx, &server).await;
    let already = already_here(&held, &name).cloned();
    let standing = standing_of(already.as_ref());

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
            withdrawn: held.spent.into_iter().map(|it| it.member.name).collect(),
            rehearsed: true,
            standing,
        });
    }

    let withdrawn = take_back(&server, &held.spent).await;

    // The name comes back from whichever account this is about, so the operator is
    // told the one somebody signs in as rather than the one they typed — those
    // differ by case whenever an account was already here.
    let name = if let Some(member) = already {
        member.name
    } else {
        server
            .invite(&name)
            .await
            .map_err(|failure| Box::new(crate::error::Diagnose::problem(&failure)))?
            .name
    };

    Ok(Invitation {
        name,
        address: reachable.url,
        caution: reachable.caution,
        hours: HOURS_TO_CLAIM,
        withdrawn,
        rehearsed: false,
        standing,
    })
}

/// What was found where the invitation was going.
fn standing_of(already: Option<&Member>) -> InvitationStanding {
    match already {
        Some(member) if member.claimed => InvitationStanding::Joined,
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
/// The ones about to be taken back are not counted. An invitation that has run out
/// is one this run is replacing, and treating it as already here would make an
/// expired invitation impossible to offer again.
fn already_here<'a>(held: &'a Held, name: &str) -> Option<&'a Member> {
    let asked = name.to_lowercase();
    held.household.iter().find(|member| {
        member.name.to_lowercase() == asked
            && !held.spent.iter().any(|gone| gone.member.id == member.id)
    })
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

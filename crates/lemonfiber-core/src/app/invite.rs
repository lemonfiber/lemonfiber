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
use crate::model::Invitation;
use crate::ports::service::Household as _;

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
    let server = crate::jellyfin::Jellyfin::authenticated(
        ctx.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        &password,
    );

    let spent = run_out_now(ctx, &server).await;

    // A rehearsal makes no account and takes none back. Both halves of this command
    // change the household, and the one that removes accounts is the half nobody
    // would want rehearsed by doing it. What it can still do is say exactly what
    // would happen, because every part of the answer is known before anything is
    // written: the name is the one asked for, the address is the stack's, and what
    // has run out has just been read.
    if ctx.dry_run {
        return Ok(Invitation {
            name,
            address: jellyfin.network_url,
            hours: HOURS_TO_CLAIM,
            withdrawn: spent.into_iter().map(|it| it.member.name).collect(),
            rehearsed: true,
        });
    }

    let withdrawn = take_back(&server, &spent).await;
    let member = server
        .invite(&name)
        .await
        .map_err(|failure| Box::new(crate::error::Diagnose::problem(&failure)))?;

    Ok(Invitation {
        name: member.name,
        address: jellyfin.network_url,
        hours: HOURS_TO_CLAIM,
        withdrawn,
        rehearsed: false,
    })
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
async fn run_out_now(ctx: &Ctx, server: &crate::jellyfin::Jellyfin) -> Vec<Offered> {
    let cutoff = ctx.hours_ago(HOURS_TO_CLAIM);
    let since = ctx.hours_ago(HOURS_OF_RECORD);
    let (Ok(household), Ok(records)) =
        (server.household().await, server.when_invited(&since).await)
    else {
        return Vec::new();
    };
    let waiting = offered(household, &records);
    run_out(&waiting, &cutoff).into_iter().cloned().collect()
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

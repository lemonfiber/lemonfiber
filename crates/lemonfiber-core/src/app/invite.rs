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
use crate::invitation::{offered, run_out, HOURS_OF_RECORD, HOURS_TO_CLAIM};
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

    let withdrawn = sweep(ctx, &server).await;
    let member = server
        .invite(&name)
        .await
        .map_err(|failure| Box::new(crate::error::Diagnose::problem(&failure)))?;

    Ok(Invitation {
        name: member.name,
        address: jellyfin.network_url,
        hours: HOURS_TO_CLAIM,
        withdrawn,
    })
}

/// Take back the invitations nobody claimed in time.
///
/// Best-effort: a media server that will not answer the sweep is not a reason to
/// refuse the invitation the operator asked for. The sweep runs again next time.
async fn sweep(ctx: &Ctx, server: &crate::jellyfin::Jellyfin) -> Vec<String> {
    let cutoff = ctx.hours_ago(HOURS_TO_CLAIM);
    let since = ctx.hours_ago(HOURS_OF_RECORD);
    let (Ok(household), Ok(records)) =
        (server.household().await, server.when_invited(&since).await)
    else {
        return Vec::new();
    };
    let waiting = offered(household, &records);

    let mut taken = Vec::new();
    for invitation in run_out(&waiting, &cutoff) {
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

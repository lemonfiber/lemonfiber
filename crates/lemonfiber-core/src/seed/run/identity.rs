//! Making the media server the household's single sign-in.
//!
//! One account, not two: the request service authenticates against the media server
//! rather than keeping accounts of its own.
//!
//! Which server that is comes from what the stack says the request service asks for.
//! It asks for an identity source, and whatever fills that is what it signs in
//! against — so a service standing in for the bundled one is reached without a line
//! here changing, which is the test of whether the wiring was really converted.

use super::Ctx;
use crate::app::credentials::Replacing;
use crate::app::targets::MediaServer;

/// The media server's administrator, as the first half of the identity left it: the
/// password to go on with, or the state the identity rests in without one, and the
/// request service that asked for it.
pub(super) struct Admin {
    /// The password, or the state the identity rests in without one.
    administered: Result<String, crate::seed::State>,
    /// The request service that asks for the server, as the gate cleared it.
    requests: crate::wiring::Filler,
}

/// The first half of making whatever fills the identity source the one the request
/// service signs in against: the media server's administrator, minted where its first-run
/// setup has not run.
///
/// Nothing where nothing fills the identity source, or where nothing the trust gate lets
/// the administrator's password reach asks for it: without both there is nothing to wire. The admin
/// password is the one credential minted rather than read — recorded under the server's
/// own setting on the run that mints it, before the setup is given it, and read back on a
/// later run. Recorded here, before the second half, so the steps between the two can
/// sign in with it.
pub(super) async fn seed_media_server_admin(
    ctx: &Ctx,
    server: Option<&MediaServer>,
) -> Option<Admin> {
    let server = server?;
    let requests = server.asked_by.clone()?;
    let client = server.client(ctx);
    let recorded = server.recorded_password(ctx);
    let keep = |password: &str| server.record_password(ctx, password);
    let administered = crate::seed::wire_media_server_admin(
        &client,
        ctx.seams.random.as_ref(),
        recorded.as_deref(),
        ctx.dry_run,
        &keep,
    )
    .await;
    Some(Admin {
        administered,
        requests,
    })
}

/// The second half: the request service signed in through the media server — at the
/// request gate's route to it where the stack runs the gate, and at the server's own
/// address on the stack's network where it does not — then, through the gate, handed the
/// token it reaches the server with after that.
pub(super) async fn seed_request_identity(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    expected: &crate::baseline::Baseline,
    server: Option<&MediaServer>,
    admin: Option<Admin>,
    project: Option<&std::path::Path>,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let mut records = crate::baseline::Baseline::new();
    let (
        Some(server),
        Some(Admin {
            administered,
            requests,
        }),
    ) = (server, admin)
    else {
        return (Vec::new(), records);
    };
    let gate = project.filter(|_| crate::app::gating::service(services).is_some());
    let server_url = match gate {
        Some(_) => {
            let at = super::tokens::through_the_gate(server.id());
            format!("http://{}:{}{}", at.host, at.port, at.base)
        }
        None => server.network.url(),
    };

    let Some(client) = crate::app::targets::requests_as_owner(ctx, &requests).await else {
        return (Vec::new(), records);
    };
    let wiring = match administered {
        Ok(password) => {
            crate::seed::wire_request_identity(
                client.as_ref(),
                server.protocol(),
                &password,
                &server_url,
                ctx.dry_run,
            )
            .await
        }
        Err(state) => crate::seed::Wiring::settled(crate::seed::IDENTITY.to_owned(), state),
    };

    // What the household is told and where the request service reaches the media server
    // are read and written with the request service's own key, which the setup above
    // is what writes, so the client is opened only now.
    let owner = crate::app::targets::requests_as_owner(ctx, &requests)
        .await
        .unwrap_or(client);

    let linked = match (gate, &wiring.state) {
        (Some(_), crate::seed::State::WouldWire { .. }) => {
            Some(super::linking::would_link(&requests, server))
        }
        (Some(project), crate::seed::State::Wired | crate::seed::State::AlreadyWired) => Some(
            super::linking::seed_media_server_link(ctx, owner.as_ref(), &requests, server, project)
                .await,
        ),
        _ => None,
    };

    // The run that set the request service up is the one that showed it the
    // administrator's password, so that password is changed straight after it and the
    // one the request service saw opens nothing.
    let changed = (wiring.state == crate::seed::State::Wired && !ctx.dry_run)
        .then(|| changed_after_setup(ctx, server));
    let changed = match changed {
        Some(changing) => Some(changing.await),
        None => None,
    };

    // What the household is told is its own managed field, reconciled whether or not
    // the identity above was wired this run: the identity step stops at a service
    // already initialised, and that is every install after the first.
    let (told, held) = crate::seed::wire_household_telling(
        owner.as_ref(),
        expected.entry(&requests.id, crate::seed::TELLING),
        ctx.dry_run,
    )
    .await;
    remember(&mut records, &requests.id, &told.state, &held, &ctx.stamp());

    (
        [Some(wiring), linked, changed, Some(told)]
            .into_iter()
            .flatten()
            .collect(),
        records,
    )
}

/// What the report calls the change made after the request service was set up.
fn changed(server: &MediaServer) -> String {
    format!(
        "{}'s administrator password, changed once the request service was set up",
        server.name()
    )
}

/// Change the administrator's password, and say how that went.
async fn changed_after_setup(ctx: &Ctx, server: &MediaServer) -> crate::seed::Wiring {
    let failed = match crate::app::credentials::replace_jellyfin_password(ctx, server, false).await
    {
        Ok(_) => return crate::seed::Wiring::settled(changed(server), crate::seed::State::Wired),
        Err(Replacing::Refused) => {
            format!("{} refused the password lemonfiber holds", server.name())
        }
        Err(Replacing::Unproven(detail) | Replacing::Unkept(detail)) => detail,
    };
    let mut wiring = crate::seed::Wiring::settled(
        changed(server),
        crate::seed::State::Failed { detail: failed },
    );
    wiring.escalate(
        format!(
            "The password the request service saw at setup still opens {}.",
            server.name()
        ),
        rotated_by(server),
    );
    wiring
}

/// What replaces the administrator's password by hand: the rotation for the stack's own
/// server, and the server itself for a plugin's, whose credentials lemonfiber does not
/// rotate.
fn rotated_by(server: &MediaServer) -> String {
    match server.brought_by() {
        None => format!("Run `lemonfiber credentials rotate {}`.", server.id()),
        Some(_) => format!(
            "Change the administrator's password in {} itself, then record it with \
             `lemonfiber config set {} <password>`.",
            server.name(),
            server.setting
        ),
    }
}

/// Write down what this pass leaves the telling at.
///
/// lemonfiber's own value where it wrote or confirmed one, and the operator's where
/// it found one it never wrote — adopted rather than reported as drift from an
/// expectation that was never formed. Everything else leaves the baseline alone,
/// which is what keeps a preserved edit preserved on the next run too.
fn remember(
    records: &mut crate::baseline::Baseline,
    service: &str,
    state: &crate::seed::State,
    held: &crate::ports::service::Telling,
    at: &str,
) {
    match state {
        crate::seed::State::Wired | crate::seed::State::AlreadyWired => records.record(
            service,
            crate::seed::TELLING,
            &crate::seed::said(&crate::seed::wanted_telling()),
            at,
        ),
        crate::seed::State::Unmanaged => {
            records.adopt(service, crate::seed::TELLING, &crate::seed::said(held), at);
        }
        _ => {}
    }
}

/// Jellyfin's addresses, if the stack has it. Jellyfin's kind carries no key source of
/// the usual sort: it is the one service lemonfiber sets an account on rather than reading
/// a key from, so its password is generated.
pub(crate) fn jellyfin_service(
    services: &[lemonfiber_manifest::Service],
) -> Option<crate::app::targets::ServiceAddr> {
    crate::app::targets::service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)
}

/// The Jellyfin admin password recorded on the run that minted it, so a later run
/// can point Seerr at Jellyfin without minting again.
pub(crate) fn recorded_jellyfin_password(ctx: &Ctx) -> Option<String> {
    crate::app::targets::recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)
}

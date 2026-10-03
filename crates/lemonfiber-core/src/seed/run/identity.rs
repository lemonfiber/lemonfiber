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

/// What this connection asks the stack for: a service that answers, for the services
/// that ask, whether a person is who they say they are.
const IDENTITY: &str = "identity.source";

/// Jellyfin's administrator, as the first half of the identity left it: the password
/// to go on with, or the state the identity rests in without one.
pub(super) struct Admin(Result<String, crate::seed::State>);

/// The first half of making whatever fills the identity source the one Seerr signs in
/// against: the media server's administrator, minted where its wizard has not run.
///
/// Both must be in the stack; without either there is nothing to wire. The admin
/// password is the one credential minted rather than read — recorded on the run that
/// mints it and read back on a later run. Recorded here, before the second half, so the
/// steps between the two can sign in with it.
pub(super) async fn seed_jellyfin_admin(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    filled: &std::collections::BTreeMap<String, Vec<String>>,
) -> Option<Admin> {
    seerr_service(services)?;
    let jellyfin = identity_source(services, filled)?;
    let client =
        crate::jellyfin::Jellyfin::new(ctx.seams.http.clone(), &jellyfin.loopback, "jellyfin");
    let recorded = recorded_jellyfin_password(ctx);
    let administered = crate::seed::wire_jellyfin_admin(
        &client,
        ctx.seams.random.as_ref(),
        recorded.as_deref(),
        ctx.dry_run,
    )
    .await;
    // A rehearsal mints nothing, so there is nothing here to record — the condition is
    // already false. Written as a pair with the sign-in below rather than left to that
    // coincidence, because a value that arrived from anywhere else would be recorded by
    // a run that promised to write nothing.
    if let (Ok((_, Some(password))), false) = (&administered, ctx.dry_run) {
        record_jellyfin_password(ctx, password);
    }
    Some(Admin(administered.map(|(password, _)| password)))
}

/// The second half: Seerr signed in through Jellyfin — at the request gate's Jellyfin
/// route where the stack runs the gate, and at Jellyfin's own address where it does
/// not — then, through the gate, handed the token it reaches Jellyfin with after that.
pub(super) async fn seed_jellyfin_identity(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    expected: &crate::baseline::Baseline,
    filled: &std::collections::BTreeMap<String, Vec<String>>,
    admin: Option<Admin>,
    project: Option<&std::path::Path>,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let mut records = crate::baseline::Baseline::new();
    let (Some(seerr_base), Some(jellyfin), Some(Admin(administered))) = (
        seerr_service(services),
        identity_source(services, filled),
        admin,
    ) else {
        return (Vec::new(), records);
    };
    let gate = project.filter(|_| crate::app::gating::service(services).is_some());
    let server_url = match gate {
        Some(_) => {
            let at = super::tokens::through_the_gate(&jellyfin.id);
            format!("http://{}:{}{}", at.host, at.port, at.base)
        }
        None => jellyfin.network_url.clone(),
    };

    let wiring = match administered {
        Ok(password) => {
            let seerr_client =
                crate::seerr::Seerr::new(ctx.seams.http.clone(), &seerr_base, "seerr");
            crate::seed::wire_seerr_identity(&seerr_client, &password, &server_url, ctx.dry_run)
                .await
        }
        Err(state) => crate::seed::Wiring::settled(crate::seed::IDENTITY.to_owned(), state),
    };

    // What the household is told and where the request service reaches the media server
    // are read and written with the request service's own key, which the setup above
    // is what writes, so the client is opened only now.
    let owner = crate::app::targets::seerr_as_owner(ctx, services, seerr_base.clone()).await;

    let linked = match (gate, &wiring.state) {
        (Some(_), crate::seed::State::WouldWire { .. }) => {
            Some(super::linking::would_link(&jellyfin.id))
        }
        (Some(project), crate::seed::State::Wired | crate::seed::State::AlreadyWired) => {
            Some(super::linking::seed_media_server_link(ctx, &owner, &jellyfin.id, project).await)
        }
        _ => None,
    };

    // The run that set the request service up is the one that showed it the
    // administrator's password, so that password is changed straight after it and the
    // one the request service saw opens nothing.
    let changed = (wiring.state == crate::seed::State::Wired && !ctx.dry_run)
        .then(|| changed_after_setup(ctx, services));
    let changed = match changed {
        Some(changing) => Some(changing.await),
        None => None,
    };

    // What the household is told is its own managed field, reconciled whether or not
    // the identity above was wired this run: the identity step stops at a service
    // already initialised, and that is every install after the first.
    let (told, held) = crate::seed::wire_household_telling(
        &owner,
        expected.entry(SEERR, crate::seed::TELLING),
        ctx.dry_run,
    )
    .await;
    remember(&mut records, &told.state, held, &ctx.stamp());

    (
        [Some(wiring), linked, changed, Some(told)]
            .into_iter()
            .flatten()
            .collect(),
        records,
    )
}

/// What the report calls the change made after the request service was set up.
const CHANGED: &str =
    "Jellyfin's administrator password, changed once the request service was set up";

/// Change the administrator's password, and say how that went.
async fn changed_after_setup(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> crate::seed::Wiring {
    let failed =
        match crate::app::credentials::replace_jellyfin_password(ctx, services, false).await {
            Ok(_) => {
                return crate::seed::Wiring::settled(CHANGED.to_owned(), crate::seed::State::Wired)
            }
            Err(crate::app::credentials::Replacing::Refused) => {
                "Jellyfin refused the password lemonfiber holds".to_owned()
            }
            Err(crate::app::credentials::Replacing::Unproven(detail)) => detail,
        };
    let mut wiring = crate::seed::Wiring::settled(
        CHANGED.to_owned(),
        crate::seed::State::Failed { detail: failed },
    );
    wiring.escalate(
        "The password the request service saw at setup still opens Jellyfin.".to_owned(),
        "Run `lemonfiber credentials rotate jellyfin`.".to_owned(),
    );
    wiring
}

/// The service the household's telling is recorded under.
const SEERR: &str = "seerr";

/// Write down what this pass leaves the telling at.
///
/// lemonfiber's own value where it wrote or confirmed one, and the operator's where
/// it found one it never wrote — adopted rather than reported as drift from an
/// expectation that was never formed. Everything else leaves the baseline alone,
/// which is what keeps a preserved edit preserved on the next run too.
fn remember(
    records: &mut crate::baseline::Baseline,
    state: &crate::seed::State,
    held: crate::ports::service::Telling,
    at: &str,
) {
    match state {
        crate::seed::State::Wired | crate::seed::State::AlreadyWired => records.record(
            SEERR,
            crate::seed::TELLING,
            &crate::seed::said(crate::seed::wanted_telling()),
            at,
        ),
        crate::seed::State::Unmanaged => {
            records.adopt(SEERR, crate::seed::TELLING, &crate::seed::said(held), at);
        }
        _ => {}
    }
}

/// Where the host reaches Seerr's API, if the stack has it — resolved the way every
/// service lemonfiber speaks to is.
pub(super) fn seerr_service(services: &[lemonfiber_manifest::Service]) -> Option<String> {
    crate::app::targets::service_addr(services, lemonfiber_manifest::ApiKind::Seerr)
        .map(|addr| addr.loopback)
}

/// The addresses of whatever fills the identity source, if anything does.
///
/// The service is chosen by what it can do and then opened by what it is: the stack
/// says which service answers the ask, and the adapter that speaks to it comes from
/// that service's own declared shape. A filler this build has no adapter for is
/// nothing to wire rather than something to guess at — which is the same answer the
/// stack already gives for a service it declares no API for.
pub(crate) fn identity_source(
    services: &[lemonfiber_manifest::Service],
    filled: &std::collections::BTreeMap<String, Vec<String>>,
) -> Option<crate::app::targets::ServiceAddr> {
    let [fills_it] = filled.get(IDENTITY)?.as_slice() else {
        return None;
    };
    let kind = services
        .iter()
        .find(|service| &service.id == fills_it)?
        .api
        .as_ref()?
        .kind;
    crate::app::targets::service_addr(services, kind).filter(|addr| &addr.id == fills_it)
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

/// Record the minted Jellyfin admin password where a later run reads it back.
/// Best-effort: a value that could not be written is reported by the next run
/// re-minting rather than by failing the wiring that did land.
pub(super) fn record_jellyfin_password(ctx: &Ctx, password: &str) {
    crate::app::targets::record_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY, password);
}

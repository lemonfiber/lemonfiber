//! Making the media server the household's single sign-in.
//!
//! One account, not two: the request service authenticates against the media server
//! rather than keeping accounts of its own.

use super::Ctx;

/// Make Jellyfin the identity source for Seerr, so the household signs in once.
///
/// Both must be in the stack; without either there is nothing to wire. Jellyfin's
/// admin password is the one credential minted rather than read — recorded on the
/// run that mints it and read back on a later run — so the driver is given what
/// was recorded and hands back a freshly minted one for the surface to record.
pub(super) async fn seed_jellyfin_identity(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    expected: &crate::baseline::Baseline,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let mut records = crate::baseline::Baseline::new();
    let (Some(seerr_base), Some(jellyfin)) = (seerr_service(services), jellyfin_service(services))
    else {
        return (Vec::new(), records);
    };

    let jellyfin_client =
        crate::jellyfin::Jellyfin::new(ctx.http.clone(), &jellyfin.loopback, "jellyfin");
    let seerr_client = crate::seerr::Seerr::new(ctx.http.clone(), &seerr_base, "seerr");
    let recorded = recorded_jellyfin_password(ctx);

    let (wiring, minted) = crate::seed::wire_jellyfin_identity(
        &jellyfin_client,
        &seerr_client,
        ctx.random.as_ref(),
        recorded.as_deref(),
        &jellyfin.network_url,
    )
    .await;

    if let Some(password) = &minted {
        record_jellyfin_password(ctx, password);
    }

    // What the household is told is its own managed field, reconciled whether or not
    // the identity above was wired this run: the identity step stops at a service
    // already initialised, and that is every install after the first.
    let (told, held) = crate::seed::wire_household_telling(
        &seerr_client,
        expected.entry(SEERR, crate::seed::TELLING),
    )
    .await;
    remember(&mut records, &told.state, held, &ctx.stamp());

    (vec![wiring, told], records)
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

/// Jellyfin's addresses, if the stack has it. Jellyfin's kind carries no key source of
/// the usual sort: it is the one service lemonfiber sets an account on rather than reading
/// a key from, so its password is generated.
pub(super) fn jellyfin_service(
    services: &[lemonfiber_manifest::Service],
) -> Option<crate::app::targets::ServiceAddr> {
    crate::app::targets::service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)
}

/// The Jellyfin admin password recorded on the run that minted it, so a later run
/// can point Seerr at Jellyfin without minting again.
pub(super) fn recorded_jellyfin_password(ctx: &Ctx) -> Option<String> {
    crate::app::targets::recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)
}

/// Record the minted Jellyfin admin password where a later run reads it back.
/// Best-effort: a value that could not be written is reported by the next run
/// re-minting rather than by failing the wiring that did land.
pub(super) fn record_jellyfin_password(ctx: &Ctx, password: &str) {
    crate::app::targets::record_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY, password);
}

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
) -> Vec<crate::seed::Wiring> {
    let (Some(seerr_base), Some(jellyfin)) = (seerr_service(services), jellyfin_service(services))
    else {
        return Vec::new();
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
    vec![wiring]
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

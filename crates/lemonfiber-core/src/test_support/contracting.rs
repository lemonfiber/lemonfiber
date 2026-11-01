//! A context reaching the services of contracted plugins, and the guards standing in for
//! the stack's own over their contracts.

use super::{a_context, context, guards, media, Reporting, CONTRACTED_KEY, CONTRACTED_PORT};

/// A context reaching `service` on loopback [`CONTRACTED_PORT`], holding its key in
/// `project` where `keyed`, beside the media server's adapter running where a moved server
/// is reached.
pub(crate) fn contracted_context(
    project: &std::path::Path,
    service: &str,
    keyed: bool,
) -> context::Context {
    if keyed {
        contracted_key(project, service);
    }
    a_context().engine(std::sync::Arc::new(
        contracted_engine(&[service]).alongside(media::running()),
    ))
}

/// `service`'s key written beside its configuration in `project`, as its plugin holds it.
fn contracted_key(project: &std::path::Path, service: &str) {
    let at = crate::plugin::key_file(project, service);
    let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
    let _ = std::fs::write(&at, CONTRACTED_KEY);
}

/// `ctx` reaching `gate` as [`super::a_contracted_gate`] does: its key written in
/// `project`, the engine saying it runs beside whatever else runs, and every
/// `request.gate` call answered from it.
pub(crate) fn gated(
    ctx: crate::app::Ctx,
    project: &std::path::Path,
    gate: &std::sync::Arc<guards::Gate>,
) -> crate::app::Ctx {
    let ctx = contracted_beside(ctx, project, crate::app::gating::SERVICE);
    let http = std::sync::Arc::clone(&ctx.seams.http);
    ctx.with_http(std::sync::Arc::new(
        guards::Serving::beside(http).gate(gate),
    ))
}

/// `ctx` reaching `decline` as [`super::a_contracted_decline`] does: its key written in
/// `project`, the engine saying it runs beside whatever else runs, and every
/// `invite.decline` call answered from it.
pub(crate) fn declined(
    ctx: crate::app::Ctx,
    project: &std::path::Path,
    decline: &std::sync::Arc<guards::Decline>,
) -> crate::app::Ctx {
    let ctx = contracted_beside(ctx, project, crate::app::invite::declining::SERVICE);
    let http = std::sync::Arc::clone(&ctx.seams.http);
    ctx.with_http(std::sync::Arc::new(
        guards::Serving::beside(http).decline(decline),
    ))
}

/// `ctx` with `service` keyed in `project` and running on loopback [`CONTRACTED_PORT`]
/// beside whatever else its engine says runs.
fn contracted_beside(
    mut ctx: crate::app::Ctx,
    project: &std::path::Path,
    service: &str,
) -> crate::app::Ctx {
    contracted_key(project, service);
    ctx.seams.engine = std::sync::Arc::new(guards::Alongside::new(
        std::sync::Arc::clone(&ctx.seams.engine),
        contracted_engine(&[service]),
    ));
    ctx
}

/// The engine's account of each of `services` running healthy, published on loopback
/// [`CONTRACTED_PORT`].
fn contracted_engine(services: &[&str]) -> Reporting {
    let published: Vec<(&str, &str, u16)> = services
        .iter()
        .map(|service| (*service, "127.0.0.1", CONTRACTED_PORT))
        .collect();
    Reporting::holding(
        services,
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    )
    .publishing(&published)
}

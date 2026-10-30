//! The household's proxy, applied the routing table: a route for every household service,
//! the stack's own and every installed plugin's, at its label in front of the operator's
//! domain, and none to the proxy itself.
//!
//! What is applied replaces what the proxy serves, so the table is the whole of it. The
//! proxy keeps how it holds TLS: that is the proxy's to say, and the table carries back
//! what it answered.

use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_manifest::Service;

use super::restarting::Restarting;
use super::{withheld, withheld_brought, Ctx};
use crate::app::targets::{chosen_fillers, project_directory, spoken, Spoken};
use crate::plugin::Installed;
use crate::ports::service::{Proxying, Route, Routing};
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection.
pub(super) const CONNECTION: &str = "The proxy's routes";

/// Hold the filler of `proxy.front` to a route for every household service of the stack's
/// `services` and the `installed` plugins, where it speaks the contract.
pub(super) async fn seed_routes(
    ctx: &Ctx,
    services: &[Service],
    installed: &[Installed],
    fillers: &Fillers,
) -> Option<Wiring> {
    let (proxy, itself) = asked(ctx, fillers).await?;
    let domain = crate::settings::household_domain(ctx);
    let routes = crate::plugin::stack_routes(services, installed, &domain)
        .into_iter()
        .chain(crate::plugin::household_routes(installed, &domain))
        .filter(|route| route.host != itself)
        .collect();
    Some(settled(routed(ctx, &proxy, routes).await))
}

/// Apply `routes` to the proxy, keeping the TLS it holds, where it serves anything else.
async fn routed(ctx: &Ctx, proxy: &Restarting, routes: Vec<Route>) -> State {
    let fronting = front::Adapter(proxy.adapter().clone());
    let served = match fronting.routing().await {
        Ok(served) => served,
        Err(failure) => return crate::seed::unreached(&failure),
    };
    let wanted = Routing {
        tls: served.tls.clone(),
        routes,
    };
    if served == wanted {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        return proxy.would(Some(said(&served)), &said(&wanted));
    }
    proxy.taken_up(ctx, fronting.apply(&wanted).await).await
}

/// The filler of `proxy.front`, where it speaks the contract, and the id of the proxy
/// itself: the upstream it fronts, or the filler where it fronts none.
async fn asked(ctx: &Ctx, fillers: &Fillers) -> Option<(Restarting, String)> {
    let (asker, _) = fillers.filling(front::CAPABILITY)?;
    match spoken(ctx, asker, front::CAPABILITY, front::MAJOR).await {
        Spoken::Over(adapter) => Some((
            Restarting::of(fillers, asker, adapter),
            fillers.fronted_by(asker).unwrap_or(asker).id.clone(),
        )),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// The stack's services and what is installed here, as a seed pass reads them, and who
/// fills each ask: nothing where the stack or the record of what is installed cannot be
/// read.
fn here(ctx: &Ctx) -> Option<(Vec<Service>, Vec<Installed>, Fillers)> {
    let mut manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let _ = withheld(&mut manifest.services, &ctx.settings.unmanaged);
    let register = crate::app::plugins::read(ctx).ok()?;
    let (installed, _) = withheld_brought(register.installed(), &ctx.settings.unmanaged);
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let fillers = Fillers::of(
        &manifest,
        &installed,
        &chosen_fillers(ctx),
        project.as_deref(),
    )
    .numbered(crate::app::targets::ordinals(ctx));
    Some((manifest.services, installed, fillers))
}

/// Apply the proxy the routing table again, as installing, updating or removing a plugin
/// or changing the domain owes it, and say how that came out: nothing where the stack or
/// the record of what is installed cannot be read, or nothing speaks `proxy.front`.
pub(crate) async fn refront(ctx: &Ctx) -> Option<State> {
    let (services, installed, fillers) = here(ctx)?;
    seed_routes(ctx, &services, &installed, &fillers)
        .await
        .map(|wiring| wiring.state)
}

/// Every route the proxy serves now: none where nothing speaks `proxy.front` or it does
/// not answer.
pub(crate) async fn served(ctx: &Ctx) -> Vec<Route> {
    let Some((_, _, fillers)) = here(ctx) else {
        return Vec::new();
    };
    let Some((proxy, _)) = asked(ctx, &fillers).await else {
        return Vec::new();
    };
    front::Adapter(proxy.adapter().clone())
        .routing()
        .await
        .map(|served| served.routes)
        .unwrap_or_default()
}

/// A routing table by the names it answers.
fn said(routing: &Routing) -> String {
    if routing.routes.is_empty() {
        return "no names".to_owned();
    }
    routing
        .routes
        .iter()
        .map(|route| route.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}

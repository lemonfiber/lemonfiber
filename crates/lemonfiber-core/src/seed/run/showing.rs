//! The household's dashboard, applied the panel list: an entry for every service with a
//! port, and the page's widgets.

use lemonfiber_ports::service::Panels;

use super::Ctx;
use crate::app::showing;
use crate::plugin::Installed;
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection.
pub(super) const CONNECTION: &str = "The dashboard's panels";

/// Hold the dashboard to the panel list for the stack's `services` and those of the
/// `installed` plugins.
pub(super) async fn seed_dashboard(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[Installed],
    fillers: &Fillers,
) -> Option<Wiring> {
    let dashboard = showing::asked(ctx, fillers).await?;
    let search = match crate::settings::dashboard_search(ctx) {
        Ok(search) => search,
        Err(detail) => return Some(settled(State::Failed { detail })),
    };
    let named = ctx.site.name().await;
    let wanted = showing::panels::panels(
        services,
        installed,
        fillers,
        &crate::door::run::place(ctx, named.as_deref()),
        &search,
    );
    let shown = match dashboard.panels().await {
        Ok(shown) => shown,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    let state = if shown == wanted {
        State::AlreadyWired
    } else if ctx.dry_run {
        State::WouldWire {
            yours: Some(said(&shown)),
            ours: Some(said(&wanted)),
        }
    } else {
        match dashboard.apply(&wanted).await {
            Ok(()) => State::Wired,
            Err(failure) => State::Failed {
                detail: format!("the panel list could not be applied to the dashboard: {failure}"),
            },
        }
    };
    Some(settled(state))
}

/// Apply the dashboard the panel list again, as a change to what it is made from owes it,
/// as a seed pass does it, and say how that came out: nothing where the stack or the
/// record of what is installed cannot be read, or no dashboard is there to apply.
pub(crate) async fn reshow(ctx: &Ctx) -> Option<State> {
    let (services, installed, fillers) = super::managed(ctx)?;
    seed_dashboard(ctx, &services, &installed, &fillers)
        .await
        .map(|wiring| wiring.state)
}

/// A panel list by how much it shows.
fn said(panels: &Panels) -> String {
    let entries: usize = panels.groups.iter().map(|group| group.entries.len()).sum();
    format!(
        "{entries} entries in {} groups and {} page widgets",
        panels.groups.len(),
        panels.widgets.len()
    )
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}

//! The household's dashboard, applied the panel list: an entry for every service with a
//! port, and the page's widgets.

use std::path::Path;

use lemonfiber_ports::service::Panels;

use super::Ctx;
use crate::app::showing;
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection.
const CONNECTION: &str = "The dashboard's panels";

/// Hold the dashboard to the panel list for the stack's `services` and every installed
/// plugin's.
pub(super) async fn seed_dashboard(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &Fillers,
    project: Option<&Path>,
) -> Option<Wiring> {
    let project = project?;
    let dashboard = match showing::asked(ctx, fillers, project).await? {
        showing::Dashboard::Asked(dashboard) => dashboard,
        showing::Dashboard::Unmanaged => {
            return Some(settled(State::Skipped {
                reason: "the operator declared the dashboard's files unmanaged, so lemonfiber \
                         writes nothing there"
                    .to_owned(),
            }))
        }
    };
    let search = match crate::settings::dashboard_search(ctx) {
        Ok(search) => search,
        Err(detail) => return Some(settled(State::Failed { detail })),
    };
    let register =
        crate::app::plugins::read(ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    let named = ctx.site.name().await;
    let wanted = showing::panels::panels(
        services,
        register.installed(),
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

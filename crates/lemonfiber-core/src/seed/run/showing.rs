//! The household's dashboard, applied the panel list: an entry for every service with a
//! port, and the page's widgets.

use std::path::Path;

use lemonfiber_ports::service::Panels;

use super::{withheld, withheld_brought, Ctx};
use crate::app::showing;
use crate::app::targets::{chosen_fillers, project_directory};
use crate::plugin::Installed;
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection, which writes the dashboard's files and reaches
/// no service to do it.
pub(super) const CONNECTION: &str = "The dashboard's panels";

/// Hold the dashboard to the panel list for the stack's `services` and those of the
/// `installed` plugins.
pub(super) async fn seed_dashboard(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[Installed],
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
    );
    seed_dashboard(
        ctx,
        &manifest.services,
        &installed,
        &fillers,
        project.as_deref(),
    )
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

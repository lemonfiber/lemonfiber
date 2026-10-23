//! The panel list the core applies to the dashboard: an entry for every service with a
//! port, under its group, and the page's widgets.

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::{Bind, HealthKind, Service};
use lemonfiber_ports::service::{Entry, Group, PageWidget, Panels};

use crate::door::Place;
use crate::plugin::{Installed, Placed, Reached};
use crate::wiring::{Filler, Fillers};

/// The group a household service is listed under where its wiring named none.
const HOUSEHOLD_GROUP: &str = "Library";

/// The group an operator surface is listed under where its wiring named none.
const OPERATOR_GROUP: &str = "Automation";

/// One service as its entry is made, whichever declares it.
struct Listing<'a> {
    /// The id it runs under.
    id: &'a str,
    /// What it is called.
    name: &'a str,
    /// The group its wiring named.
    group: Option<&'a str>,
    /// The line beside it.
    description: &'a str,
    /// The icon it shows.
    icon: String,
    /// Whether its wiring hid it.
    hidden: bool,
    /// The port this machine publishes it on.
    port: Option<u16>,
    /// Whether the household reaches it.
    household: bool,
    /// The path it answers over HTTP to say it is up.
    health: Option<&'a str>,
}

impl<'a> Listing<'a> {
    /// One of the stack's own services.
    fn stack(service: &'a Service) -> Self {
        let dashboard = &service.dashboard;
        Self {
            id: &service.id,
            name: &service.name,
            group: dashboard.group.as_deref(),
            description: dashboard
                .description
                .as_deref()
                .unwrap_or(&service.describes),
            icon: dashboard
                .icon
                .clone()
                .unwrap_or_else(|| format!("{}.png", service.id)),
            hidden: dashboard.hidden,
            port: service.port,
            household: service.bind == Some(Bind::Lan),
            health: service
                .health
                .as_ref()
                .filter(|health| health.kind == HealthKind::Http)
                .and_then(|health| health.path.as_deref()),
        }
    }

    /// One of an installed plugin's services.
    fn placed(placed: &'a Placed) -> Self {
        Self {
            id: &placed.service,
            name: placed.called(),
            group: placed.reached.as_ref().and_then(Reached::group),
            description: &placed.description,
            icon: placed.icon(),
            hidden: placed.hidden,
            port: placed.published(),
            household: matches!(placed.reached, Some(Reached::Household { .. })),
            health: placed.health.as_deref(),
        }
    }
}

/// The panel list for the stack's `services` and every `installed` plugin's, linked from
/// `place`, with the search box sending to `search`.
///
/// The stack's services first, in the order it declares them, then each plugin's; a stack
/// service an installed plugin's service has taken the place of is the plugin's. Groups
/// come in the order they first appear.
pub(crate) fn panels(
    services: &[Service],
    installed: &[Installed],
    fillers: &Fillers,
    place: &Place<'_>,
    search: &str,
) -> Panels {
    let placed: Vec<&Placed> = installed.iter().flat_map(|one| &one.services).collect();
    let listings: Vec<Listing<'_>> = services
        .iter()
        .filter(|service| !placed.iter().any(|one| one.service == service.id))
        .map(Listing::stack)
        .chain(placed.iter().map(|one| Listing::placed(one)))
        .collect();
    let guarded = guarded(fillers, &listings);
    let mut groups: Vec<Group> = Vec::new();
    for listing in &listings {
        if listing.hidden || guarded.is_some_and(|(_, guard)| guard.id == listing.id) {
            continue;
        }
        let reached = match guarded {
            Some((media, guard)) if media == listing.id => guard,
            _ => listing,
        };
        let Some(port) = reached.port else {
            continue;
        };
        let entry = Entry {
            name: listing.name.to_owned(),
            icon: listing.icon.clone(),
            link: link(place, port, reached.household),
            description: listing.description.to_owned(),
            check: check(fillers.service(listing.id), listing.health),
            widget: None,
        };
        let group = listing.group.unwrap_or(if reached.household {
            HOUSEHOLD_GROUP
        } else {
            OPERATOR_GROUP
        });
        match groups.iter_mut().find(|one| one.name == group) {
            Some(one) => one.entries.push(entry),
            None => groups.push(Group {
                name: group.to_owned(),
                entries: vec![entry],
            }),
        }
    }
    Panels {
        groups,
        widgets: vec![
            PageWidget::Resources,
            PageWidget::Search {
                url: search.to_owned(),
            },
        ],
    }
}

/// The media server's id and the listing of the guard the household reaches it through,
/// where a guard runs in front of it.
fn guarded<'b, 'a>(
    fillers: &Fillers,
    listings: &'b [Listing<'a>],
) -> Option<(&'b str, &'b Listing<'a>)> {
    let (served, _) = fillers.filling(serve::CAPABILITY)?;
    let media = fillers.fronted_by(served).unwrap_or(served);
    let (guarding, _) = fillers.filling(guard::CAPABILITY)?;
    let media = listings.iter().find(|one| one.id == media.id)?;
    let guard = listings.iter().find(|one| one.id == guarding.id)?;
    Some((media.id, guard))
}

/// Where a browser goes for a service on `port`: the address the household is handed for
/// a household service, and this machine for an operator surface.
fn link(place: &Place<'_>, port: u16, household: bool) -> String {
    household
        .then(|| crate::door::address(place.named, place.recorded, place.environment, port))
        .flatten()
        .map_or_else(
            || format!("http://{}:{port}", crate::door::ONLY_HERE),
            |address| address.url,
        )
}

/// Where the dashboard asks whether a service is up: its health path, at the address the
/// stack's network reaches it on.
fn check(filler: Option<&Filler>, health: Option<&str>) -> Option<String> {
    let address = filler?.address.as_ref()?;
    health.map(|path| format!("{}{path}", address.url()))
}

#[cfg(test)]
mod tests;

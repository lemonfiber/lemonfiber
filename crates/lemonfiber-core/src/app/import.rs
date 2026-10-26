//! Copying an operator's own records out of the setup already here into lemonfiber's.
//!
//! The mode that leaves both stacks standing. lemonfiber runs its own services and takes
//! across what the operator built by hand: where releases are searched for, and the
//! three libraries the curators follow.
//!
//! Nothing of theirs is written to, stopped, or removed — theirs is only ever read. What
//! is written is written to lemonfiber's own services, and only where they do not
//! already hold a record of that name: an import that overwrote what the operator has
//! since changed on the new stack would be undoing their work in the name of copying it.
//!
//! **Both sides are asked over `library.curate`.** Ours through the adapter in front of
//! it. Theirs has none, so the adapter of the first-party plugin lemonfiber runs ours
//! from is lent to it: started as a throwaway container handed their key alone, asked
//! for their records, and taken away whatever the reading came to. A plugin that is not
//! first-party is never lent their key: their copy is not read through it.
//!
//! Unconfirmed it names what it would carry and carries nothing.

use lemonfiber_contract::adapter::{UPSTREAM_PASSWORD, UPSTREAM_USERNAME};
use lemonfiber_contract::capabilities::library::curate;

use super::targets::{fillers_here, target_named};
use crate::error::Problem;
use crate::migration::importing::{carryable, missing, unmatched};
use crate::model::{ImportReport, MigrationReport, RecordReport, UnsupportedReport};
use crate::plugin::first_party::FirstParty;
use crate::plugin::lending::{lent, Borrowing};
use crate::ports::docker::Container;
use crate::ports::service::{Carried, Record};
use crate::reconfigure::Stance;
use crate::wiring::Fillers;

use super::Ctx;

/// The three services that keep a library of their own, and what each keeps.
const LIBRARIES: [(&str, &[Record]); 3] = [
    ("sonarr", &[Record::Indexer, Record::Series]),
    ("radarr", &[Record::Indexer, Record::Film]),
    ("lidarr", &[Record::Indexer, Record::Artist]),
];

/// What a service holds worth carrying.
///
/// Every service of this shape has indexers, which is why that is the answer for one
/// this does not otherwise know: an aggregator keeps nothing else, and a service added
/// to the stack later would still have somewhere its releases are searched for.
fn kinds_for(service: &str) -> &'static [Record] {
    LIBRARIES
        .iter()
        .find(|(id, _)| *id == service)
        .map_or(&[Record::Indexer], |(_, kinds)| *kinds)
}

/// Carry what the setup already here holds into lemonfiber's own services, lending their
/// copies only the adapters of plugins `trusted` holds as first-party.
///
/// # Errors
///
/// Never in practice: a service that cannot be reached on either side is reported as a
/// record that could not be carried, rather than refusing the whole import.
pub async fn carry(
    ctx: &Ctx,
    survey: &MigrationReport,
    running: &[Container],
    confirmed: bool,
    trusted: &[FirstParty],
) -> Result<ImportReport, Box<Problem>> {
    let Some(project) = crate::migration::one_setup(survey) else {
        return Ok(nothing(survey));
    };

    // Read again rather than carried through: the survey has already refused a stack
    // it could not read, so what comes back here is the same manifest it read. A
    // failure now leaves no services to look up, which carries nothing and says so.
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Ok(nothing(survey));
    };
    let fillers = fillers_here(ctx, &manifest);

    let mut found = Found::default();

    for container in running
        .iter()
        .filter(|container| container.project == project)
    {
        let Some(ours) = target_named(ctx, &fillers, &container.service).await else {
            continue;
        };
        let (service, kinds) = (ours.id.as_str(), kinds_for(&ours.id));
        let Some(to) = ours.asked.as_ref() else {
            found.refused.push(unreachable(
                service,
                "lemonfiber's own copy could not be asked over its contract",
            ));
            continue;
        };
        let borrowing = match borrowed(ctx, &fillers, container, trusted).await {
            Ok(borrowing) => borrowing,
            Err(why) => {
                found.refused.push(unreachable(service, &why));
                continue;
            }
        };
        let theirs = match lent(ctx, &borrowing.as_asked()).await {
            Ok(theirs) => theirs,
            Err(why) => {
                found.refused.push(unreachable(
                    service,
                    &format!("the adapter lent to their copy did not start: {why}"),
                ));
                continue;
            }
        };
        let from = curate::Adapter(theirs.adapter.clone());
        gather((&from, to.as_ref()), service, kinds, confirmed, &mut found).await;
        theirs.returned(ctx).await;
    }

    // Read before the report takes the lists apart.
    let stance = stood(confirmed, &found);
    Ok(ImportReport {
        project: Some(project),
        would_carry: named(&found.going),
        carried: found.carried,
        not_carried: found.refused,
        stance,
        ..ImportReport::default()
    })
}

/// What one pass over both stacks turned up.
///
/// One accumulator rather than three out-parameters: the three are filled together and
/// read together, and a function taking them separately had one more argument than a
/// reader can hold.
#[derive(Default)]
struct Found {
    /// Every record on its way across.
    going: Vec<Going>,
    /// Every record that went.
    carried: Vec<RecordReport>,
    /// Everything that did not, and why.
    refused: Vec<UnsupportedReport>,
}

/// One record on its way across, with where it came from and where it goes.
struct Going {
    /// The service it belongs to, by id.
    service: String,
    /// What kind of record it is.
    kind: Record,
    /// The record itself.
    item: Carried,
}

/// What each side holds, and what of it would travel: from their copy to ours, filed
/// under `service`.
async fn gather(
    pair: (&dyn curate::Fills, &dyn curate::Fills),
    service: &str,
    kinds: &[Record],
    confirmed: bool,
    found: &mut Found,
) {
    let (from, to) = pair;
    let Ok(held) = to.quality_profiles().await else {
        found.refused.push(unreachable(
            service,
            "its quality profiles could not be read",
        ));
        return;
    };
    let profiles: Vec<String> = held.into_iter().map(|profile| profile.name).collect();

    for kind in kinds {
        let (Ok(held), Ok(already)) = (from.records(*kind).await, to.records(*kind).await) else {
            found.refused.push(unreachable(
                service,
                &format!("its {} could not be read", kind.plural()),
            ));
            continue;
        };

        let wanted = missing(&held, &already);
        found.refused.extend(unmatched(&wanted, &profiles));
        for item in carryable(&wanted, &profiles) {
            let record = RecordReport {
                service: service.to_owned(),
                kind: kind.plural().to_owned(),
                name: item.name.clone(),
            };
            found.going.push(Going {
                service: service.to_owned(),
                kind: *kind,
                item: item.clone(),
            });
            if !confirmed {
                continue;
            }
            match to.carry(*kind, &item).await {
                Ok(()) => found.carried.push(record),
                Err(failure) => found.refused.push(UnsupportedReport {
                    what: item.name.clone(),
                    because: format!("{service} would not take it: {failure}"),
                }),
            }
        }
    }
}

/// The name the adapter lent to their copy reaches this machine by.
const THEIRS: &str = "theirs";

/// What an adapter is lent to their copy of a service with, worked out before anything
/// is started.
struct Borrowed {
    /// The plugin lemonfiber runs its own copy from.
    installed: crate::plugin::Installed,
    /// Its adapter in front of that copy.
    adapter: crate::plugin::Placed,
    /// Where the adapter reaches their copy.
    upstream: String,
    /// Each field of the adapter's credential, with their copy's key.
    credentials: Vec<(String, String)>,
}

impl Borrowed {
    /// The borrowing it comes to.
    fn as_asked(&self) -> Borrowing<'_> {
        Borrowing {
            installed: &self.installed,
            adapter: &self.adapter,
            upstream: self.upstream.clone(),
            reaching: vec![THEIRS.to_owned()],
            credentials: self.credentials.clone(),
        }
    }
}
/// Why their copy is not read through the adapter of a plugin that is not first-party.
const NOT_FIRST_PARTY: &str =
    "its plugin is not first-party, so their copy's key is not handed to it";

/// Why no adapter is lent where the record of what is installed holds no plugin that runs
/// lemonfiber's own copy with an adapter in front of it.
const UNRECORDED: &str = "the record of what is installed holds no plugin that runs \
                          lemonfiber's own copy with an adapter in front of it, so there is \
                          no adapter to lend their copy";

/// Why no adapter is lent where it signs in with an account rather than a key.
const ACCOUNTED: &str =
    "its adapter signs in with an account, and lemonfiber holds none for their copy";

/// Why no key is read where their copy names no configuration to read it from.
const UNCONFIGURED: &str = "no configuration to read a key from";

/// What the adapter in front of lemonfiber's own copy of `container`'s service is lent to
/// their copy with: the plugin, where `trusted` holds it as first-party, and the adapter,
/// their key read from where the plugin says the service keeps it beneath the directory
/// their container mounts, and where this machine reaches their copy; or why it cannot
/// be lent.
async fn borrowed(
    ctx: &Ctx,
    fillers: &Fillers,
    container: &Container,
    trusted: &[FirstParty],
) -> Result<Borrowed, String> {
    let service = container.service.as_str();
    let plugin = fillers
        .service(service)
        .and_then(crate::wiring::Filler::brought_by);
    let (installed, adapter) = crate::app::plugins::read(ctx)
        .ok()
        .and_then(|register| {
            register
                .installed()
                .iter()
                .filter(|one| Some(one.plugin.as_str()) == plugin)
                .find_map(|one| {
                    let adapter = one
                        .services
                        .iter()
                        .find(|placed| placed.fronts.as_deref() == Some(service))?;
                    Some((one.clone(), adapter.clone()))
                })
        })
        .ok_or_else(|| UNRECORDED.to_owned())?;
    if !crate::plugin::first_party::holds(trusted, &installed) {
        return Err(NOT_FIRST_PARTY.to_owned());
    }
    let port = container
        .published
        .iter()
        .find(|published| !published.address.is_loopback())
        .map(|published| published.port)
        .ok_or_else(|| {
            "it answers on no address but this machine's loopback, which a container cannot \
             reach"
                .to_owned()
        })?;
    if adapter
        .upstream
        .keys()
        .any(|field| field == UPSTREAM_USERNAME || field == UPSTREAM_PASSWORD)
    {
        return Err(ACCOUNTED.to_owned());
    }
    let mut credentials = Vec::new();
    for (field, secret) in &adapter.upstream {
        let key = their_key(ctx, &installed, secret, container).await?;
        credentials.push((field.clone(), key));
    }
    Ok(Borrowed {
        upstream: format!("http://{THEIRS}:{port}"),
        installed,
        adapter,
        credentials,
    })
}

/// The key their copy of `container`'s service wrote for itself, read from where the
/// plugin `installed`'s secret `secret` says the service keeps it, beneath the first
/// directory their container mounts.
async fn their_key(
    ctx: &Ctx,
    installed: &crate::plugin::Installed,
    secret: &str,
    container: &Container,
) -> Result<String, String> {
    let manifest = crate::app::plugins::kept::manifest(ctx, installed)?;
    let from = manifest
        .secrets
        .iter()
        .find(|declared| declared.id == secret)
        .and_then(|declared| declared.carried_from.as_ref());
    let mount = manifest
        .services
        .iter()
        .find(|service| service.id == container.service)
        .map(lemonfiber_plugin::Service::configuration);
    let read = from
        .zip(mount)
        .zip(container.mounts.first())
        .and_then(|((from, mount), theirs)| {
            Some((
                from,
                theirs,
                crate::app::targets::config_file(theirs, mount, &from.file)?,
            ))
        });
    let Some((from, theirs, path)) = read else {
        return Err(UNCONFIGURED.to_owned());
    };
    crate::app::targets::read_owned(ctx.seams.filesystem.as_ref(), &path, theirs)
        .await
        .and_then(|text| crate::credential::written::at(from.format, &from.key, &text))
        .ok_or_else(|| "their copy has written no key lemonfiber could read".to_owned())
}

/// What is said about a service that could not be read on one side or the other.
fn unreachable(service: &str, why: &str) -> UnsupportedReport {
    UnsupportedReport {
        what: service.to_owned(),
        because: format!("nothing of it was carried because {why}"),
    }
}

/// Every record on its way, as a report reads it.
fn named(going: &[Going]) -> Vec<RecordReport> {
    going
        .iter()
        .map(|one| RecordReport {
            service: one.service.clone(),
            kind: one.kind.plural().to_owned(),
            name: one.item.name.clone(),
        })
        .collect()
}

/// Where an import stands once it has been through both stacks.
///
/// Two stacks that already hold the same records is `Unchanged` rather than an applied
/// import that carried nothing: an operator reading "carried nothing" wants to know
/// whether that is because there was nothing to carry or because something went wrong.
const fn stood(confirmed: bool, found: &Found) -> Stance {
    if !confirmed {
        return Stance::Pending;
    }
    if found.carried.is_empty() && found.refused.is_empty() {
        return Stance::Unchanged;
    }
    Stance::Applied
}

/// What is answered where there is no one setup to carry anything out of.
fn nothing(survey: &MigrationReport) -> ImportReport {
    let why = if survey.read {
        "there is no single setup here holding services lemonfiber runs, so there is \
         nothing to carry across"
    } else {
        "what is on this machine could not be read, so there is nothing to carry across"
    };
    ImportReport {
        stance: Stance::Blocked,
        refusal: Some(why.to_owned()),
        ..ImportReport::default()
    }
}

#[cfg(test)]
mod tests;

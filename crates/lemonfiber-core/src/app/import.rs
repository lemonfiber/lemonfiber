//! Copying an operator's own records out of the setup already here into lemonfiber's.
//!
//! The mode that leaves both stacks standing. lemonfiber runs its own services and takes
//! across what the operator built by hand: where releases are searched for, and the
//! three libraries the *arrs follow.
//!
//! Nothing of theirs is written to, stopped, or removed — theirs is only ever read. What
//! is written is written to lemonfiber's own services, and only where they do not
//! already hold a record of that name: an import that overwrote what the operator has
//! since changed on the new stack would be undoing their work in the name of copying it.
//!
//! Unconfirmed it names what it would carry and carries nothing.

use super::targets::{project_directory, target_named};
use crate::doctor::credentials::Target;
use crate::error::Problem;
use crate::migration::importing::{carryable, missing, unmatched};
use crate::model::{ImportReport, MigrationReport, RecordReport, UnsupportedReport};
use crate::ports::docker::Container;
use crate::ports::service::{Carried, Carrying, Record};

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

/// Carry what the setup already here holds into lemonfiber's own services.
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
) -> Result<ImportReport, Box<Problem>> {
    let Some(project) = crate::migration::one_setup(survey) else {
        return Ok(nothing(survey));
    };

    // Read again rather than carried through: the survey has already refused a stack
    // it could not read, so what comes back here is the same manifest it read. A
    // failure now leaves no services to look up, which carries nothing and says so.
    let services = ctx
        .stack
        .checked_manifest(ctx.today())
        .map(|manifest| manifest.services)
        .unwrap_or_default();

    let mut found = Found::default();

    for container in running
        .iter()
        .filter(|container| container.project == project)
    {
        let Some((kinds, ours)) = holds(&services, ctx, &container.service) else {
            continue;
        };
        let Some(theirs) = theirs(container, &ours) else {
            found.refused.push(unreachable(
                &container.service,
                "no configuration to read a key from",
            ));
            continue;
        };

        gather(ctx, (&theirs, &ours), kinds, confirmed, &mut found).await;
    }

    Ok(ImportReport {
        project: Some(project),
        would_carry: named(&found.going),
        carried: found.carried,
        not_carried: found.refused,
        applied: confirmed,
        rehearsed: !confirmed,
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

/// What each side holds, and what of it would travel.
async fn gather(
    ctx: &Ctx,
    pair: (&Target, &Target),
    kinds: &[Record],
    confirmed: bool,
    found: &mut Found,
) {
    let (theirs, ours) = pair;
    let (Some(from), Some(to)) = (
        theirs.open(&ctx.http, ctx.filesystem.as_ref()).await,
        ours.open(&ctx.http, ctx.filesystem.as_ref()).await,
    ) else {
        found.refused.push(unreachable(
            &ours.id,
            "one of the two copies could not be reached",
        ));
        return;
    };

    let Ok(held) = crate::ports::service::Client::quality_profiles(&to).await else {
        found.refused.push(unreachable(
            &ours.id,
            "its quality profiles could not be read",
        ));
        return;
    };
    let profiles: Vec<String> = held.into_iter().map(|profile| profile.name).collect();

    for kind in kinds {
        let (Ok(held), Ok(already)) = (from.records(*kind).await, to.records(*kind).await) else {
            found.refused.push(unreachable(
                &ours.id,
                &format!("its {} could not be read", kind.plural()),
            ));
            continue;
        };

        let wanted = missing(&held, &already);
        found.refused.extend(unmatched(&wanted, &profiles));
        for item in carryable(&wanted, &profiles) {
            let record = RecordReport {
                service: ours.id.clone(),
                kind: kind.plural().to_owned(),
                name: item.name.clone(),
            };
            found.going.push(Going {
                service: ours.id.clone(),
                kind: *kind,
                item: item.clone(),
            });
            if !confirmed {
                continue;
            }
            // Put where the client is already open, rather than opening a second one to
            // write through: two openings are two chances for them to disagree about
            // which service this is.
            match to.carry(*kind, &item).await {
                Ok(()) => found.carried.push(record),
                Err(failure) => found.refused.push(UnsupportedReport {
                    what: item.name.clone(),
                    because: format!("{} would not take it: {failure}", ours.id),
                }),
            }
        }
    }
}

/// Where lemonfiber's own copy of a service is, and what it holds worth carrying.
///
/// Nothing for a service lemonfiber does not run, which is the ordinary answer for the
/// rest of somebody's stack — a request manager, a subtitle finder, anything they added
/// themselves.
fn holds(
    services: &[lemonfiber_manifest::Service],
    ctx: &Ctx,
    service: &str,
) -> Option<(&'static [Record], Target)> {
    let ours = target_named(
        services,
        project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref()).as_deref(),
        service,
    )?;
    Some((kinds_for(&ours.id), ours))
}

/// Where their copy of a service is, and what its key is read from.
///
/// The base comes from the port the engine says it publishes, and the key from the
/// configuration file under one of the paths it mounts — which is the only way to reach
/// a stack whose Compose file lemonfiber has never seen.
fn theirs(container: &Container, ours: &Target) -> Option<Target> {
    let port = container
        .published
        .first()
        .map(|published| published.port)?;
    let config = container
        .mounts
        .first()
        .map(|mount| mount.join("config.xml"))?;
    Some(Target {
        id: ours.id.clone(),
        name: ours.name.clone(),
        base: format!("http://127.0.0.1:{port}"),
        config,
        version: ours.version,
    })
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

/// What is answered where there is no one setup to carry anything out of.
fn nothing(survey: &MigrationReport) -> ImportReport {
    let why = if survey.read {
        "there is no single setup here holding services lemonfiber runs, so there is \
         nothing to carry across"
    } else {
        "what is on this machine could not be read, so there is nothing to carry across"
    };
    ImportReport {
        refused: Some(why.to_owned()),
        ..ImportReport::default()
    }
}

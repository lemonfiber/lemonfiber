//! Replacing a media \*arr's own API key, and handing the new one to everything that
//! reads it.
//!
//! The one rotation that cannot prove before it replaces. The \*arr makes the new key
//! itself and drops the old one in the same moment, so the order kept everywhere else
//! — set, prove, then record — becomes ask, read the new key back, prove it, then hand
//! it out. A refused ask leaves the old key in force; a new key that will not answer
//! is said as exactly that, with the command that finishes the job once it does.
//!
//! What reads the key is reached in the same run rather than left to the next seeding:
//! Prowlarr's application, the subtitle finder and the request gate's route each hold
//! a copy, and a copy left behind is one that stops working the moment the reset lands.
//! Prowlarr's indexer aggregator keeps the republishing rotation: it files no media,
//! and every \*arr reads its key, which is a different list of consumers.

use std::path::Path;
use std::time::Duration;

use lemonfiber_manifest::Service;

use super::rotating::would_rotate;
use crate::app::targets::{record_secret, target_for};
use crate::app::Ctx;
use crate::credential::{Held, Propagation, Reach, Rotation, Settled};
use crate::doctor::credentials::Target;
use crate::ports::service::Client as _;
use crate::seed::run::published_as;
use crate::seed::State;

/// What a rehearsal says about replacing an \*arr's own key.
const RESETTING: &str = "a real run would ask the service to replace its own key, read the \
     new one it writes, prove the service answers to it, and only then hand it to everything \
     that reads it. Nothing was replaced and nothing was handed out.";

/// How many times the configuration file is read for the new key before the reset is
/// said not to have written one. The pinned \*arrs write it before they answer.
const READS: u32 = 10;

/// What a copy says that ended neither holding the new key nor failing in words of
/// its own.
const UNTAKEN: &str = "it did not take the new key; `lemonfiber seed` says why";

/// The pause between those reads.
const BETWEEN_READS: Duration = Duration::from_millis(500);

/// The media \*arr whose key `held` is, where it is one: a Servarr-shaped service that
/// files media. Nothing for every other service key, including the aggregator's.
pub(super) fn resettable(
    held: &Held,
    services: &[Service],
    project: Option<&Path>,
) -> Option<Target> {
    services
        .iter()
        .filter(|service| !service.media_types.is_empty())
        .filter_map(|service| project.and_then(|project| target_for(service, project)))
        .find(|target| published_as(&target.id) == held.setting)
}

/// Ask the \*arr to replace its key, prove the new one, and hand it to every copy.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    target: Target,
) -> Rotation {
    // Above the read of the key: reading it would put a credential in this run's
    // memory to describe a rotation, and the reset itself is the one step a rehearsal
    // must never take.
    if ctx.dry_run {
        return would_rotate(held, RESETTING);
    }
    reset(
        ctx,
        &held.name,
        &held.setting,
        services,
        fillers,
        project,
        target,
    )
    .await
}

/// Replace the key of the \*arr `target` is, on a run that means it: what taking a key
/// back from a service that held it is, as well as what a rotation asked for is.
pub(crate) async fn reset_arr(
    ctx: &Ctx,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    target: Target,
) -> Rotation {
    let name = format!("{} API key", target.name);
    let setting = published_as(&target.id);
    reset(ctx, &name, &setting, services, fillers, project, target).await
}

/// The reset itself, recorded as `setting` and reported as `name`.
async fn reset(
    ctx: &Ctx,
    name: &str,
    setting: &str,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    target: Target,
) -> Rotation {
    let fs = ctx.seams.filesystem.as_ref();
    let Some(old) = target.key(fs).await else {
        return Rotation::stopped(
            name,
            Settled::Unproven {
                detail: "the service has not written an API key yet, so there is none to replace"
                    .to_owned(),
            },
        );
    };
    let service = |key: String| {
        crate::servarr::Servarr::new(
            ctx.seams.http.clone(),
            &target.base,
            key,
            &target.id,
            target.version,
        )
    };
    if let Err(failure) = service(old.clone()).replace_key().await {
        return Rotation::stopped(
            name,
            Settled::Refused {
                detail: crate::config::store::withheld_text(&format!(
                    "the service would not replace its key: {failure}. The existing key is \
                     untouched and still in force."
                )),
            },
        );
    }
    let Some(new) = written_after(&target, fs, &old).await else {
        return lost(name, "it wrote no new key to its configuration");
    };
    let identity = match service(new.clone()).identity().await {
        Ok(identity) => identity,
        Err(failure) => return lost(name, &failure.to_string()),
    };
    record_secret(ctx, setting, &new);
    let mut consumers: Vec<Propagation> = super::reading::service_consumers(&target.name, setting)
        .into_iter()
        .map(|(consumer, reached)| Propagation {
            consumer,
            reach: reached.reach(),
        })
        .collect();
    consumers.extend(copies(ctx, services, fillers, project, &target).await);
    Rotation::landed(
        name,
        &format!(
            "{} {} replaced its key and answered to the new one",
            identity.name, identity.version
        ),
        consumers,
    )
}

/// The key the service's configuration holds once it is no longer `old`, read a
/// bounded number of times.
async fn written_after(
    target: &Target,
    fs: &dyn crate::ports::filesystem::FileSystem,
    old: &str,
) -> Option<String> {
    for _ in 0..READS {
        if let Some(new) = target.key(fs).await.filter(|key| key != old) {
            return Some(new);
        }
        tokio::time::sleep(BETWEEN_READS).await;
    }
    None
}

/// Every other service holding a copy of the \*arr's key, each given the new one.
///
/// Only the copies this stack has: no Prowlarr, no subtitle finder, or no gate in front
/// of the request service is a consumer that is not there rather than one left behind.
async fn copies(
    ctx: &Ctx,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    target: &Target,
) -> Vec<Propagation> {
    let arr = target.name.as_str();
    let mut copies = Vec::new();
    for (prowlarr, state) in crate::seed::run::resync_application(ctx, fillers, &target.id).await {
        copies.push(copy(
            format!("{prowlarr}, which supplies {arr} with indexers"),
            state,
        ));
    }
    for (bazarr, state) in crate::seed::run::rewatch(ctx, fillers, &target.id).await {
        copies.push(copy(
            format!("{bazarr}, which finds subtitles for {arr}"),
            state,
        ));
    }
    if let Some(state) =
        crate::seed::run::reroute(ctx, services, fillers, project, &target.id).await
    {
        copies.push(copy(
            format!("the request gate, which reaches {arr} for the request service"),
            state,
        ));
    }
    copies
}

/// One copy, and whether it now holds the new key.
fn copy(consumer: String, state: State) -> Propagation {
    let reach = match state {
        State::Failed { detail } => Reach::Failed {
            detail: crate::config::store::withheld_text(&detail),
        },
        settled if settled.is_settled() => Reach::Updated,
        _ => Reach::Failed {
            detail: UNTAKEN.to_owned(),
        },
    };
    Propagation { consumer, reach }
}

/// A reset the service made whose new key did not answer: the old key is gone too.
fn lost(name: &str, reason: &str) -> Rotation {
    Rotation::stopped(
        name,
        Settled::ReplacedUnproven {
            detail: crate::config::store::withheld_text(&format!(
                "the service replaced its key and the new one did not answer: {reason}. The old \
                 key no longer works; run `lemonfiber seed` once the service answers, which \
                 hands the new key to everything that reads it."
            )),
        },
    )
}

#[cfg(test)]
mod tests;

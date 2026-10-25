//! Publishing each service's key where the stack's own services read it.
//!
//! Two services in the stack are configured by what they read out of the environment
//! rather than by an API call: the quality sync and the archive extractor each name the
//! curators they work with and expect a key for each. Neither can be told anything over
//! HTTP — there is nothing to POST to — so the only way to wire them is to put the keys
//! where they look.
//!
//! **The names here are this product's own, not theirs.** A key is published as
//! `{SERVICE}_API_KEY`, and a consumer's declaration names a secret of that service
//! under whatever variable it reads — the extractor wants `UN_SONARR_0_API_KEY` — which
//! lemonfiber writes into the consumer's secrets file. Which means a service added later
//! needs a line of its declaration rather than a line of Rust, and this file never
//! learns any consumer's vocabulary.
//!
//! **Only keys a service already wrote down are published.** Nothing here is minted for
//! the purpose. The dashboard is published to the household network and holds no
//! credential of any service, so a media server API key filed under lemonfiber's name,
//! and a listening server token in the environment file, are retired rather than
//! published.
//!
//! A service that has not written its key yet is left out rather than published as
//! empty: the quality sync refuses its whole configuration over one undefined variable,
//! so an empty value is worse than an absent one.

use lemonfiber_manifest::Service;

use super::keys::read_curator_key;
use super::Ctx;

/// What this connection is called where it is reported.
const CONNECTION: &str = "Keys the stack's own services read";

/// The service in this stack answering a given kind of API, where there is one.
fn with_api(services: &[Service], kind: lemonfiber_manifest::ApiKind) -> Option<&Service> {
    services
        .iter()
        .find(|service| service.api.as_ref().is_some_and(|api| api.kind == kind))
}

/// The environment name a service's key is published under.
pub(crate) fn published_as(id: &str) -> String {
    crate::config::for_service(id, crate::config::API_KEY_SUFFIX)
}

/// Put every key this pass could read where the stack's own services read it.
pub(super) async fn publish_keys(
    ctx: &Ctx,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&std::path::Path>,
) -> crate::seed::Wiring {
    let published = written_down(ctx, services, project).await;

    // Before anything is written: retiring revokes a key, so a rehearsal that got past
    // here would have changed the very thing it promised only to describe.
    if ctx.dry_run {
        return would_publish(published);
    }

    retired(ctx, services, fillers).await;

    let state = if published.is_empty() {
        nothing_to_publish()
    } else {
        recorded(ctx, published)
    };
    crate::seed::Wiring::settled(CONNECTION.to_owned(), state)
}

/// Record every key, write it again into every secrets file that reads it where it
/// changed, and say which could not be recorded or written where any could not.
///
/// A key left out is a consumer left reading a stale one, so the connection is not
/// called wired over it; the rest are still recorded, since each one that lands is a
/// consumer that works.
fn recorded(ctx: &Ctx, published: Vec<(String, String)>) -> crate::seed::State {
    let mut unrecorded = Vec::new();
    let mut unwritten = Vec::new();
    for (name, key) in published {
        let moved = crate::app::targets::recorded_secret(ctx, &name).as_deref() != Some(&key);
        if let Err(failure) = crate::app::targets::record_secret(ctx, &name, &key) {
            unrecorded.push(format!("{name} ({failure})"));
        } else if moved {
            let holding = crate::plugin::Holding::Crossing(name.clone());
            if let Err(problem) = crate::settings::files::reread_now(ctx, &holding) {
                unwritten.push(format!("{name} ({})", problem.summary));
            }
        }
    }
    let said: Vec<String> = [
        ("these keys could not be recorded", unrecorded),
        (
            "these keys were recorded and could not be written into every file that reads them",
            unwritten,
        ),
    ]
    .into_iter()
    .filter(|(_, names)| !names.is_empty())
    .map(|(what, names)| format!("{what}: {}", names.join(", ")))
    .collect();
    if said.is_empty() {
        return crate::seed::State::Wired;
    }
    crate::seed::State::Failed {
        detail: said.join("; "),
    }
}

/// Why there is nothing to publish yet, in both tenses: the services write their keys
/// on first start, and a stack that has not got that far has none to read.
fn nothing_to_publish() -> crate::seed::State {
    crate::seed::State::Skipped {
        reason: "no service has written a key yet; a later run completes it".to_owned(),
    }
}

/// What a rehearsal says this connection would do: name the settings, and not one
/// character of what would go in them.
///
/// Every pair gathered here is a setting and the credential destined for it, and the
/// report is serialized — so the names are the whole of what an operator is deciding
/// about, and the values are the one thing a question must never make a second copy of.
fn would_publish(written: Vec<(String, String)>) -> crate::seed::Wiring {
    let mut settings: Vec<String> = written.into_iter().map(|(name, _)| name).collect();
    settings.sort();
    settings.dedup();
    let state = if settings.is_empty() {
        nothing_to_publish()
    } else {
        crate::seed::State::WouldWire {
            yours: None,
            ours: Some(settings.join(", ")),
        }
    };
    crate::seed::Wiring::settled(CONNECTION.to_owned(), state)
}

/// A key published under the id of the service answering `kind`, where both the key
/// and the service are there.
///
/// The pairing every one of these needs: a key is worth nothing without the name to
/// file it under, and the name comes from the service that answered.
fn under_its_service(
    services: &[Service],
    kind: lemonfiber_manifest::ApiKind,
    key: Option<String>,
) -> Option<(String, String)> {
    let service = with_api(services, kind)?;
    Some((published_as(&service.id), key?))
}

/// The keys the services wrote to disk themselves.
///
/// Every curator-shaped service, not only the ones that file media: the indexer aggregator
/// manages none and so declares no media types, which is what keeps it out of the curator
/// list used for root folders and download clients — but it has a key like the rest, and a
/// service added to the stack is given it by a secret its declaration names. The request
/// service is not curator-shaped and keeps its key in a file of its own shape.
async fn written_down(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&std::path::Path>,
) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for service in services {
        let Some(target) = project.and_then(|project| super::target_for(service, project)) else {
            continue;
        };
        if let Some(key) = read_curator_key(ctx, &target.config).await {
            found.push((published_as(&target.id), key));
        }
    }
    let requests = crate::app::targets::stack_requests_key(ctx, services, project).await;
    found.extend(under_its_service(
        services,
        lemonfiber_manifest::ApiKind::Seerr,
        requests,
    ));
    found
}

/// Revoke the media server key filed under lemonfiber's name, and forget the media
/// server's and the listening server's settings in the environment file.
///
/// The media server's is forgotten only once its key is off the server — or was never
/// there — so a revocation that failed leaves the value where the next run can still
/// find which key it was. The listening server's token cannot be revoked on its own;
/// forgetting it is what leaves nothing on this machine holding it.
async fn retired(ctx: &Ctx, services: &[Service], fillers: &crate::wiring::Fillers) {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return;
    };
    let server = crate::app::targets::MediaServer::of(fillers);
    let client = match &server {
        Some(server) => server.administering(ctx).await,
        None => None,
    };
    let revoked = match client {
        Some(client) => crate::app_keys::revoke_ours(client.as_ref()).await.ok(),
        None => None,
    };
    let media_server = server
        .filter(|_| revoked.is_some())
        .map(|server| published_as(server.id()));
    let listening = with_api(services, lemonfiber_manifest::ApiKind::Audiobookshelf)
        .map(|service| published_as(&service.id));
    // Only what is there: the file is rewritten by every removal, and a setting that
    // was never published is not a reason to touch it.
    for setting in media_server
        .into_iter()
        .chain(listening)
        .filter(|setting| crate::app::targets::recorded_secret(ctx, setting).is_some())
    {
        let _ = crate::config::store::unset(env, &setting);
    }
}

#[cfg(test)]
mod tests;

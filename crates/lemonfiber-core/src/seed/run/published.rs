//! Publishing each service's key where the stack's own services read it.
//!
//! Two services in the stack are configured by what they read out of the environment
//! rather than by an API call: the quality sync and the archive extractor each name the
//! \*arrs they work with and expect a key for each. Neither can be told anything over
//! HTTP — there is nothing to POST to — so the only way to wire them is to put the keys
//! where they look.
//!
//! **The names here are this product's own, not theirs.** A key is published as
//! `{SERVICE}_API_KEY`, and the stack maps that to whatever each consumer calls it —
//! the extractor wants `UN_SONARR_0_API_KEY`. Which means a service added later needs a
//! line of Compose rather than a line of Rust, and this file never learns any
//! consumer's vocabulary.
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

use super::clients::Held;
use super::curating::read_servarr_key;
use super::Ctx;
use crate::ports::service::Credential;

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
///
/// `held` is what was already gathered for the download-client registration, passed
/// in rather than read again — the same files, and a second read could only fail where
/// the first had.
pub(super) async fn publish_keys(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&std::path::Path>,
    held: &Held,
) -> crate::seed::Wiring {
    let mut published = written_down(ctx, services, project).await;

    // Before anything is written: retiring revokes a key, so a rehearsal that got past
    // here would have changed the very thing it promised only to describe.
    if ctx.dry_run {
        return would_publish(published, held);
    }

    retired(ctx, services).await;
    published.extend(from_the_clients(held));

    let state = if published.is_empty() {
        nothing_to_publish()
    } else {
        recorded(ctx, published)
    };
    crate::seed::Wiring::settled(CONNECTION.to_owned(), state)
}

/// Record every key, and say which could not be recorded where any could not.
///
/// A key left out is a consumer left reading a stale one, so the connection is not
/// called wired over it; the rest are still recorded, since each one that lands is a
/// consumer that works.
fn recorded(ctx: &Ctx, published: Vec<(String, String)>) -> crate::seed::State {
    let unrecorded: Vec<String> = published
        .into_iter()
        .filter_map(|(name, key)| {
            crate::app::targets::record_secret(ctx, &name, &key)
                .err()
                .map(|failure| format!("{name} ({failure})"))
        })
        .collect();
    if unrecorded.is_empty() {
        return crate::seed::State::Wired;
    }
    crate::seed::State::Failed {
        detail: format!(
            "these keys could not be recorded: {}",
            unrecorded.join(", ")
        ),
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
fn would_publish(written: Vec<(String, String)>, held: &Held) -> crate::seed::Wiring {
    let mut settings: Vec<String> = written.into_iter().map(|(name, _)| name).collect();
    settings.extend(from_the_clients(held).into_iter().map(|(name, _)| name));
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
/// Every Servarr-shaped service, not only the ones that file media: Prowlarr manages
/// none and so declares no media types, which is what keeps it out of the \*arr list
/// used for root folders and download clients — but it has a key like the rest, and a
/// service added to the stack is given it with a line of Compose. The other two are not
/// Servarr-shaped and each keeps its key in a file of its own shape.
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
        if let Some(key) = read_servarr_key(ctx, &target.config).await {
            found.push((published_as(&target.id), key));
        }
    }
    let bazarr = crate::app::targets::bazarr_key(ctx, services, project).await;
    found.extend(under_its_service(
        services,
        lemonfiber_manifest::ApiKind::Bazarr,
        bazarr,
    ));
    let seerr = crate::app::targets::seerr_key(ctx, services, project).await;
    found.extend(under_its_service(
        services,
        lemonfiber_manifest::ApiKind::Seerr,
        seerr,
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
async fn retired(ctx: &Ctx, services: &[Service]) {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return;
    };
    let revoked = crate::app::targets::revoke_jellyfin_key(ctx, services).await;
    let media_server =
        with_api(services, lemonfiber_manifest::ApiKind::Jellyfin).filter(|_| revoked.is_some());
    let listening = with_api(services, lemonfiber_manifest::ApiKind::Audiobookshelf);
    // Only what is there: the file is rewritten by every removal, and a setting that
    // was never published is not a reason to touch it.
    for setting in media_server
        .into_iter()
        .chain(listening)
        .map(|service| published_as(&service.id))
        .filter(|setting| crate::app::targets::recorded_secret(ctx, setting).is_some())
    {
        let _ = crate::config::store::unset(env, &setting);
    }
}

/// What the stack's own download clients' credentials publish: each client's key under
/// its own name, and the account name each torrent client is reached under.
///
/// The stack's alone. What reads these is the stack's own Compose — the tunnel's
/// forwarded-port push reads the torrent client's account — and a plugin's client has
/// nothing reading its credential out of the environment, so writing it there would be
/// a second copy of a secret nobody asked for.
///
/// The name is published only once a password exists for it to pair with — one half of
/// a credential authenticates with neither, and a name published on its own would let
/// this connection report success on a stack where nothing was read at all. The
/// password itself is already recorded under its own setting by the run that minted it.
fn from_the_clients(held: &Held) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (service, credential) in held.stacks() {
        found.push(match credential {
            Credential::ApiKey(key) => (published_as(service), key.clone()),
            Credential::UserPass { username, .. } => (
                crate::config::for_service(service, crate::config::USERNAME_SUFFIX),
                username.clone(),
            ),
        });
    }
    found
}

#[cfg(test)]
mod tests;

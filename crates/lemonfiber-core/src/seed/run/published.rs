//! Publishing each service's key where the stack's own services read it.
//!
//! Three services in the stack are configured by what they read out of the
//! environment rather than by an API call: the quality sync, the archive extractor
//! and the dashboard each name the \*arrs they work with and expect a key for each.
//! None of them can be told anything over HTTP — there is nothing to POST to — so
//! the only way to wire them is to put the keys where they look.
//!
//! **The names here are this product's own, not theirs.** A key is published as
//! `{SERVICE}_API_KEY`, and the stack maps that to whatever each consumer calls it —
//! one of them wants `UN_SONARR_0_API_KEY`, another `HOMEPAGE_VAR_SONARR_KEY`. Which
//! means a service added later needs a line of Compose rather than a line of Rust,
//! and this file never learns any consumer's vocabulary.
//!
//! Only keys that were actually read are published. A service that has not written
//! its key yet is left out rather than published as empty: the quality sync refuses
//! its whole configuration over one undefined variable, so an empty value is worse
//! than an absent one.

use lemonfiber_manifest::Service;

use super::arrs::read_servarr_key;
use super::Ctx;

/// What this connection is called where it is reported.
const CONNECTION: &str = "Keys the stack's own services read";

/// The suffix a published key is named with.
const SUFFIX: &str = "_API_KEY";

/// The service in this stack answering a given kind of API, where there is one.
fn with_api(services: &[Service], kind: lemonfiber_manifest::ApiKind) -> Option<&Service> {
    services
        .iter()
        .find(|service| service.api.as_ref().is_some_and(|api| api.kind == kind))
}

/// The environment name a service's key is published under.
///
/// Upper-cased, with anything that cannot appear in an environment name replaced —
/// a service id is a Compose name and may carry hyphens, which a shell would read
/// as an operator rather than as part of the name.
pub(crate) fn published_as(id: &str) -> String {
    let name: String = id
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{name}{SUFFIX}")
}

/// Put every key this pass could read where the stack's own services read it.
///
/// `sabnzbd_key` is the one already read for the download-client registration, passed
/// in rather than read again — the same file, and a second read could only fail where
/// the first had.
pub(super) async fn publish_keys(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&std::path::Path>,
    sabnzbd_key: Option<&str>,
) -> crate::seed::Wiring {
    let mut published = written_down(ctx, services, project).await;

    // Before [`asked_for`], because asking is where this connection does its writing.
    // Both of the keys it gathers are read by being made — the media server mints one
    // when it is asked for one, and the listening server has no account at all until
    // this makes it, with a password minted and recorded to go with it — so a pass that
    // gathered them would have created the very things it promised only to describe,
    // and left a secret behind for a question.
    if ctx.dry_run {
        return would_publish(ctx, services, published, sabnzbd_key);
    }

    published.extend(asked_for(ctx, services).await);
    published.extend(pairs_with_a_password(ctx, services, sabnzbd_key));

    if published.is_empty() {
        return crate::seed::Wiring::settled(CONNECTION.to_owned(), nothing_to_publish());
    }

    for (name, key) in published {
        crate::app::targets::record_secret(ctx, &name, &key);
    }

    crate::seed::Wiring::settled(CONNECTION.to_owned(), crate::seed::State::Wired)
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
///
/// Two of the names come from the stack rather than from a gathered key, because those
/// two are the ones [`asked_for`] would have had to create to learn. A real run reaches
/// this line having just minted the media server's admin password and made the
/// listening server's first account; this one did neither, and taking the names from
/// the services present says what a real run would fill without filling anything.
fn would_publish(
    ctx: &Ctx,
    services: &[Service],
    written: Vec<(String, String)>,
    sabnzbd_key: Option<&str>,
) -> crate::seed::Wiring {
    let mut settings: Vec<String> = written.into_iter().map(|(name, _)| name).collect();
    settings.extend(would_ask_for(services));
    settings.extend(
        pairs_with_a_password(ctx, services, sabnzbd_key)
            .into_iter()
            .map(|(name, _)| name),
    );
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

/// The names the asked-for keys would be published under, taken without asking for
/// them.
///
/// Named from the stack rather than from what the services answered, which is the only
/// way to name them at all without making them: see [`would_publish`].
fn would_ask_for(services: &[Service]) -> Vec<String> {
    [
        lemonfiber_manifest::ApiKind::Audiobookshelf,
        lemonfiber_manifest::ApiKind::Jellyfin,
    ]
    .into_iter()
    .filter_map(|kind| with_api(services, kind))
    .map(|service| published_as(&service.id))
    .collect()
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
/// used for root folders and download clients — but it has a key, and the dashboard
/// has a widget that reads it. The other two are not Servarr-shaped and each keeps its
/// key in a file of its own shape.
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

/// The keys no service writes down, which have to be asked for.
///
/// The media server keeps its own in a database, and the listening server has no
/// account at all until one is made — so where this makes one, the password it used is
/// recorded, since that is the durable half. The token is signed in for again on every
/// later run.
async fn asked_for(ctx: &Ctx, services: &[Service]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Some((token, minted)) = crate::app::targets::audiobookshelf_token(ctx, services).await {
        if let Some(password) = &minted {
            crate::app::targets::record_secret(
                ctx,
                crate::config::AUDIOBOOKSHELF_PASSWORD_KEY,
                password,
            );
        }
        found.extend(under_its_service(
            services,
            lemonfiber_manifest::ApiKind::Audiobookshelf,
            Some(token),
        ));
    }
    let jellyfin = crate::app::targets::jellyfin_key(ctx, services).await;
    found.extend(under_its_service(
        services,
        lemonfiber_manifest::ApiKind::Jellyfin,
        jellyfin,
    ));
    found
}

/// The credentials that are not a key: one already read for the download clients, and
/// one account name.
///
/// The name is published only once a password exists for it to pair with — a dashboard
/// holding one half of a credential authenticates with neither, and a name published
/// on its own would let this connection report success on a stack where nothing was
/// read at all.
fn pairs_with_a_password(
    ctx: &Ctx,
    services: &[Service],
    sabnzbd_key: Option<&str>,
) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = services
        .iter()
        .find(|service| service.id == "sabnzbd")
        .zip(sabnzbd_key)
        .map(|(service, key)| (published_as(&service.id), key.to_owned()))
        .into_iter()
        .collect();
    if with_api(services, lemonfiber_manifest::ApiKind::Qbittorrent).is_some()
        && crate::app::targets::recorded_qbittorrent_password(ctx).is_some()
    {
        found.push((
            crate::config::QBITTORRENT_USERNAME_KEY.to_owned(),
            crate::config::QBITTORRENT_USER.to_owned(),
        ));
    }
    found
}

#[cfg(test)]
mod tests;

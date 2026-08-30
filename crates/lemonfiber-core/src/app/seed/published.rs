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
fn published_as(id: &str) -> String {
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
    published.extend(asked_for(ctx, services).await);
    published.extend(pairs_with_a_password(ctx, services, sabnzbd_key));

    if published.is_empty() {
        return crate::seed::Wiring::settled(
            CONNECTION.to_owned(),
            crate::seed::State::Skipped {
                reason: "no service has written a key yet; a later run completes it".to_owned(),
            },
        );
    }

    for (name, key) in published {
        super::super::targets::record_secret(ctx, &name, &key);
    }

    crate::seed::Wiring::settled(CONNECTION.to_owned(), crate::seed::State::Wired)
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
    let bazarr = super::super::targets::bazarr_key(ctx, services, project).await;
    found.extend(under_its_service(
        services,
        lemonfiber_manifest::ApiKind::Bazarr,
        bazarr,
    ));
    let seerr = super::super::targets::seerr_key(ctx, services, project).await;
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
    if let Some((token, minted)) = super::super::targets::audiobookshelf_token(ctx, services).await
    {
        if let Some(password) = &minted {
            super::super::targets::record_secret(
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
    let jellyfin = super::super::targets::jellyfin_key(ctx, services).await;
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
        && super::super::targets::recorded_qbittorrent_password(ctx).is_some()
    {
        found.push((
            crate::config::QBITTORRENT_USERNAME_KEY.to_owned(),
            crate::config::QBITTORRENT_USER.to_owned(),
        ));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::published_as;

    /// The name is the service's own, upper-cased.
    #[test]
    fn a_key_is_published_under_the_service_it_came_from() {
        assert_eq!(published_as("sonarr"), "SONARR_API_KEY");
        assert_eq!(published_as("sabnzbd"), "SABNZBD_API_KEY");
    }

    /// A hyphen is not something an environment name may carry.
    ///
    /// Service ids are Compose names and several of them are hyphenated. A shell
    /// reads a hyphen as an operator, so a name built straight from the id would be
    /// one nothing could reference.
    #[test]
    fn a_hyphenated_service_is_published_under_a_name_a_shell_can_read() {
        assert_eq!(
            published_as("calibre-web-automated"),
            "CALIBRE_WEB_AUTOMATED_API_KEY"
        );
    }
}

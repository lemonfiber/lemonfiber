//! The \*arrs the request service hands a request to.
//!
//! The request service does not discover them. Until it is told, a household member
//! asks for something, the ask is accepted, and no downloader ever hears about it.
//!
//! Only the \*arrs actually in the stack are offered, which is the half that decides
//! what the household may ask for at all: the request service offers what its
//! targets can deliver, so television is not offered where Sonarr is not running.
//!
//! Two of the four \*arrs are request targets. The request service fetches film and
//! television and nothing else, so the ones filing music and books are not targets —
//! not an omission, but the same rule applied: an \*arr that cannot fulfil a request
//! is not offered as somewhere to send one.

use std::path::Path;

use lemonfiber_manifest::Service;

use super::arrs::{reached_at, read_servarr_key, servarr_arrs};
use super::Ctx;
use crate::ports::service::{Client as _, FulfilmentTarget, QualityProfile};

/// The media type an \*arr must file for the request service to send it anything.
const TELEVISION: &str = "tv";
const FILM: &str = "movies";

/// Every \*arr in this stack the request service should hand requests to.
///
/// Each is read rather than assumed: the profile it fetches at and the folder it
/// files into are asked of the \*arr itself, because the request service must name
/// both when it hands over a request, and an operator may have renamed or replaced
/// what setup created.
///
/// An \*arr that cannot answer, or that has no profile or folder to name, is left
/// out rather than registered half-configured — a target the request service holds
/// but cannot fetch through is worse than one it does not hold, because the request
/// is accepted either way and only the second is visibly missing.
pub(super) async fn wanted_targets(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&Path>,
) -> Vec<FulfilmentTarget> {
    let mut wanted = Vec::new();
    for arr in servarr_arrs(services, project) {
        // Taken together because only the first can actually decline: an \*arr that
        // reached this point came from a service that publishes a port, so there is
        // no separate way for the endpoint to be missing.
        let Some((television, (host, port))) =
            fetches(&arr.media_types).zip(reached_at(services, &arr.target.id))
        else {
            continue;
        };
        let Some(key) = read_servarr_key(ctx, &arr.target.config).await else {
            continue;
        };
        // Built from the key just read rather than opened again. Opening re-reads the
        // same file, so a second failure there could only happen if the first had.
        let client = crate::servarr::Servarr::new(
            ctx.http.clone(),
            &arr.target.base,
            key.clone(),
            &arr.target.id,
            arr.target.version,
        );
        let Some(profile) = first_profile(&client).await else {
            continue;
        };
        let Some(folder) = first_folder(&client).await else {
            continue;
        };
        wanted.push(FulfilmentTarget {
            name: arr.target.name.clone(),
            host,
            port,
            key,
            television,
            profile,
            folder,
        });
    }
    wanted
}

/// Whether this \*arr fetches television, film, or neither.
///
/// `None` is not a failure: it is Lidarr or Bindery, which file media the request
/// service does not deal in at all.
fn fetches(media_types: &[String]) -> Option<bool> {
    if media_types.iter().any(|kind| kind == TELEVISION) {
        return Some(true);
    }
    if media_types.iter().any(|kind| kind == FILM) {
        return Some(false);
    }
    None
}

/// The profile requests are fetched at.
///
/// The first the \*arr reports, which is what setup wired: a stack with several is
/// one the operator has arranged themselves, and the request service takes one
/// default rather than choosing between them.
async fn first_profile(client: &crate::servarr::Servarr) -> Option<QualityProfile> {
    client.quality_profiles().await.ok()?.into_iter().next()
}

/// Where what it fetches is filed — the first folder it reports, for the same reason.
async fn first_folder(client: &crate::servarr::Servarr) -> Option<String> {
    client
        .root_folders()
        .await
        .ok()?
        .into_iter()
        .map(|folder| folder.path)
        .next()
}

/// Hand the request service every \*arr this stack has that can fulfil a request.
///
/// Nothing to do where the stack has no request service: there is nobody to tell.
pub(super) async fn seed_fulfilment_targets(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&Path>,
) -> Vec<crate::seed::Wiring> {
    // Asked before anything else, because what follows asks every \*arr what it holds
    // and there is no sense doing that with nobody to tell about it.
    let Some(base) = super::identity::seerr_service(services) else {
        return Vec::new();
    };
    let wanted = wanted_targets(ctx, services, project).await;
    if wanted.is_empty() {
        return Vec::new();
    }
    // Signed in, because every call that follows is an authenticated one: registering
    // a target reads what the service already holds and then writes. Unsigned, all of
    // it comes back as a refusal about a credential.
    let seerr = super::super::targets::seerr_as_owner(ctx, base).await;
    let mut journal = crate::journal::Journal::new();
    crate::seed::wire_fulfilment_targets(&seerr, &wanted, &mut journal, &ctx.stamp()).await
}

#[cfg(test)]
mod tests {
    use super::{fetches, reached_at, Service};

    fn kinds(of: &[&str]) -> Vec<String> {
        of.iter().map(|kind| (*kind).to_owned()).collect()
    }

    /// One container in a stack, with only the fields this module reads set to
    /// anything meaningful.
    fn a_service(id: &str, port: Option<u16>) -> Service {
        Service {
            id: id.to_owned(),
            name: format!("{id} the app"),
            profile: "media".to_owned(),
            image: "example/image".to_owned(),
            tag: "1".to_owned(),
            port,
            bind: None,
            health: None,
            api: None,
            criticality: lemonfiber_manifest::Criticality::Core,
            license: "MIT".to_owned(),
            upstream: "https://example.test".to_owned(),
            last_release: "2026-01-01".to_owned(),
            describes: "an example service".to_owned(),
            without_it: "nothing works".to_owned(),
            media_types: Vec::new(),
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            host_managed: false,
        }
    }

    /// Which \*arr is a request target, and which is not one at all.
    ///
    /// The third case is the one that carries the requirement: music and books are
    /// filed by \*arrs the request service cannot fetch through, so they are not
    /// offered as somewhere to send a request. Asserting only the first two would
    /// pass a version that offered every \*arr it found.
    #[test]
    fn only_the_arrs_that_fetch_film_and_television_are_targets() {
        assert_eq!(fetches(&kinds(&["tv"])), Some(true), "television");
        assert_eq!(fetches(&kinds(&["movies"])), Some(false), "film");
        assert_eq!(
            fetches(&kinds(&["music"])),
            None,
            "music is not requestable"
        );
        assert_eq!(
            fetches(&kinds(&["books"])),
            None,
            "books are not requestable"
        );
        assert_eq!(fetches(&[]), None, "an *arr filing nothing is not a target");
    }

    /// The request service reaches an \*arr by container name, not by loopback.
    ///
    /// It is a container itself, so `127.0.0.1` there is the request service rather
    /// than the \*arr — an address that resolves and answers, wrongly.
    #[test]
    fn an_arr_is_named_by_its_container_and_port() {
        let services = vec![a_service("sonarr", Some(8989))];

        assert_eq!(
            reached_at(&services, "sonarr"),
            Some(("sonarr".to_owned(), 8989))
        );
        assert_eq!(
            reached_at(&services, "radarr"),
            None,
            "an *arr that is not in the stack has nowhere to be reached"
        );
        assert_eq!(
            reached_at(&[a_service("sonarr", None)], "sonarr"),
            None,
            "an *arr publishing no port has no endpoint to hand over"
        );
    }
}

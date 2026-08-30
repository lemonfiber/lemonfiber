//! Opening an authenticated client for a service.
//!
//! A target says where a service is and where its key is written; this reads the key and
//! hands back something that can talk. Absent where the key is not written yet, which is a
//! service still starting rather than a fault.

use crate::app::Ctx;
use crate::doctor::credentials::Target;
use crate::jellyfin::Jellyfin;
use crate::prowlarr::Prowlarr;
use crate::sabnzbd::Sabnzbd;
use crate::seerr::Seerr;
use crate::servarr::Servarr;
use std::path::Path;

use crate::recyclarr::Kind;

use super::downloads::download_targets;
use super::layout::project_directory;
use super::secrets::recorded_secret;
use super::servarr::{servarr_targets, target_for, DownloadKind};

/// The household's Jellyfin as a reading client, for the last stage of a trace —
/// whether the item is finally in the library. Present only where the stack has a
/// Jellyfin and lemonfiber recorded the admin password it minted for it: the read signs
/// in with the household's own credential, so without it there is nothing to sign in as.
///
/// A trace treats its absence as one more thing it cannot tell rather than a fault, so
/// either gap simply leaves the availability question unanswered.
pub(crate) fn jellyfin_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<Jellyfin> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let password = recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    Some(Jellyfin::authenticated(
        ctx.http.clone(),
        addr.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    ))
}

/// One \*arr a read can be made against: the service it files, a client already carrying
/// its key, and the name a report calls it by.
pub(crate) struct OpenArr {
    /// The service's display name, as a report names where a fact came from.
    pub name: String,
    /// Which of the two media services it is.
    pub kind: Kind,
    /// A client carrying the key it wrote.
    pub service: Servarr,
}

/// Every \*arr whose key could be read, ready to be asked something.
///
/// One that has not finished starting has not written its key yet, so it cannot be opened
/// and is left out. That is deliberately not a failed read: a service still coming up
/// holds nothing to report, so its absence understates nothing — the convention every
/// caller here follows, stated once rather than re-derived at each of them.
pub(crate) async fn open_servarrs(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Vec<OpenArr> {
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let mut open = Vec::new();
    for target in servarr_targets(services, project.as_deref()) {
        let Some(kind) = Kind::for_section(&target.id) else {
            continue;
        };
        let Some(service) = target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        open.push(OpenArr {
            name: target.name.clone(),
            kind,
            service,
        });
    }
    open
}

/// The request service, already signed in as the owner.
///
/// **Every authenticated call it takes needs a session**, and nothing but signing in
/// opens one — so a client handed out unsigned is one whose every use comes back as a
/// refusal about a credential, which is what registering the \*arrs did for as long as
/// it existed. Built in one place so that cannot be forgotten again.
///
/// **Takes the address rather than finding it**, so it always hands a client back and
/// the caller keeps the one place that decides there is nobody to talk to. A stack
/// with no credential to sign in with, or a sign-in that fails, still gets a client:
/// whatever is about to use it reports what went wrong in its own words, and handing
/// back nothing would leave the operator with no line at all about work that was
/// attempted and failed — worse than a failure they can read.
pub(crate) async fn seerr_as_owner(ctx: &Ctx, base: String) -> Seerr {
    let seerr = Seerr::new(ctx.http.clone(), base, "seerr");
    if let Some(password) = recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY) {
        let _ = crate::ports::service::Requests::sign_in(
            &seerr,
            crate::config::JELLYFIN_ADMIN_USER,
            &password,
        )
        .await;
    }
    seerr
}

/// What reading the household's requests needs: the request service, and the media-server
/// credential the sign-in is made with. Seerr authenticates its household against Jellyfin,
/// so the account lemonfiber holds a password for is how it asks — as the owner, whose
/// session sees every member's requests.
pub(crate) struct HouseholdAccess {
    /// The request service, reached on the host.
    pub seerr: Seerr,
    /// The media-server admin password lemonfiber minted and recorded.
    pub password: String,
}

/// The request service and the credential to read it with, or nothing where the stack has
/// no request service, no media server to authenticate against, or no recorded password
/// to sign in with.
///
/// The household view treats any of those as nothing to report rather than a fault: a
/// stack without a request service has no household requests, and one whose password was
/// never recorded has no way to ask for them.
pub(crate) fn seerr_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<HouseholdAccess> {
    let seerr = service_addr(services, lemonfiber_manifest::ApiKind::Seerr)?;
    // A stack with no media server has nothing for the request service to authenticate
    // against, so there is nobody to ask on behalf of. Where it is is not needed — the
    // request service was pointed at it when it was set up, and asking it to be pointed
    // somewhere again is what it refuses.
    service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let password = recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    Some(HouseholdAccess {
        seerr: Seerr::new(ctx.http.clone(), seerr.loopback, "seerr"),
        password,
    })
}

/// Where a service is reached — on the host, and across the stack's own network — the
/// two forms every service address is wanted in.
pub(crate) struct ServiceAddr {
    /// The service's compose id: names the container, and is the host in a network URL.
    pub id: String,
    /// Where the host reaches it: `http://127.0.0.1:{port}`.
    pub loopback: String,
    /// Where another container reaches it across the stack network: `http://{id}:{port}`.
    pub network_url: String,
    /// The port the host publishes it on.
    ///
    /// Kept because neither URL above is one to hand a person: both name a host only
    /// this machine or this stack can resolve. An address for the household is built
    /// from what the *machine* is called, and that needs the port on its own.
    pub port: u16,
}

/// The address of the one service of a given api kind, or nothing where the stack has
/// none or it publishes no port to reach it on. The single place the "find the service
/// by its kind, format where it is reached" step lives, so every caller that speaks to a
/// named service resolves it the same way rather than re-deriving the URLs.
pub(crate) fn service_addr(
    services: &[lemonfiber_manifest::Service],
    kind: lemonfiber_manifest::ApiKind,
) -> Option<ServiceAddr> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != kind {
            return None;
        }
        let port = service.port?;
        Some(ServiceAddr {
            id: service.id.clone(),
            loopback: format!("http://127.0.0.1:{port}"),
            network_url: format!("http://{}:{port}", service.id),
            port,
        })
    })
}

/// The stack's Usenet download client, as a reader of the accounts behind it.
///
/// Nothing where the stack has no Usenet client, or where the client has not written
/// its key yet — a service still starting holds nothing to report, the same skip every
/// read here makes.
pub(crate) async fn usenet_client(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<Sabnzbd> {
    let (base, config) = download_targets(services, project)
        .into_iter()
        .find_map(|target| match target.kind {
            DownloadKind::Sabnzbd { config } => Some((target.base, config)),
            DownloadKind::Qbittorrent => None,
        })?;
    let text = ctx.filesystem.read(&config).await?;
    let key = crate::sabnzbd::api_key(&text)?;
    Some(Sabnzbd::new(ctx.http.clone(), base, key))
}

/// The Servarr-shape service that files no media of its own — the indexer aggregator,
/// which is what makes it the one that knows how the indexers have been behaving.
///
/// Identified by what it does rather than by name, like every other service here, so a
/// fork that ships a different aggregator under the same shape resolves the same way.
pub(crate) fn aggregator_target(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<Target> {
    let project = project?;
    // A plain walk rather than a closure: this resolves from two callers in two
    // crates, and a closure instantiated in both is one the coverage gate counts
    // twice and sees run once.
    for service in services {
        if !service.media_types.is_empty() {
            continue;
        }
        if let Some(target) = target_for(service, project) {
            return Some(target);
        }
    }
    None
}

/// The indexer aggregator, ready to be asked how its indexers have been behaving.
///
/// Its API is a major behind the media \*arrs', which is why it is its own client
/// rather than the shared Servarr one — but it writes its key exactly the way they do.
pub(crate) async fn indexer_aggregator(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<Prowlarr> {
    let target = aggregator_target(services, project)?;
    let key = target.key(ctx.filesystem.as_ref()).await?;
    Some(Prowlarr::new(
        ctx.http.clone(),
        &target.base,
        key,
        &target.id,
    ))
}

/// The subtitle finder, holding the key it wrote for itself.
///
/// Nothing where the stack has no subtitle finder, no project to read its
/// configuration from, or where it has not written a key yet — the last is a
/// service still starting rather than a fault, and a later run completes it.
pub(crate) async fn bazarr_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Option<crate::bazarr::Bazarr> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Bazarr)?;
    let service = services.iter().find(|service| service.id == addr.id)?;
    let path = crate::app::targets::config_path(
        project?,
        service,
        service.api.as_ref().and_then(|api| api.path.as_deref()),
    )?;
    let key = crate::bazarr::api_key(&ctx.filesystem.read(&path).await?)?;
    Some(crate::bazarr::Bazarr::new(
        ctx.http.clone(),
        addr.loopback,
        &addr.id,
        key,
    ))
}

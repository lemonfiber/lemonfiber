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
        ctx.seams.http.clone(),
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
        let Some(service) = target
            .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
            .await
        else {
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

/// The request service, carrying its own key, which it answers as its owner.
///
/// **Its own key, never the media server's administrator password.** That password
/// passes through the request service once, on the sign-in that sets it up, and every
/// read and write after that carries the key the service wrote for itself. A key is a
/// header on each request rather than a session left open on somebody else's service,
/// so a pass that only says what it would do reads with it too.
///
/// **Takes the address rather than finding it**, so it always hands a client back and
/// the caller keeps the one place that decides there is nobody to talk to. A service
/// that has not written its key yet still gets a client: whatever is about to use it
/// reports the refusal in its own words, and handing back nothing would leave the
/// operator with no line at all about work that was attempted and failed.
pub(crate) async fn seerr_as_owner(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    base: String,
) -> Seerr {
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    match seerr_key(ctx, services, project.as_deref()).await {
        Some(key) => Seerr::keyed(ctx.seams.http.clone(), base, "seerr", key),
        None => Seerr::new(ctx.seams.http.clone(), base, "seerr"),
    }
}

/// What reading the household's requests needs: the request service, carrying the key
/// it answers as its owner, whose reads see every member's requests.
pub(crate) struct HouseholdAccess {
    /// The request service, reached on the host.
    pub seerr: Seerr,
}

/// The request service to read the household from, or nothing where the stack has no
/// request service, no media server for it to authenticate the household against, or a
/// request service that has not written its own key yet.
///
/// The household view treats any of those as nothing to report rather than a fault: a
/// stack without a request service has no household requests, and one not yet set up
/// has nobody to have asked for anything.
pub(crate) async fn seerr_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<HouseholdAccess> {
    let seerr = service_addr(services, lemonfiber_manifest::ApiKind::Seerr)?;
    service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let key = seerr_key(ctx, services, project.as_deref()).await?;
    Some(HouseholdAccess {
        seerr: Seerr::keyed(ctx.seams.http.clone(), seerr.loopback, "seerr", key),
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
    let text = ctx.seams.filesystem.read(&config).await?;
    let key = crate::sabnzbd::api_key(&text)?;
    Some(Sabnzbd::new(ctx.seams.http.clone(), base, key))
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
    let key = target.key(ctx.seams.filesystem.as_ref()).await?;
    Some(Prowlarr::new(
        ctx.seams.http.clone(),
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
    let key = bazarr_key(ctx, services, project).await?;
    Some(crate::bazarr::Bazarr::new(
        ctx.seams.http.clone(),
        addr.loopback,
        &addr.id,
        key,
    ))
}

/// Claim the listening server by making its first account, where nobody has.
///
/// The server gives its root account to whoever makes the first one, from anywhere on
/// the network, so an unclaimed one is anybody's. Made here with a minted password,
/// which is answered back to be recorded; nothing where the server already has an
/// account or will not answer, and nothing where the randomness to mint one is
/// unavailable.
pub(crate) async fn claim_audiobookshelf(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<String> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Audiobookshelf)?;
    let client =
        crate::audiobookshelf::Audiobookshelf::new(ctx.seams.http.clone(), addr.loopback, &addr.id);
    if client.has_account().await.ok()? {
        return None;
    }
    let fresh = crate::secret::generate(ctx.seams.random.as_ref())?;
    client
        .create_account(crate::config::AUDIOBOOKSHELF_USER, &fresh)
        .await
        .ok()?;
    Some(fresh)
}

/// Revoke the media server key filed under lemonfiber's name, where it can.
///
/// Answers whether one was revoked. Nothing where lemonfiber does not hold the admin
/// password: a server somebody else set up is one this cannot sign in to.
pub(crate) async fn revoke_jellyfin_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<bool> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let password = crate::seed::run::identity::recorded_jellyfin_password(ctx)?;
    let client = crate::jellyfin::Jellyfin::authenticated(
        ctx.seams.http.clone(),
        addr.loopback,
        &addr.id,
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    client.revoke_our_key().await.ok()
}

/// A client for the book \*arr, holding the key lemonfiber minted for it.
///
/// The key is minted where the services are started, before this one has ever run,
/// and the service adopts it from its environment then — so the recorded value is what
/// both sides hold. Nothing before that has happened.
///
/// Synchronous, unlike its neighbours: they read a file the service wrote or ask it
/// something, and this reads only what lemonfiber recorded for itself.
pub(crate) fn bindery_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<crate::bindery::Bindery> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Bindery)?;
    let key = super::recorded_secret(ctx, crate::config::BINDERY_API_KEY)?;
    Some(crate::bindery::Bindery::new(
        ctx.seams.http.clone(),
        addr.loopback,
        &addr.id,
        key,
    ))
}

/// The request service's own key, read from the settings file it writes.
///
/// Published with the rest of the stack's keys, and the key lemonfiber itself reads and
/// writes Seerr with. Nothing before Seerr is initialised, since that is the run that
/// writes one.
pub(crate) async fn seerr_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Option<String> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Seerr)?;
    let service = services.iter().find(|service| service.id == addr.id)?;
    let path = crate::app::targets::config_path(
        project?,
        service,
        service.api.as_ref().and_then(|api| api.path.as_deref()),
    )?;
    crate::seerr::api_key(&ctx.seams.filesystem.read(&path).await?)
}

/// The subtitle finder's own key, read from the configuration it writes.
///
/// Its own rather than shared with a Servarr read: the file is a YAML holding an
/// `apikey` under several sections, so which one is this service's is decided by the
/// section it sits under. Nothing where the stack has no subtitle finder, or where it
/// has not written a key yet.
pub(crate) async fn bazarr_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Option<String> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Bazarr)?;
    let service = services.iter().find(|service| service.id == addr.id)?;
    let path = crate::app::targets::config_path(
        project?,
        service,
        service.api.as_ref().and_then(|api| api.path.as_deref()),
    )?;
    crate::bazarr::api_key(&ctx.seams.filesystem.read(&path).await?)
}

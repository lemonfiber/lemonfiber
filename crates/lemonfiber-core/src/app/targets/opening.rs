//! Opening an authenticated client for a service.
//!
//! A target says where a service is and where its key is written; this reads the key and
//! hands back something that can talk. Absent where the key is not written yet, which is a
//! service still starting rather than a fault.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_manifest::ApiKind;

use crate::app::Ctx;
use crate::doctor::credentials::Target;
use crate::jellyfin::Jellyfin;
use crate::ports::service::UsenetAccounts;
use crate::prowlarr::Prowlarr;
use crate::seerr::Seerr;
use crate::servarr::Servarr;
use crate::wiring::{Filler, Fillers, Holder};

use crate::recyclarr::Kind;

use super::downloads::download_targets;
use super::filled::{spoken, Spoken};
use super::layout::{project_directory, read_owned, service_config_dir};
use super::servarr::{servarr_targets, target_for};

/// The stack's own Jellyfin as a reading client signed in as its administrator: the
/// server the decline service acts on, which it names rather than asking for whatever
/// serves identity. Nothing where the stack has none or lemonfiber holds no password
/// for it.
pub(crate) fn declined_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<Jellyfin> {
    let addr = service_addr(services, ApiKind::Jellyfin)?;
    let password =
        super::secrets::recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    Some(
        Jellyfin::authenticated(
            ctx.seams.http.clone(),
            addr.loopback,
            &addr.id,
            crate::config::JELLYFIN_ADMIN_USER,
            password,
        )
        .remembering(Arc::clone(&ctx.sessions)),
    )
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
        let Some(kind) = target.kind else {
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

/// What reading the household's requests needs: the request service, asked as its
/// owner, whose reads see every member's requests.
pub(crate) struct HouseholdAccess {
    /// The request service.
    pub requests: Arc<dyn intake::Fills>,
}

/// The request service to read the household from, or nothing where there is no media
/// server for it to authenticate the household against or [`requests_from`] finds none.
///
/// The household view treats either as nothing to report rather than a fault: a stack
/// without a request service has no household requests, and one not yet set up has
/// nobody to have asked for anything.
pub(crate) async fn household_requests(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
) -> Option<HouseholdAccess> {
    let fillers = super::media::fillers_here(ctx, manifest);
    super::media::MediaServer::of(&fillers)?;
    requests_from(ctx, &fillers).await
}

/// Whatever among `fillers` fills `request.intake`, asked over the contract where it
/// speaks it, and otherwise as the stack's own request service through
/// [`owned_requests`].
///
/// Nothing where nothing here fills it, where it speaks the contract and cannot be
/// asked over it — never then asked any other way — or where it speaks none and is not
/// the stack's own request service holding the key it wrote for itself.
pub(crate) async fn requests_from(ctx: &Ctx, fillers: &Fillers) -> Option<HouseholdAccess> {
    let (filler, _) = fillers.filling(intake::CAPABILITY)?;
    let requests: Arc<dyn intake::Fills> =
        match spoken(ctx, filler, intake::CAPABILITY, intake::MAJOR).await {
            Spoken::Over(adapter) => Arc::new(intake::Adapter(adapter)),
            Spoken::Unanswered => return None,
            Spoken::Not => Arc::new(owned_requests(ctx, filler).await?),
        };
    Some(HouseholdAccess { requests })
}

/// Where the host reaches `filler` as the stack's own request service: nothing where it
/// is a plugin's, is spoken to through another adapter, or publishes no port. The one
/// gate before the stack's own request service is asked through this build's adapter
/// for it.
pub(crate) fn bundled_requests(filler: &Filler) -> Option<String> {
    (filler.holder() == Holder::Stack && filler.speaks(ApiKind::Seerr))
        .then_some(filler.published)
        .flatten()
        .map(loopback)
}

/// The key the stack's own request service `filler` wrote for itself, read from the
/// settings beneath its own directory; nothing before it has written one.
pub(crate) async fn requests_key(ctx: &Ctx, filler: &Filler) -> Option<String> {
    crate::seerr::api_key(&credential_file(ctx, filler).await.text()?)
}

/// The stack's own request service `filler` is, carrying the key it wrote for itself;
/// nothing where [`bundled_requests`] refuses it or it has not written its key yet.
async fn owned_requests(ctx: &Ctx, filler: &Filler) -> Option<Seerr> {
    let base = bundled_requests(filler)?;
    let key = requests_key(ctx, filler).await?;
    Some(Seerr::keyed(ctx.seams.http.clone(), base, &filler.id, key))
}

/// Where the host reaches a service, and the id it runs under.
pub(crate) struct ServiceAddr {
    /// The service's compose id, which names its container.
    pub id: String,
    /// Where the host reaches it: `http://127.0.0.1:{port}`.
    pub loopback: String,
}

/// What the file a service's credential is read from holds.
///
/// Read only where it is a plain file beneath the directory its container owns: whatever
/// runs there can write that directory, and a link put where the file is expected would
/// otherwise have lemonfiber read any file on the host and hand it to the container as its
/// own credential. Absent where the service names no such file.
pub(crate) async fn credential_file(
    ctx: &Ctx,
    filler: &Filler,
) -> crate::ports::filesystem::Beneath {
    match (filler.key_file.as_deref(), filler.confined_to.as_deref()) {
        (Some(file), Some(within)) => ctx.seams.filesystem.read_beneath(file, within).await,
        _ => crate::ports::filesystem::Beneath::Absent,
    }
}

/// Why a service's credential file was refused rather than read, naming the plugin
/// that brought it where a plugin did.
pub(crate) fn escaped(filler: &Filler) -> String {
    let whose = match &filler.origin {
        crate::origin::Origin::Plugin { named } => named.as_str(),
        _ => filler.name.as_str(),
    };
    format!(
        "{whose}'s credential file is a link, leads outside the directory its container \
         owns, is not a file at all, or is too large to be one, so it was not read"
    )
}

/// Where this machine reaches a service that publishes this port.
pub(crate) fn loopback(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// The address of the one service of a given api kind, or nothing where the stack has
/// none or it publishes no port to reach it on. The single place the "find the service
/// by its kind, format where it is reached" step lives, so every caller that speaks to a
/// named service resolves it the same way rather than re-deriving the URLs.
pub(crate) fn service_addr(
    services: &[lemonfiber_manifest::Service],
    kind: ApiKind,
) -> Option<ServiceAddr> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != kind {
            return None;
        }
        let port = service.port?;
        Some(ServiceAddr {
            id: service.id.clone(),
            loopback: loopback(port),
        })
    })
}

/// The first Usenet download client among `fillers`, as a reader of the accounts behind
/// it.
///
/// Nothing where there is no Usenet client, or where the client has not written its key
/// yet — a service still starting holds nothing to report, the same skip every read here
/// makes.
pub(crate) async fn usenet_client(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn UsenetAccounts>> {
    let client: Box<dyn UsenetAccounts> = download_targets(ctx, fillers)
        .await
        .iter()
        .find_map(|target| target.usenet(ctx))?;
    Some(Arc::from(client))
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

/// Revoke the media server key filed under lemonfiber's name, where it can.
///
/// Answers whether one was revoked. Nothing where lemonfiber does not hold the admin
/// password: a server somebody else set up is one this cannot sign in to.
pub(crate) async fn revoke_jellyfin_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<bool> {
    let addr = service_addr(services, ApiKind::Jellyfin)?;
    let password = crate::seed::run::identity::recorded_jellyfin_password(ctx)?;
    let client = crate::jellyfin::Jellyfin::authenticated(
        ctx.seams.http.clone(),
        addr.loopback,
        &addr.id,
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    crate::app_keys::revoke_ours(&client).await.ok()
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
    let addr = service_addr(services, ApiKind::Seerr)?;
    let service = services.iter().find(|service| service.id == addr.id)?;
    let path = crate::app::targets::config_path(
        project?,
        service,
        service.api.as_ref().and_then(|api| api.path.as_deref()),
    )?;
    let within = service_config_dir(project?, &service.id);
    crate::seerr::api_key(&read_owned(ctx.seams.filesystem.as_ref(), &path, &within).await?)
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
    let addr = service_addr(services, ApiKind::Bazarr)?;
    let service = services.iter().find(|service| service.id == addr.id)?;
    let path = crate::app::targets::config_path(
        project?,
        service,
        service.api.as_ref().and_then(|api| api.path.as_deref()),
    )?;
    let within = service_config_dir(project?, &service.id);
    crate::bazarr::api_key(&read_owned(ctx.seams.filesystem.as_ref(), &path, &within).await?)
}

#[cfg(test)]
mod tests;

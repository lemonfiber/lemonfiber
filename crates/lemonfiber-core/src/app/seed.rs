//! Wiring the stack's services to each other, idempotently.
//!
//! One connection lemonfiber mints (qBittorrent's web UI password); the rest it
//! reads and writes. The orchestration lives here so [`super::dispatch`] stays a
//! table of one-line calls rather than carrying the whole graph.

use std::path::{Path, PathBuf};

use super::targets::{project_directory, target_for};
use super::{Ctx, Problem};
use crate::config::store;
use crate::error::Diagnose;
use crate::ports::docker::LogQuery;

/// Wire the stack's services to each other, idempotently, and report what was
/// wired and what a re-run still owes.
///
/// One connection is unlike the rest: qBittorrent's web UI password, the
/// credential lemonfiber mints rather than reads — its temporary password is
/// read from the container's log, replaced with a generated one, and the
/// generated one recorded where the forwarded-port push reads it. The rest of
/// the graph reads a credential and writes a connection: each media-filing
/// \*arr's root folders, and its download clients (`SABnzbd` and qBittorrent).
/// Prowlarr's app sync registers each of those \*arrs back into Prowlarr, so it
/// pushes them indexers. It then makes Jellyfin the identity source for Seerr, so
/// the household signs in once. Bindery wiring lands next.
pub(super) async fn seed(ctx: &Ctx) -> Result<crate::seed::Report, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    let mut wirings = Vec::new();

    // qBittorrent's password, the one credential lemonfiber mints. Collecting the
    // optional target into a list wires it where the stack has it and does nothing
    // where it does not, without a branch a test could not reach. The generated
    // value is kept to register qBittorrent as a download client below.
    let mut qbittorrent_password = None;
    for target in qbittorrent_target(&manifest.services)
        .into_iter()
        .collect::<Vec<_>>()
    {
        let (wiring, generated) = seed_qbittorrent_password(ctx, &target).await;
        qbittorrent_password = generated.or(qbittorrent_password);
        wirings.push(wiring);
    }

    // A later run mints nothing — the temporary password is long gone — so the
    // value recorded on the run that minted it stands in. Without this an \*arr
    // that came up after the first seed would never learn about qBittorrent,
    // since its password cannot be read back from qBittorrent itself.
    let qbittorrent_password = qbittorrent_password.or_else(|| recorded_qbittorrent_password(ctx));

    // Root folders and download clients for each \*arr that files media. The
    // download clients' own credentials are read once: SABnzbd's key from its
    // config, qBittorrent's the password minted or recorded above.
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let sabnzbd_key = read_sabnzbd_key(ctx, &manifest.services, project.as_deref()).await;
    for arr in servarr_arrs(&manifest.services, project.as_deref()) {
        wirings.extend(seed_root_folders(ctx, &arr).await);
        wirings.extend(
            seed_download_clients(
                ctx,
                &arr,
                sabnzbd_key.as_deref(),
                qbittorrent_password.as_deref(),
            )
            .await,
        );
    }

    // Prowlarr's app sync: register each of those media-filing \*arrs back into
    // Prowlarr, so it pushes them its indexers. Bindery is left out here — it is
    // not one of Prowlarr's applications and is wired via Torznab instead.
    wirings.extend(seed_applications(ctx, &manifest.services, project.as_deref()).await);

    // Jellyfin as Seerr's identity source: one household account, not two.
    // Jellyfin has no key to read, so its admin password is minted and recorded
    // like qBittorrent's, then Seerr is pointed at it.
    wirings.extend(seed_jellyfin_identity(ctx, &manifest.services).await);

    Ok(crate::seed::Report { wirings })
}

/// A Servarr application that files media: its identity and address (as the
/// credential check resolves them) and the media types it manages, which give
/// the root folders it needs.
struct Arr {
    target: crate::doctor::credentials::Target,
    media_types: Vec<String>,
}

/// The Servarr applications that file media — Sonarr, Radarr, Lidarr — resolved
/// with their media types. Prowlarr shares the shape but manages no media, so it
/// declares no media types and is left out.
fn servarr_arrs(services: &[lemonfiber_manifest::Service], project: Option<&Path>) -> Vec<Arr> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| {
            let target = target_for(service, project)?;
            if service.media_types.is_empty() {
                return None;
            }
            Some(Arr {
                target,
                media_types: service.media_types.clone(),
            })
        })
        .collect()
}

/// Register an application's root folders, one per media type, under
/// `/data/media`. The application's key is read from its configuration; without
/// it — the application has not finished starting — the folders are skipped for a
/// re-run rather than failed.
async fn seed_root_folders(ctx: &Ctx, arr: &Arr) -> Vec<crate::seed::Wiring> {
    let wanted: Vec<crate::ports::service::RootFolder> = arr
        .media_types
        .iter()
        .map(|media| crate::ports::service::RootFolder {
            path: format!("/data/media/{media}"),
            media_type: media.clone(),
        })
        .collect();

    let Some(key) = read_servarr_key(ctx, &arr.target.config).await else {
        return wanted
            .iter()
            .map(|folder| crate::seed::Wiring {
                connection: format!("{} root folder in {}", folder.media_type, arr.target.name),
                state: crate::seed::State::Skipped {
                    reason: format!(
                        "{} has not written its API key yet; a later run completes it",
                        arr.target.name
                    ),
                },
            })
            .collect();
    };

    let client = crate::servarr::Servarr::new(
        ctx.http.clone(),
        &arr.target.base,
        key,
        &arr.target.id,
        arr.target.version,
    );
    let mut journal = crate::journal::Journal::new();
    crate::seed::wire_root_folders(
        &client,
        &arr.target.name,
        &wanted,
        &mut journal,
        &seed_stamp(ctx),
    )
    .await
}

/// A Servarr application's API key, read from the configuration file it wrote it
/// to, or nothing where it has not written one yet.
async fn read_servarr_key(ctx: &Ctx, config: &Path) -> Option<String> {
    let text = ctx.filesystem.read(config).await?;
    crate::servarr::api_key(&text)
}

/// A timestamp for the change journal — seconds since the epoch, the clock's own
/// account of now.
fn seed_stamp(ctx: &Ctx) -> String {
    ctx.clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}

/// Where a Servarr application reaches `SABnzbd` on the stack's network: the
/// container name, and `SABnzbd`'s own listening port rather than the
/// host-published one, because the application connects across the network, not
/// through the host.
const SABNZBD_HOST: (&str, u16) = ("sabnzbd", 8080);

/// Where a Servarr application reaches qBittorrent: through Gluetun, whose network
/// namespace qBittorrent shares, on qBittorrent's web UI port. qBittorrent has no
/// network of its own, so its address is Gluetun's.
const QBITTORRENT_HOST: (&str, u16) = ("gluetun", 8081);

/// The category an application files under, named as that application names its
/// category field, for the media type it manages.
///
/// The field is fixed per application — Sonarr names it `tvCategory`, Radarr
/// `movieCategory`, Lidarr `musicCategory` — so the mapping is by the media type
/// that identifies the application. A media type lemonfiber does not recognise has
/// no known field, so it names none rather than guessing.
fn category_for(media: &str) -> Option<crate::ports::service::Category> {
    let field = match media {
        "tv" => "tvCategory",
        "movies" => "movieCategory",
        "music" => "musicCategory",
        _ => return None,
    };
    Some(crate::ports::service::Category {
        field: field.to_owned(),
        value: media.to_owned(),
    })
}

/// `SABnzbd`'s API key, read from its `sabnzbd.ini` under the project root, or
/// nothing where the stack has no `SABnzbd`, no project to read from, or
/// `SABnzbd` has not written its key yet.
async fn read_sabnzbd_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<String> {
    let path = sabnzbd_config_path(services, project?)?;
    let text = ctx.filesystem.read(&path).await?;
    crate::sabnzbd::api_key(&text)
}

/// The host path to `SABnzbd`'s configuration file, resolved the way a Servarr
/// config path is: its `/config` mount lives at `config/<id>` under the project
/// root. Nothing where the stack has no `SABnzbd` writing a config there.
fn sabnzbd_config_path(
    services: &[lemonfiber_manifest::Service],
    project: &Path,
) -> Option<PathBuf> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != lemonfiber_manifest::ApiKind::Sabnzbd {
            return None;
        }
        let inside = api.path.as_deref()?.strip_prefix("/config/")?;
        Some(project.join("config").join(&service.id).join(inside))
    })
}

/// Register into an application the download clients whose credentials are in
/// hand: `SABnzbd` where its key was read, qBittorrent where its password was
/// minted. Both carry the application's category. Without the application's own
/// key its clients are skipped for a re-run, as its root folders are.
async fn seed_download_clients(
    ctx: &Ctx,
    arr: &Arr,
    sabnzbd_key: Option<&str>,
    qbittorrent_password: Option<&str>,
) -> Vec<crate::seed::Wiring> {
    let categories: Vec<crate::ports::service::Category> = arr
        .media_types
        .first()
        .and_then(|media| category_for(media))
        .into_iter()
        .collect();

    let mut wirings = Vec::new();
    for category in &categories {
        let clients = download_clients(sabnzbd_key, qbittorrent_password, category);
        if clients.is_empty() {
            continue;
        }
        wirings.extend(wire_arr_download_clients(ctx, arr, &clients).await);
    }
    wirings
}

/// The download clients to register, one per credential that is in hand.
fn download_clients(
    sabnzbd_key: Option<&str>,
    qbittorrent_password: Option<&str>,
    category: &crate::ports::service::Category,
) -> Vec<crate::ports::service::DownloadClient> {
    let mut clients = Vec::new();
    if let Some(key) = sabnzbd_key {
        clients.push(crate::ports::service::DownloadClient {
            name: "SABnzbd".to_owned(),
            host: SABNZBD_HOST.0.to_owned(),
            port: SABNZBD_HOST.1,
            kind: crate::ports::service::ClientKind::Sabnzbd,
            credential: crate::ports::service::Credential::ApiKey(key.to_owned()),
            category: category.clone(),
        });
    }
    if let Some(password) = qbittorrent_password {
        clients.push(crate::ports::service::DownloadClient {
            name: "qBittorrent".to_owned(),
            host: QBITTORRENT_HOST.0.to_owned(),
            port: QBITTORRENT_HOST.1,
            kind: crate::ports::service::ClientKind::Qbittorrent,
            credential: crate::ports::service::Credential::UserPass {
                username: "admin".to_owned(),
                password: password.to_owned(),
            },
            category: category.clone(),
        });
    }
    clients
}

/// Wire the given clients into one application, reading its key first; without it
/// the application has not finished starting and the clients are skipped.
async fn wire_arr_download_clients(
    ctx: &Ctx,
    arr: &Arr,
    clients: &[crate::ports::service::DownloadClient],
) -> Vec<crate::seed::Wiring> {
    let Some(key) = read_servarr_key(ctx, &arr.target.config).await else {
        return clients
            .iter()
            .map(|client| crate::seed::Wiring {
                connection: format!("{} into {}", client.name, arr.target.name),
                state: crate::seed::State::Skipped {
                    reason: format!(
                        "{} has not written its API key yet; a later run completes it",
                        arr.target.name
                    ),
                },
            })
            .collect();
    };
    let servarr = crate::servarr::Servarr::new(
        ctx.http.clone(),
        &arr.target.base,
        key,
        &arr.target.id,
        arr.target.version,
    );
    let mut journal = crate::journal::Journal::new();
    crate::seed::wire_download_clients(
        &servarr,
        &arr.target.name,
        clients,
        &mut journal,
        &seed_stamp(ctx),
    )
    .await
}

/// Prowlarr as the app-sync source, and the media-filing \*arrs to register into
/// it — the resolution app sync starts from.
struct AppSyncSource {
    /// Prowlarr itself: where to reach its API and read its key.
    target: crate::doctor::credentials::Target,
    /// The address the \*arrs reach Prowlarr back on, on the stack's network.
    network_url: String,
}

/// One media-filing \*arr to register into Prowlarr: what to call it, which
/// application it is, where to read its key, and where Prowlarr reaches it.
struct SyncableArr {
    name: String,
    kind: crate::ports::service::ApplicationKind,
    config: PathBuf,
    network_url: String,
}

/// Register each media-filing \*arr as an application in Prowlarr, so Prowlarr
/// pushes it indexers.
///
/// Two keys gate a write, both read from configuration and never asked for:
/// Prowlarr's own — without it Prowlarr is still starting, so every application
/// is skipped for a re-run — and each \*arr's, which is what lets Prowlarr write
/// into that \*arr; an \*arr that has not written its key yet is skipped on its
/// own while the others proceed.
async fn seed_applications(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<crate::seed::Wiring> {
    let Some(source) = prowlarr_source(services, project) else {
        return Vec::new();
    };

    let Some(prowlarr_key) = read_servarr_key(ctx, &source.target.config).await else {
        return syncable_arrs(services, project)
            .into_iter()
            .map(|arr| skipped_application(&arr.name, &source.target.name))
            .collect();
    };

    let mut wanted = Vec::new();
    let mut skipped = Vec::new();
    for arr in syncable_arrs(services, project) {
        match read_servarr_key(ctx, &arr.config).await {
            Some(key) => wanted.push(crate::ports::service::Application {
                name: arr.name,
                kind: arr.kind,
                prowlarr_url: source.network_url.clone(),
                base_url: arr.network_url,
                api_key: key,
            }),
            None => skipped.push(skipped_application(&arr.name, &source.target.name)),
        }
    }

    let client = crate::prowlarr::Prowlarr::new(
        ctx.http.clone(),
        &source.target.base,
        prowlarr_key,
        &source.target.id,
    );
    let mut journal = crate::journal::Journal::new();
    let mut wirings = crate::seed::wire_applications(
        &client,
        &source.target.name,
        &wanted,
        &mut journal,
        &seed_stamp(ctx),
    )
    .await;
    wirings.extend(skipped);
    wirings
}

/// Prowlarr as the app-sync source: the Servarr-shape service that manages no
/// media, reached at its published port and known on the network by its own
/// container name. Nothing where the stack has no such service or no project to
/// read its key from.
fn prowlarr_source(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<AppSyncSource> {
    let project = project?;
    services.iter().find_map(|service| {
        if !service.media_types.is_empty() {
            return None;
        }
        // The target's api version is required for it to resolve, but Prowlarr's
        // own client fixes v1, so it is the config path here that is load-bearing,
        // not the version the target carries.
        let target = target_for(service, project)?;
        let port = service.port?;
        Some(AppSyncSource {
            target,
            network_url: format!("http://{}:{port}", service.id),
        })
    })
}

/// The media-filing \*arrs Prowlarr's app sync covers, each as the application it
/// is and where Prowlarr reaches it on the network. A Servarr service whose media
/// is not one app sync covers — or none — is left out rather than guessed at.
fn syncable_arrs(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<SyncableArr> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| {
            let target = target_for(service, project)?;
            let kind = application_kind(&service.media_types)?;
            let port = service.port?;
            Some(SyncableArr {
                name: service.name.clone(),
                kind,
                config: target.config,
                network_url: format!("http://{}:{port}", service.id),
            })
        })
        .collect()
}

/// The Prowlarr application kind for an \*arr's media, or nothing where its media
/// is not one Prowlarr's app sync covers — the same by-media mapping the download
/// clients' categories use, so the two stay in step.
fn application_kind(media_types: &[String]) -> Option<crate::ports::service::ApplicationKind> {
    match media_types.first().map(String::as_str) {
        Some("tv") => Some(crate::ports::service::ApplicationKind::Sonarr),
        Some("movies") => Some(crate::ports::service::ApplicationKind::Radarr),
        Some("music") => Some(crate::ports::service::ApplicationKind::Lidarr),
        _ => None,
    }
}

/// An application skipped for a re-run because a key it needs is not written yet,
/// named as the driver names it so a re-run's report reads consistently.
fn skipped_application(arr: &str, prowlarr: &str) -> crate::seed::Wiring {
    crate::seed::Wiring {
        connection: format!("{arr} indexer sync via {prowlarr}"),
        state: crate::seed::State::Skipped {
            reason: format!("{arr} has not written its API key yet; a later run completes it"),
        },
    }
}

/// Jellyfin's addresses: where the host reaches its setup, and where Seerr reaches
/// it across the stack's own network.
struct JellyfinAddr {
    loopback: String,
    network_url: String,
}

/// Make Jellyfin the identity source for Seerr, so the household signs in once.
///
/// Both must be in the stack; without either there is nothing to wire. Jellyfin's
/// admin password is the one credential minted rather than read — recorded on the
/// run that mints it and read back on a later run — so the driver is given what
/// was recorded and hands back a freshly minted one for the surface to record.
async fn seed_jellyfin_identity(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Vec<crate::seed::Wiring> {
    let (Some(seerr_base), Some(jellyfin)) = (seerr_service(services), jellyfin_service(services))
    else {
        return Vec::new();
    };

    let jellyfin_client =
        crate::jellyfin::Jellyfin::new(ctx.http.clone(), &jellyfin.loopback, "jellyfin");
    let seerr_client = crate::seerr::Seerr::new(ctx.http.clone(), &seerr_base, "seerr");
    let recorded = recorded_jellyfin_password(ctx);

    let (wiring, minted) = crate::seed::wire_jellyfin_identity(
        &jellyfin_client,
        &seerr_client,
        ctx.random.as_ref(),
        recorded.as_deref(),
        &jellyfin.network_url,
    )
    .await;

    if let Some(password) = &minted {
        record_jellyfin_password(ctx, password);
    }
    vec![wiring]
}

/// Where the host reaches Seerr's API, if the stack has it — selected by its api
/// kind, the way every service lemonfiber speaks to is.
fn seerr_service(services: &[lemonfiber_manifest::Service]) -> Option<String> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != lemonfiber_manifest::ApiKind::Seerr {
            return None;
        }
        let port = service.port?;
        Some(format!("http://127.0.0.1:{port}"))
    })
}

/// Jellyfin's addresses, if the stack has it — selected by its api kind, the way
/// every service lemonfiber speaks to is. Jellyfin's kind carries no key source
/// of the usual sort: it is the one service lemonfiber sets an account on rather
/// than reading a key from, so its password is generated.
fn jellyfin_service(services: &[lemonfiber_manifest::Service]) -> Option<JellyfinAddr> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != lemonfiber_manifest::ApiKind::Jellyfin {
            return None;
        }
        let port = service.port?;
        Some(JellyfinAddr {
            loopback: format!("http://127.0.0.1:{port}"),
            network_url: format!("http://{}:{port}", service.id),
        })
    })
}

/// The Jellyfin admin password recorded on the run that minted it, so a later run
/// can point Seerr at Jellyfin without minting again. Nothing where no env file
/// holds one; an unreadable file reads the same as an empty one.
fn recorded_jellyfin_password(ctx: &Ctx) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    let value = file.get(crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// Record the minted Jellyfin admin password where a later run reads it back.
/// Best-effort: a value that could not be written is reported by the next run
/// re-minting rather than by failing the wiring that did land.
fn record_jellyfin_password(ctx: &Ctx, password: &str) {
    if let Some(path) = ctx.settings.env_file.as_deref() {
        let _ = store::set(path, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY, password);
    }
}

/// qBittorrent's address, if the stack has it: the id names the container to read
/// a log from, the base is where the host reaches its web UI.
fn qbittorrent_target(services: &[lemonfiber_manifest::Service]) -> Option<(String, String)> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        (api.kind == lemonfiber_manifest::ApiKind::Qbittorrent).then(|| {
            let port = service.port.unwrap_or(0);
            (service.id.clone(), format!("http://127.0.0.1:{port}"))
        })
    })
}

/// Replace qBittorrent's temporary web UI password and record the generated one.
///
/// The temporary password is read from the container's own log; without it there
/// is nothing to authenticate with, so the connection is skipped for a re-run
/// once the container has announced one. A generated password that lands is
/// recorded in the environment where the forwarded-port push reads it.
async fn seed_qbittorrent_password(
    ctx: &Ctx,
    target: &(String, String),
) -> (crate::seed::Wiring, Option<String>) {
    let (id, base) = target;
    let connection = "qBittorrent web UI password".to_owned();

    let Some(temporary) = read_temporary_password(ctx, id).await else {
        let wiring = crate::seed::Wiring {
            connection,
            state: crate::seed::State::Skipped {
                reason: "qBittorrent has not announced a temporary password yet; a later run completes it".to_owned(),
            },
        };
        return (wiring, None);
    };

    let client = crate::qbittorrent::Qbittorrent::new(ctx.http.clone(), base);
    let (wiring, recorded) =
        crate::seed::wire_qbittorrent_password(&client, ctx.random.as_ref(), &temporary).await;

    if let Some(password) = &recorded {
        record_qbittorrent_password(ctx, password);
    }
    (wiring, recorded)
}

/// The temporary password qBittorrent announced in its log, if it has.
async fn read_temporary_password(ctx: &Ctx, service: &str) -> Option<String> {
    let mut lines = ctx
        .engine
        .logs(
            &ctx.settings.project,
            &[service.to_owned()],
            LogQuery::recent(TEMP_PASSWORD_LOG_LINES),
        )
        .await
        .ok()?;

    let mut log = String::new();
    while let Some(line) = lines.recv().await {
        log.push_str(&line.line);
        log.push('\n');
    }
    crate::qbittorrent::temporary_password(&log)
}

/// Record the generated password where the forwarded-port push reads it — the
/// `QBITTORRENT_PASSWORD` setting in the environment file. Best-effort: a value
/// that could not be written is reported by the push's own missing-password
/// message rather than failing the wiring that did land.
fn record_qbittorrent_password(ctx: &Ctx, password: &str) {
    if let Some(path) = ctx.settings.env_file.as_deref() {
        let _ = store::set(path, crate::config::QBITTORRENT_PASSWORD_KEY, password);
    }
}

/// The qBittorrent password recorded on the run that minted it, so a later run
/// still offers qBittorrent as a download client. Nothing where no env file
/// holds one yet; an unreadable file reads the same as an empty one, because a
/// missing password is a missing password however the reading failed.
fn recorded_qbittorrent_password(ctx: &Ctx) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    let value = file.get(crate::config::QBITTORRENT_PASSWORD_KEY)?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// How many lines back to read for qBittorrent's start-up announcement. Its
/// temporary password is printed once, early, so a generous tail finds it well
/// after start without pulling the whole log.
const TEMP_PASSWORD_LOG_LINES: u32 = 200;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        application_kind, category_for, download_clients, prowlarr_source, read_sabnzbd_key,
        recorded_qbittorrent_password, sabnzbd_config_path, servarr_arrs, syncable_arrs,
    };
    use crate::app::targets::{project_directory, servarr_targets};
    use crate::app::{dispatch, Command, Ctx, Outcome};
    use crate::config::{store, Settings};
    use crate::model::VersionReport;
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::stack::Source;
    use crate::test_support::{
        spoke, stack, FixedRandom, Reporting, RoutedHttp, Scripted, ScriptedHttp, SeedFs,
    };

    /// A manifest service with the few fields the credential resolver reads, and
    /// filler for the rest, so a test can vary the shape, port and key file.
    fn manifest_service(
        id: &str,
        api: Option<lemonfiber_manifest::Api>,
        port: Option<u16>,
    ) -> lemonfiber_manifest::Service {
        lemonfiber_manifest::Service {
            id: id.to_owned(),
            name: format!("{id} the app"),
            profile: "media".to_owned(),
            image: "example/image".to_owned(),
            tag: "1".to_owned(),
            port,
            bind: None,
            health: None,
            api,
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

    /// A Servarr-shape API declaration naming the given key file, or none, at the
    /// v3 most tests need; the version-specific tests set their own.
    fn servarr_api(path: Option<&str>) -> lemonfiber_manifest::Api {
        servarr_api_at(path, Some(3))
    }

    /// The same, at a given API version — `None` to omit it entirely.
    fn servarr_api_at(path: Option<&str>, version: Option<u32>) -> lemonfiber_manifest::Api {
        lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Servarr,
            key_source: lemonfiber_manifest::KeySource::ConfigXml,
            path: path.map(str::to_owned),
            version,
        }
    }

    #[test]
    fn the_project_directory_is_the_external_path_or_the_materialise_target() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/ports");
        assert_eq!(
            project_directory(&Source::External(std::path::Path::new("/srv/stack")), None)
                .as_deref(),
            Some(std::path::Path::new("/srv/stack")),
            "an external stack is its own project root"
        );
        assert_eq!(
            project_directory(
                &Source::Embedded(&EMBEDDED),
                Some(std::path::Path::new("/opt/lemonfiber/stack"))
            )
            .as_deref(),
            Some(std::path::Path::new("/opt/lemonfiber/stack")),
            "an embedded stack's root is wherever it was materialised"
        );
        assert_eq!(
            project_directory(&Source::Embedded(&EMBEDDED), None),
            None,
            "an embedded stack materialised nowhere has no root to read from"
        );
    }

    #[test]
    fn only_reachable_servarr_services_with_a_config_path_become_targets() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![
            manifest_service(
                "sonarr",
                Some(servarr_api(Some("/config/config.xml"))),
                Some(8989),
            ),
            manifest_service(
                "sabnzbd",
                Some(lemonfiber_manifest::Api {
                    kind: lemonfiber_manifest::ApiKind::Sabnzbd,
                    key_source: lemonfiber_manifest::KeySource::ConfigIni,
                    path: Some("/config/sabnzbd.ini".to_owned()),
                    version: None,
                }),
                Some(8080),
            ),
            manifest_service("jellyfin", None, Some(8096)),
            manifest_service(
                "radarr",
                Some(servarr_api(Some("/config/config.xml"))),
                None,
            ),
            manifest_service(
                "lidarr",
                Some(servarr_api(Some("/data/elsewhere.xml"))),
                Some(8686),
            ),
            manifest_service("prowlarr", Some(servarr_api(None)), Some(9696)),
        ];

        let targets = servarr_targets(&services, Some(project));

        assert_eq!(
            targets.len(),
            1,
            "only the reachable Servarr service qualifies"
        );
        let target = targets.first();
        assert!(
            target.is_some_and(|target| target.id == "sonarr"
                && target.base == "http://127.0.0.1:8989"
                && target.config == project.join("config/sonarr/config.xml")),
            "the key is read from where Compose mounts the service's config"
        );
    }

    #[test]
    fn a_sabnzbd_config_path_is_the_config_mount_of_the_one_sabnzbd_service() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let sabnzbd_api = |path: Option<&str>| lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: path.map(str::to_owned),
            version: None,
        };
        let services = vec![
            manifest_service("jellyfin", None, Some(8096)),
            manifest_service(
                "sonarr",
                Some(servarr_api(Some("/config/config.xml"))),
                Some(8989),
            ),
            manifest_service(
                "sabnzbd",
                Some(sabnzbd_api(Some("/config/sabnzbd.ini"))),
                Some(8080),
            ),
        ];

        assert_eq!(
            sabnzbd_config_path(&services, project),
            Some(project.join("config/sabnzbd/sabnzbd.ini")),
            "read from where Compose mounts SABnzbd's config"
        );
        assert!(
            sabnzbd_config_path(&[], project).is_none(),
            "no SABnzbd service, no path"
        );
        assert!(
            sabnzbd_config_path(
                &[manifest_service(
                    "sabnzbd",
                    Some(sabnzbd_api(None)),
                    Some(8080)
                )],
                project
            )
            .is_none(),
            "a SABnzbd that declares no config file"
        );
        assert!(
            sabnzbd_config_path(
                &[manifest_service(
                    "sabnzbd",
                    Some(sabnzbd_api(Some("/data/elsewhere.ini"))),
                    Some(8080)
                )],
                project
            )
            .is_none(),
            "a config path outside the /config mount"
        );
    }

    #[tokio::test]
    async fn a_sabnzbd_key_needs_a_project_and_a_sabnzbd_to_read() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        assert!(
            read_sabnzbd_key(&ctx, &[], None).await.is_none(),
            "without a project there is nowhere to read from"
        );
        assert!(
            read_sabnzbd_key(&ctx, &[], Some(std::path::Path::new("/srv/stack")))
                .await
                .is_none(),
            "without a SABnzbd service there is no key"
        );
    }

    #[test]
    fn nothing_can_be_proven_without_a_project_directory() {
        let services = vec![manifest_service(
            "sonarr",
            Some(servarr_api(Some("/config/config.xml"))),
            Some(8989),
        )];
        assert!(servarr_targets(&services, None).is_empty());
    }

    /// The seed report an outcome carried, if it was a seed outcome.
    fn seeded(outcome: Result<Outcome, super::Problem>) -> Option<crate::seed::Report> {
        match outcome {
            Ok(Outcome::Seed(report)) => Some(report),
            _ => None,
        }
    }

    /// Whether a wiring was skipped, on one line so it holds no phantom coverage.
    fn is_skipped(wiring: &crate::seed::Wiring) -> bool {
        matches!(wiring.state, crate::seed::State::Skipped { .. })
    }

    /// Whether a wiring failed, on one line so it holds no phantom coverage.
    fn is_failed(wiring: &crate::seed::Wiring) -> bool {
        matches!(wiring.state, crate::seed::State::Failed { .. })
    }

    /// A context whose engine says the given qBittorrent log line, answering
    /// seeding's HTTP from `replies` and its randomness from `bytes`.
    fn seed_ctx(
        log: Option<&str>,
        reachable: bool,
        replies: Vec<(u16, &'static str)>,
        bytes: Option<Vec<u8>>,
        env: Option<std::path::PathBuf>,
    ) -> Ctx {
        let mut engine = if reachable {
            Reporting::holding(&["qbittorrent"], Lifecycle::Running, Health::Healthy)
        } else {
            Reporting::absent()
        };
        if let Some(line) = log {
            engine = engine.saying("qbittorrent", line);
        }
        let settings = Settings {
            env_file: env,
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(engine),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .with_http(Arc::new(ScriptedHttp::new(replies)))
        .with_random(Arc::new(FixedRandom(bytes)))
    }

    /// The three replies a full password exchange expects: log in, set, confirm.
    fn exchange() -> Vec<(u16, &'static str)> {
        vec![(200, "Ok."), (200, ""), (200, "Ok.")]
    }

    /// The line qBittorrent logs its temporary password on.
    const TEMP_LOG: &str = "A temporary password is provided for this session: read-from-log";

    fn config_scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    #[test]
    fn a_non_seed_outcome_carries_no_seed_report() {
        let version = Outcome::Version(VersionReport {
            binary: env!("CARGO_PKG_VERSION").to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
        });
        assert!(seeded(Ok(version)).is_none());
    }

    #[tokio::test]
    async fn seed_replaces_and_records_the_qbittorrent_password() {
        let env = config_scratch("seed-records");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
        let ctx = seed_ctx(
            Some(TEMP_LOG),
            true,
            exchange(),
            Some(vec![0x11; 24]),
            Some(env.clone()),
        );

        let outcome = dispatch(Command::Seed, &ctx).await;

        let json = outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.clone().envelope().to_json());
        assert!(json
            .as_deref()
            .is_some_and(|json| json.contains(r#""kind":"seed""#)));

        let report = seeded(outcome).unwrap_or_default();
        let wired = report
            .wirings
            .iter()
            .any(|wiring| wiring.state == crate::seed::State::Wired);
        assert!(wired, "the password is wired");

        let written = std::fs::read_to_string(&env).unwrap_or_default();
        assert!(written.contains("QBITTORRENT_PASSWORD="));
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn seed_sets_the_password_even_with_nowhere_to_record_it() {
        let ctx = seed_ctx(Some(TEMP_LOG), true, exchange(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let wired = report
            .wirings
            .iter()
            .any(|wiring| wiring.state == crate::seed::State::Wired);
        assert!(wired, "the password is set even with nowhere to record it");
    }

    #[tokio::test]
    async fn seed_reports_an_unreadable_stack_rather_than_guessing() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let outcome = dispatch(Command::Seed, &ctx).await;
        assert_eq!(
            outcome.err().map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn seed_reports_a_failed_password_change_and_records_nothing() {
        // The temporary password is read but the client rejects it, so the change
        // fails and there is no generated value to record.
        let ctx = seed_ctx(
            Some(TEMP_LOG),
            true,
            vec![(200, "Fails.")],
            Some(vec![0x11; 24]),
            None,
        );
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let failed = report.wirings.iter().any(is_failed);
        assert!(failed, "a rejected change is reported as failed");
    }

    #[tokio::test]
    async fn seed_skips_qbittorrent_when_no_password_is_announced() {
        let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let all_skipped = report.wirings.iter().all(is_skipped);
        assert!(
            all_skipped,
            "an unannounced password is skipped, not failed"
        );
    }

    #[tokio::test]
    async fn seed_skips_qbittorrent_when_its_log_cannot_be_read() {
        let ctx = seed_ctx(None, false, Vec::new(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let all_skipped = report.wirings.iter().all(is_skipped);
        assert!(all_skipped, "an unreadable log is skipped, not failed");
    }

    /// The wirings whose connection names a root folder.
    fn root_folder_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("root folder"))
            .collect()
    }

    #[tokio::test]
    async fn seed_wires_each_arrs_root_folders() {
        // Each application already holds the folders, so each is left as wired.
        // qBittorrent announces no password, so the only calls are the arrs'.
        const FOLDERS: &str = r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#;
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let ctx = seed_ctx(
            None,
            true,
            vec![(200, FOLDERS), (200, FOLDERS), (200, FOLDERS)],
            Some(vec![0x11; 24]),
            None,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let folders = root_folder_wirings(&report);
        assert_eq!(folders.len(), 3, "one root folder per media-filing arr");
        let all_wired = folders
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired);
        assert!(all_wired, "a folder already present is left wired");
    }

    #[tokio::test]
    async fn seed_skips_arr_root_folders_when_the_key_is_not_readable() {
        // No configuration to read a key from, so the arrs have not finished
        // starting: their folders are skipped for a re-run, not failed.
        let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let folders = root_folder_wirings(&report);
        assert_eq!(folders.len(), 3);
        assert!(folders.iter().all(|wiring| is_skipped(wiring)));
    }

    #[test]
    fn no_project_directory_means_no_arrs_to_wire() {
        assert!(servarr_arrs(&[], None).is_empty());
    }

    #[tokio::test]
    async fn the_read_only_filesystem_fake_is_inert_elsewhere() {
        // The fake answers only `read`; the rest are stubs, exercised here so the
        // fake carries no uncovered lines of its own.
        use crate::ports::filesystem::FileSystem;
        let fs = SeedFs::keyed(None, None);
        let path = std::path::Path::new("/x");
        assert!(fs.canonicalize(path).await.is_ok());
        assert!(fs.touch(path).await.is_err());
        assert!(fs.link(path, path).await.is_err());
        assert!(fs.identify(path).await.is_err());
        fs.remove(path).await;
        assert!(fs.read(path).await.is_none());
        fs.write(path, "unused").await;
        assert!(fs.ownership(path).await.is_none());
        let _ = fs.describe(path).await;
    }

    #[test]
    fn a_category_is_named_by_the_media_the_application_files() {
        assert_eq!(
            category_for("tv").map(|category| category.field),
            Some("tvCategory".to_owned())
        );
        assert_eq!(
            category_for("movies").map(|category| category.field),
            Some("movieCategory".to_owned())
        );
        assert_eq!(
            category_for("music").map(|category| category.field),
            Some("musicCategory".to_owned())
        );
        assert!(category_for("comics").is_none());
    }

    #[test]
    fn a_download_client_is_built_for_each_credential_in_hand() {
        let category = crate::ports::service::Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        };
        assert_eq!(download_clients(Some("k"), Some("p"), &category).len(), 2);
        assert_eq!(download_clients(Some("k"), None, &category).len(), 1);
        assert_eq!(download_clients(None, Some("p"), &category).len(), 1);
        assert!(download_clients(None, None, &category).is_empty());
    }

    #[test]
    fn a_recorded_qbittorrent_password_is_read_back_or_read_as_absent() {
        // Nowhere to read from.
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        assert!(recorded_qbittorrent_password(&ctx).is_none());

        let path = config_scratch("qbt-readback");
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.clone()));
        // A file that holds no password of ours.
        let _ = store::set(&path, "SOMETHING_ELSE", "x");
        assert!(
            recorded_qbittorrent_password(&ctx).is_none(),
            "no password recorded"
        );
        // An empty value is not a password.
        let _ = store::set(&path, crate::config::QBITTORRENT_PASSWORD_KEY, "");
        assert!(
            recorded_qbittorrent_password(&ctx).is_none(),
            "an empty value is absent"
        );
        // The value recorded on an earlier run is handed back.
        let _ = store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            "minted-earlier",
        );
        assert_eq!(
            recorded_qbittorrent_password(&ctx).as_deref(),
            Some("minted-earlier")
        );
    }

    /// The wirings whose connection registers a download client into an arr.
    fn download_client_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("into "))
            .collect()
    }

    #[tokio::test]
    async fn seed_wires_each_arrs_download_clients() {
        // qBittorrent announces a temporary password, so it is set and its value
        // threaded to the download clients; each arr already holds both clients.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(
            clients.len(),
            6,
            "SABnzbd and qBittorrent into each of three arrs"
        );
        let all_wired = clients
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired);
        assert!(all_wired, "a client already registered is left wired");
    }

    #[tokio::test]
    async fn seed_skips_download_clients_when_the_arr_key_is_not_readable() {
        // The clients' own credentials are in hand, but the arrs have not written
        // their keys, so registration is skipped for a re-run rather than failed.
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(clients.len(), 6);
        assert!(clients.iter().all(|wiring| is_skipped(wiring)));
    }

    #[tokio::test]
    async fn a_later_seed_offers_qbittorrent_from_its_recorded_password() {
        // The temporary password is gone, so nothing is minted this run; the
        // password recorded earlier stands in and qBittorrent is offered anyway.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let path = config_scratch("qbt-later-seed");
        let _ = store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            "minted-earlier",
        );
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(path))
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(clients.len(), 6, "both clients into each of three arrs");
        let qbittorrent = clients
            .iter()
            .filter(|wiring| wiring.connection.starts_with("qBittorrent into "))
            .count();
        assert_eq!(
            qbittorrent, 3,
            "qBittorrent is offered to every arr on a later run"
        );
    }

    // ---- Prowlarr app sync: register each media-filing arr back into Prowlarr. ----

    /// A media-filing \*arr as a manifest service, with the media that makes it
    /// syncable — `manifest_service` alone leaves the media empty, which is what
    /// marks Prowlarr.
    fn arr(id: &str, port: u16, media: &str) -> lemonfiber_manifest::Service {
        let mut service = manifest_service(
            id,
            Some(servarr_api(Some("/config/config.xml"))),
            Some(port),
        );
        service.media_types = vec![media.to_owned()];
        service
    }

    /// Prowlarr as a manifest service: a Servarr shape that files no media.
    fn prowlarr() -> lemonfiber_manifest::Service {
        manifest_service(
            "prowlarr",
            Some(servarr_api(Some("/config/config.xml"))),
            Some(9696),
        )
    }

    /// The wirings whose connection registers an \*arr into Prowlarr's app sync.
    fn application_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("indexer sync via"))
            .collect()
    }

    #[test]
    fn the_application_kind_follows_from_the_media() {
        use crate::ports::service::ApplicationKind;
        assert_eq!(
            application_kind(&["tv".to_owned()]),
            Some(ApplicationKind::Sonarr)
        );
        assert_eq!(
            application_kind(&["movies".to_owned()]),
            Some(ApplicationKind::Radarr)
        );
        assert_eq!(
            application_kind(&["music".to_owned()]),
            Some(ApplicationKind::Lidarr)
        );
        // Bindery files books but is not one of Prowlarr's applications.
        assert!(application_kind(&["books".to_owned()]).is_none());
        // A service that files no media is not an application at all.
        assert!(application_kind(&[]).is_none());
    }

    #[test]
    fn prowlarr_is_the_servarr_service_that_files_no_media() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // A media-filing *arr is never the source, however reachable.
        assert!(prowlarr_source(&[arr("sonarr", 8989, "tv")], Some(project)).is_none());
        // The Servarr service with no media is, known on the network by its own
        // container name and port.
        let source = prowlarr_source(&[prowlarr()], Some(project));
        assert!(source
            .is_some_and(|source| source.network_url == "http://prowlarr:9696"
                && source.target.id == "prowlarr"));
        // Without a project there is nowhere to read a key from.
        assert!(prowlarr_source(&[prowlarr()], None).is_none());
    }

    #[test]
    fn no_project_directory_means_no_arrs_to_sync() {
        assert!(syncable_arrs(&[arr("sonarr", 8989, "tv")], None).is_empty());
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let arrs = syncable_arrs(
            &[arr("sonarr", 8989, "tv"), arr("radarr", 7878, "movies")],
            Some(project),
        );
        assert_eq!(arrs.len(), 2, "each media-filing arr is syncable");
        assert!(arrs
            .iter()
            .any(|arr| arr.network_url == "http://sonarr:8989"));
    }

    #[tokio::test]
    async fn app_sync_does_nothing_where_the_stack_has_no_prowlarr() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // Only a media-filing arr, so there is no app-sync source at all.
        let wirings =
            super::seed_applications(&ctx, &[arr("sonarr", 8989, "tv")], Some(project)).await;
        assert!(wirings.is_empty(), "no Prowlarr, no app sync");
    }

    #[tokio::test]
    async fn app_sync_skips_every_arr_until_prowlarr_has_written_its_key() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr's key is not readable yet, so it is still starting: every
        // application is skipped for a re-run rather than failed.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert!(wirings.iter().all(is_skipped));
    }

    #[tokio::test]
    async fn app_sync_skips_only_the_arr_that_has_not_written_its_key() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr's key is readable but Sonarr's is not — Sonarr came up after
        // Prowlarr — so Sonarr's application waits while Prowlarr itself proceeds.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(
                SeedFs::keyed(Some(SERVARR), None).only_for_prowlarr(),
            ));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert!(wirings.iter().all(is_skipped));
    }

    #[tokio::test]
    async fn app_sync_registers_an_arr_whose_keys_are_all_readable() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // RoutedHttp reports Sonarr already registered — its baseUrl is in the
        // application list — so the connection reads back as already wired.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::AlreadyWired)
        );
    }

    /// A Prowlarr transport that starts with no applications, captures the POST
    /// that registers one, and reports it on the next read — so the orchestrator's
    /// write path runs end to end rather than short-circuiting to already-wired.
    struct CapturingProwlarr {
        posted: std::sync::Mutex<Option<crate::ports::http::Request>>,
    }

    #[async_trait::async_trait]
    impl crate::ports::http::Http for CapturingProwlarr {
        async fn send(
            &self,
            request: &crate::ports::http::Request,
        ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
            if request.method == crate::ports::http::Method::Post {
                if let Ok(mut posted) = self.posted.lock() {
                    *posted = Some(request.clone());
                }
                return Ok(crate::ports::http::Response {
                    status: 201,
                    body: String::new(),
                });
            }
            let registered = self.posted.lock().is_ok_and(|posted| posted.is_some());
            let body = if registered {
                r#"[{"id":9,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]}]"#
            } else {
                "[]"
            };
            Ok(crate::ports::http::Response {
                status: 200,
                body: body.to_owned(),
            })
        }
    }

    #[tokio::test]
    async fn app_sync_registers_an_absent_arr_and_reads_it_back() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr holds no applications, so Sonarr is genuinely written and then
        // read back — the write path a pre-populated list would hide.
        let http = Arc::new(CapturingProwlarr {
            posted: std::sync::Mutex::new(None),
        });
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(http.clone())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));

        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::Wired),
            "an absent application is written and confirmed by read-back"
        );

        // The orchestrator built the registration for the right *arr, reaching it
        // and Prowlarr on the stack network, and posted it to Prowlarr's v1 API.
        let posted = http.posted.lock().ok().and_then(|guard| guard.clone());
        assert!(posted
            .as_ref()
            .is_some_and(|request| request.url.ends_with("/api/v1/applications")));
        let body = posted.and_then(|request| request.body).unwrap_or_default();
        assert!(
            body.contains("http://sonarr:8989"),
            "the *arr's address: {body}"
        );
        assert!(body.contains(r#""implementation":"Sonarr""#), "{body}");
        assert!(
            body.contains("http://prowlarr:9696"),
            "Prowlarr's callback url: {body}"
        );
    }

    #[tokio::test]
    async fn seed_registers_each_arr_into_prowlarr() {
        // The whole command against the real manifest: Prowlarr and the three
        // media-filing arrs, each already registered per RoutedHttp.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(Arc::new(RoutedHttp))
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let applications = application_wirings(&report);
        assert_eq!(
            applications.len(),
            3,
            "Sonarr, Radarr and Lidarr each registered into Prowlarr"
        );
        assert!(applications
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired));
    }

    // ---- Jellyfin as Seerr's identity: two services and a minted credential. ----

    /// A Seerr-shape service declaration.
    fn seerr_api() -> lemonfiber_manifest::Api {
        lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Seerr,
            key_source: lemonfiber_manifest::KeySource::ApiSettings,
            path: None,
            version: None,
        }
    }

    fn seerr_svc() -> lemonfiber_manifest::Service {
        manifest_service("seerr", Some(seerr_api()), Some(5055))
    }

    fn jellyfin_api() -> lemonfiber_manifest::Api {
        lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Jellyfin,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }
    }

    fn jellyfin_svc() -> lemonfiber_manifest::Service {
        manifest_service("jellyfin", Some(jellyfin_api()), Some(8096))
    }

    /// A transport standing in for the household pair, routed by path: Jellyfin's
    /// public info reports whether its wizard has run, its `/Startup/*` calls
    /// succeed, Seerr's sign-in flips it to initialised, and its public settings
    /// report that state.
    struct HouseholdHttp {
        completed: bool,
        signed_in: std::sync::Mutex<bool>,
    }

    #[async_trait::async_trait]
    impl crate::ports::http::Http for HouseholdHttp {
        async fn send(
            &self,
            request: &crate::ports::http::Request,
        ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
            let url = &request.url;
            let body = if url.contains("/System/Info/Public") {
                format!(r#"{{"StartupWizardCompleted":{}}}"#, self.completed)
            } else if url.contains("/Startup/") || url.contains("/auth/jellyfin") {
                // Jellyfin's setup calls and Seerr's sign-in succeed but neither
                // by itself finishes Seerr's setup.
                String::new()
            } else if url.contains("/settings/initialize") {
                // Finishing setup is what marks Seerr initialised.
                if let Ok(mut signed) = self.signed_in.lock() {
                    *signed = true;
                }
                String::new()
            } else {
                let done = self.signed_in.lock().is_ok_and(|signed| *signed);
                format!(r#"{{"initialized":{done}}}"#)
            };
            Ok(crate::ports::http::Response { status: 200, body })
        }
    }

    #[tokio::test]
    async fn identity_does_nothing_without_both_jellyfin_and_seerr() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        // Seerr present but no Jellyfin, and the other way round: either alone is
        // nothing to wire.
        assert!(super::seed_jellyfin_identity(&ctx, &[seerr_svc()])
            .await
            .is_empty());
        assert!(super::seed_jellyfin_identity(&ctx, &[jellyfin_svc()])
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn identity_leaves_an_already_set_up_household_alone() {
        let env = config_scratch("jellyfin-already");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Jellyfin's password was recorded on an earlier run, and both services are
        // already set up: nothing is minted and nothing re-pointed.
        let _ = store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            "minted-earlier",
        );
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone())).with_http(Arc::new(
            HouseholdHttp {
                completed: true,
                signed_in: std::sync::Mutex::new(true),
            },
        ));

        let wirings = super::seed_jellyfin_identity(&ctx, &[jellyfin_svc(), seerr_svc()]).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::AlreadyWired)
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn identity_mints_records_and_wires_a_fresh_household() {
        let env = config_scratch("jellyfin-fresh");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
        let ctx = seed_ctx(
            None,
            true,
            Vec::new(),
            Some(vec![0x11; 24]),
            Some(env.clone()),
        )
        .with_http(Arc::new(HouseholdHttp {
            completed: false,
            signed_in: std::sync::Mutex::new(false),
        }));

        let wirings = super::seed_jellyfin_identity(&ctx, &[jellyfin_svc(), seerr_svc()]).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::Wired),
            "a fresh household is minted, signed in, and confirmed"
        );
        let written = std::fs::read_to_string(&env).unwrap_or_default();
        assert!(
            written.contains("JELLYFIN_ADMIN_PASSWORD="),
            "the minted password is recorded: {written}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn seed_wires_jellyfin_as_seerrs_identity() {
        // The whole command against the real manifest, which has both services;
        // Jellyfin reports its wizard done and no password was recorded, so the
        // household set it up and the identity is skipped for them to complete.
        let ctx = seed_ctx(None, true, Vec::new(), None, None).with_http(Arc::new(HouseholdHttp {
            completed: true,
            signed_in: std::sync::Mutex::new(false),
        }));
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let identity = report
            .wirings
            .iter()
            .find(|wiring| wiring.connection.contains("Seerr's identity"));
        assert!(
            identity.is_some_and(is_skipped),
            "an externally set-up Jellyfin leaves the identity for the household"
        );
    }

    #[test]
    fn a_target_carries_the_servarr_api_version() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // Sonarr answers at v3, Lidarr at v1: the version travels with the target
        // rather than being assumed by the client.
        let sonarr = manifest_service(
            "sonarr",
            Some(servarr_api_at(Some("/config/config.xml"), Some(3))),
            Some(8989),
        );
        assert_eq!(
            super::target_for(&sonarr, project).map(|target| target.version),
            Some(3)
        );
        let lidarr = manifest_service(
            "lidarr",
            Some(servarr_api_at(Some("/config/config.xml"), Some(1))),
            Some(8686),
        );
        assert_eq!(
            super::target_for(&lidarr, project).map(|target| target.version),
            Some(1)
        );
        // A servarr service that names no version cannot be reached at a known
        // path, so it is no target rather than one guessed at the wrong version.
        let versionless = manifest_service(
            "sonarr",
            Some(servarr_api_at(Some("/config/config.xml"), None)),
            Some(8989),
        );
        assert!(super::target_for(&versionless, project).is_none());
    }
}

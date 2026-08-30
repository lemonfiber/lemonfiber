//! Resolving the download clients a service should be told about.
//!
//! Where each one is reached, which credential proves it, and what category its
//! transfers are filed under.

use super::{read_temporary_password, record_qbittorrent_password, Ctx, Path, PathBuf};

/// Where a Servarr application reaches `SABnzbd` on the stack's network: the
/// container name, and `SABnzbd`'s own listening port rather than the
/// host-published one, because the application connects across the network, not
/// through the host.
pub(super) const SABNZBD_HOST: (&str, u16) = ("sabnzbd", 8080);

/// Where a Servarr application reaches qBittorrent: through Gluetun, whose network
/// namespace qBittorrent shares, on qBittorrent's web UI port. qBittorrent has no
/// network of its own, so its address is Gluetun's.
pub(super) const QBITTORRENT_HOST: (&str, u16) = ("gluetun", 8081);

/// The category an application files under, named as that application names its
/// category field, for the media type it manages.
///
/// The field is fixed per application — Sonarr names it `tvCategory`, Radarr
/// `movieCategory`, Lidarr `musicCategory` — so the mapping is by the media type
/// that identifies the application. A media type lemonfiber does not recognise has
/// no known field, so it names none rather than guessing.
pub(super) fn category_for(media: &str) -> Option<crate::ports::service::Category> {
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
pub(super) async fn read_sabnzbd_key(
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
pub(super) fn sabnzbd_config_path(
    services: &[lemonfiber_manifest::Service],
    project: &Path,
) -> Option<PathBuf> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != lemonfiber_manifest::ApiKind::Sabnzbd {
            return None;
        }
        crate::app::targets::config_path(project, service, api.path.as_deref())
    })
}

/// The download clients to register, one per credential that is in hand.
pub(super) fn download_clients(
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
                username: crate::config::QBITTORRENT_USER.to_owned(),
                password: password.to_owned(),
            },
            category: category.clone(),
        });
    }
    clients
}

/// qBittorrent's address, if the stack has it: the id names the container to read
/// a log from, the base is where the host reaches its web UI. Nothing where it
/// publishes no port — a client the host cannot reach is no target to wire.
pub(super) fn qbittorrent_target(
    services: &[lemonfiber_manifest::Service],
) -> Option<(String, String)> {
    crate::app::targets::service_addr(services, lemonfiber_manifest::ApiKind::Qbittorrent)
        .map(|addr| (addr.id, addr.loopback))
}

/// Set qBittorrent's web UI password, where it is still the one it started with.
///
/// A password already recorded and still accepted is the one in force, and the
/// connection reports that rather than setting another. Otherwise the temporary
/// password is read from the container's own log; without it there is nothing to
/// authenticate with, so the connection is skipped for a re-run once the container
/// has announced one. A generated password that lands is recorded in the
/// environment where the forwarded-port push reads it.
pub(super) async fn seed_qbittorrent_password(
    ctx: &Ctx,
    target: &(String, String),
) -> (crate::seed::Wiring, Option<String>) {
    let (id, base) = target;
    let connection = "qBittorrent web UI password".to_owned();
    let client = crate::qbittorrent::Qbittorrent::new(ctx.http.clone(), base);

    // A password lemonfiber has already set is the one in force, and asking again
    // is how a healthy stack gets reported as refused. The temporary one is still
    // in the container's log — a log is not consumed by being read — so a run that
    // reached for it a second time would authenticate with a credential that was
    // spent the first time. Checked against the client rather than assumed from
    // the recording, because a container rebuilt from nothing holds neither.
    if let Some(recorded) = super::super::targets::recorded_qbittorrent_password(ctx) {
        if client.accepts(&recorded).await.is_ok() {
            return (
                crate::seed::Wiring::settled(connection, crate::seed::State::AlreadyWired),
                None,
            );
        }
    }

    let Some(temporary) = read_temporary_password(ctx, id).await else {
        let wiring = crate::seed::Wiring::settled(
            connection,
            crate::seed::State::Skipped {
                reason: "qBittorrent has not announced a temporary password yet; a later run completes it".to_owned(),
            },
        );
        return (wiring, None);
    };

    let (wiring, recorded) =
        crate::seed::wire_qbittorrent_password(&client, ctx.random.as_ref(), &temporary).await;

    if let Some(password) = &recorded {
        record_qbittorrent_password(ctx, password);
    }
    (wiring, recorded)
}

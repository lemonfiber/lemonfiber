//! A served service's first run, as the in-process adapter its `api` names performs it,
//! and the credential that leaves lemonfiber holding.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_manifest::{ApiKind, KeySource};
use tokio::time::Instant;

use super::served::POLL;
use crate::nzbhydra2::{Credential, Nzbhydra2};
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::Placed;
use crate::plugin::program::Throwaway;
use crate::ports::http::Http;
use crate::ports::random::Random;
use crate::ports::serving::{Served, Serving};

/// How long a first run may take once the service is healthy.
pub(super) const FIRST_RUN: Duration = Duration::from_secs(60);

/// Where `NZBHydra2`'s configuration holds the key the instance made for itself.
const INSTANCE_KEY: &str = "/main/apiKey";

/// How many of the last lines a service wrote are read for what it announces.
const ANNOUNCED_WITHIN: usize = 200;

/// Where a request presents the credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Presented {
    /// In this header, holding this.
    Header(&'static str, String),
    /// In this query field, holding this.
    Query(&'static str, String),
    /// In the session the transport the first run signed in through carries.
    Session,
}

/// The credential a first run left lemonfiber holding, how its adapter presents it, and
/// every credential the run made or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    /// Where it is presented.
    pub(super) presented: Presented,
    /// Every credential the run made or read.
    pub(super) made: Vec<String>,
}

/// What a first run settles before the container starts: what the container's
/// environment is given, and the key that is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Before {
    /// What is added to the container's environment, as `NAME=value`.
    pub(super) env: Vec<String>,
    /// The key handed over there.
    key: Option<String>,
}

/// What a first run reaches the served service through.
pub(super) struct Reaching<'a> {
    /// The service, as installing it places it.
    pub(super) placed: &'a Placed,
    /// The throwaway copy of its configuration directory.
    pub(super) copy: &'a Throwaway,
    /// The container serving it.
    pub(super) served: &'a Served,
    /// What reads the container's log.
    pub(super) serving: &'a dyn Serving,
    /// The transport the first run signs in through, which its operator's requests are
    /// asked through after it.
    pub(super) keyed: Arc<dyn Http>,
    /// Where a minted credential comes from.
    pub(super) random: &'a dyn Random,
    /// What was settled before the container started.
    pub(super) before: &'a Before,
}

/// What the first run of the service `placed` records hands its container before it
/// starts: a key it adopts for a service that takes one so.
///
/// # Errors
///
/// Why no key could be minted.
pub(super) fn before(placed: &Placed, random: &dyn Random) -> Result<Before, String> {
    if placed.api.as_ref().map(|api| api.kind) != Some(ApiKind::Bindery) {
        return Ok(Before::default());
    }
    let key = minted(random)?;
    Ok(Before {
        env: vec![format!("{}={key}", crate::config::BOOK_CURATOR_API_KEY)],
        key: Some(key),
    })
}

/// A credential minted as lemonfiber mints every one.
fn minted(random: &dyn Random) -> Result<String, String> {
    crate::secret::generate(random)
        .ok_or_else(|| crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())
}

/// The first run of the service `reaching` serves, and what it leaves lemonfiber holding:
/// nothing for a service with no `api`.
///
/// # Errors
///
/// Why the first run could not be performed.
pub(super) async fn performed(reaching: &Reaching<'_>) -> Result<Option<Held>, String> {
    let Some(api) = reaching.placed.api.as_ref() else {
        return Ok(None);
    };
    if api.key_source == KeySource::None {
        return Ok(None);
    }
    let base = reaching
        .served
        .port
        .map(|port| format!("http://{LOOPBACK}:{port}"))
        .ok_or_else(|| "it publishes no port, so its first run cannot reach it".to_owned())?;
    match api.kind {
        ApiKind::Servarr | ApiKind::Seerr => read(reaching, |key| {
            Presented::Header(crate::endpoint::API_KEY_HEADER, key)
        })
        .await
        .map(Some),
        ApiKind::Bazarr => read(reaching, |key| Presented::Header(crate::bazarr::KEY, key))
            .await
            .map(Some),
        ApiKind::Sabnzbd => read(reaching, |key| {
            Presented::Query(crate::sabnzbd::KEY_FIELD, key)
        })
        .await
        .map(Some),
        ApiKind::Bindery => Ok(reaching.before.key.clone().map(|key| Held {
            presented: Presented::Header(crate::endpoint::API_KEY_HEADER, key.clone()),
            made: vec![key],
        })),
        ApiKind::Qbittorrent => torrenting(reaching, &base).await.map(Some),
        ApiKind::Audiobookshelf => listening(reaching, &base).await.map(Some),
        ApiKind::Nzbhydra2 => aggregating(reaching, &base).await.map(Some),
        ApiKind::Jellyfin => Err(
            "a media server's first run is its adapter's, and no adapter fronts it here".to_owned(),
        ),
    }
}

/// The key the service wrote to the file its `api` names, read from the copy once it is
/// there, and presented as `presented` makes it.
async fn read(
    reaching: &Reaching<'_>,
    presented: impl Fn(String) -> Presented,
) -> Result<Held, String> {
    let placed = reaching.placed;
    let api = placed
        .api
        .as_ref()
        .ok_or_else(|| "it names no adapter".to_owned())?;
    let reader = crate::app::credentials::reader(api.kind)
        .ok_or_else(|| format!("lemonfiber reads no key a {:?} service writes", api.kind))?;
    let path = api
        .path
        .as_deref()
        .ok_or_else(|| "its adapter names no file its key is written to".to_owned())?;
    let relative = beneath(
        path,
        crate::project::entry::configuration(&placed.config_path),
    )
    .ok_or_else(|| {
        format!("{path} is not beneath its configuration directory, so its key cannot be read")
    })?;
    let key = until_there(|| async {
        let wrote = reaching.copy.wrote(&relative)?;
        Ok(wrote
            .as_deref()
            .map(String::from_utf8_lossy)
            .and_then(|text| reader(&text)))
    })
    .await?
    .ok_or_else(|| {
        format!(
            "it wrote no key to {path} within {} seconds",
            FIRST_RUN.as_secs()
        )
    })?;
    Ok(Held {
        presented: presented(key.clone()),
        made: vec![key],
    })
}

/// qBittorrent's first run: the temporary password it announces, replaced by one
/// lemonfiber mints, signed in with through the transport its operator's requests take.
async fn torrenting(reaching: &Reaching<'_>, base: &str) -> Result<Held, String> {
    let announced = until_there(|| async {
        let log = reaching
            .serving
            .log(reaching.served, ANNOUNCED_WITHIN)
            .await;
        Ok(crate::qbittorrent::temporary_password(&log))
    })
    .await?
    .ok_or_else(|| {
        format!(
            "it announced no temporary password within {} seconds",
            FIRST_RUN.as_secs()
        )
    })?;
    let password = minted(reaching.random)?;
    crate::qbittorrent::Qbittorrent::new(Arc::clone(&reaching.keyed), base)
        .replace_password(&announced, &password)
        .await
        .map_err(|failure| failure.to_string())?;
    Ok(Held {
        presented: Presented::Session,
        made: vec![announced, password],
    })
}

/// Audiobookshelf's first run: the root account lemonfiber names, made with a password it
/// mints, and signed in as.
async fn listening(reaching: &Reaching<'_>, base: &str) -> Result<Held, String> {
    let client = crate::audiobookshelf::Audiobookshelf::new(
        Arc::clone(&reaching.keyed),
        base,
        &reaching.placed.service,
    );
    let password = minted(reaching.random)?;
    if client
        .has_account()
        .await
        .map_err(|failure| failure.to_string())?
    {
        return Err("it already holds an account, so it is not a fresh instance".to_owned());
    }
    client
        .create_account(crate::config::LISTENING_SERVER_USER, &password)
        .await
        .map_err(|failure| failure.to_string())?;
    let tokens = client
        .signed_in(crate::config::LISTENING_SERVER_USER, &password)
        .await
        .map_err(|failure| failure.to_string())?;
    let bearer = tokens.first().cloned().unwrap_or_default();
    let mut made = vec![password];
    made.extend(tokens);
    Ok(Held {
        presented: Presented::Header(crate::endpoint::AUTHORIZATION, format!("Bearer {bearer}")),
        made,
    })
}

/// `NZBHydra2`'s first run: its authentication turned on for the administrator lemonfiber
/// names, with a password it mints, and the restart that takes it into effect waited out.
async fn aggregating(reaching: &Reaching<'_>, base: &str) -> Result<Held, String> {
    let client = Nzbhydra2::new(Arc::clone(&reaching.keyed), base, &reaching.placed.service);
    let password = minted(reaching.random)?;
    let admin = Credential::admin(&password);
    let config = client
        .config(None)
        .await
        .map_err(|failure| failure.to_string())?;
    let mut made = vec![password.clone()];
    made.extend(
        config
            .pointer(INSTANCE_KEY)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    );
    client
        .guard(config, admin)
        .await
        .map_err(|unguarded| match unguarded {
            crate::nzbhydra2::Unguarded::Untaken(failure)
            | crate::nzbhydra2::Unguarded::Unknown(failure) => failure.to_string(),
        })?;
    client
        .restart()
        .await
        .map_err(|failure| failure.to_string())?;
    until_there(|| async { Ok(client.exposed().await.ok().filter(|exposed| !exposed)) })
        .await?
        .ok_or_else(|| {
            format!(
                "it still answered its configuration to anybody {} seconds after its restart",
                FIRST_RUN.as_secs()
            )
        })?;
    let header = admin.presented();
    let pair = header.strip_prefix("Basic ").unwrap_or_default().to_owned();
    made.push(pair);
    Ok(Held {
        presented: Presented::Header(crate::endpoint::AUTHORIZATION, header),
        made,
    })
}

/// Every credential a service announces in its log before anything takes it from it.
pub(super) fn announced(log: &str) -> Vec<String> {
    crate::qbittorrent::temporary_password(log)
        .into_iter()
        .collect()
}

/// What `look` finds once it finds anything, looked at again until it does or the first
/// run's limit passes; nothing where it never does.
async fn until_there<T, F, L>(look: L) -> Result<Option<T>, String>
where
    L: Fn() -> F,
    F: std::future::Future<Output = Result<Option<T>, String>>,
{
    let until = Instant::now() + FIRST_RUN;
    loop {
        if let Some(found) = look().await? {
            return Ok(Some(found));
        }
        if Instant::now() >= until {
            return Ok(None);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// `path` as it lies beneath `directory`, both as the container sees them.
fn beneath(path: &str, directory: &str) -> Option<std::path::PathBuf> {
    let inside = Path::new(path).strip_prefix(directory).ok()?;
    (!inside.as_os_str().is_empty()).then(|| inside.to_path_buf())
}

#[cfg(test)]
mod tests;

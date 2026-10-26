//! A served service's first run, as the in-process adapter its `api` names performs it,
//! and the credential that leaves lemonfiber holding.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_manifest::{Api, KeySource};
use tokio::time::Instant;

use super::served::POLL;
use crate::plugin::installed::Placed;
use crate::plugin::program::Throwaway;
use crate::ports::http::Http;
use crate::ports::serving::Served;

/// How long a first run may take once the service is healthy.
pub(super) const FIRST_RUN: Duration = Duration::from_secs(60);

/// A credential a request presents: in this header, holding this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Presented(pub(super) &'static str, pub(super) String);

/// The credential a first run left lemonfiber holding, how its adapter presents it, and
/// every credential the run made or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    /// Where it is presented.
    pub(super) presented: Presented,
    /// Every credential the run made or read.
    pub(super) made: Vec<String>,
}

/// What a first run reaches the served service through.
pub(super) struct Reaching<'a> {
    /// The service, as installing it places it.
    pub(super) placed: &'a Placed,
    /// The throwaway copy of its configuration directory.
    pub(super) copy: &'a Throwaway,
    /// The container serving it.
    pub(super) served: &'a Served,
    /// The transport a request presenting the credential the first run left is asked
    /// through.
    pub(super) keyed: Arc<dyn Http>,
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
    if reaching.served.port.is_none() {
        return Err("it publishes no port, so its first run cannot reach it".to_owned());
    }
    let reader = crate::credential::written::reader(api.key_source).ok_or_else(|| {
        "its first run is its adapter's, and no adapter fronts it here".to_owned()
    })?;
    read(reaching, api, reader).await.map(Some)
}

/// The key the service wrote to the file `api` names, read with `reader` from the copy
/// once it is there, and presented in the key header.
async fn read(
    reaching: &Reaching<'_>,
    api: &Api,
    reader: crate::credential::written::Reader,
) -> Result<Held, String> {
    let placed = reaching.placed;
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
        presented: Presented(crate::endpoint::API_KEY_HEADER, key.clone()),
        made: vec![key],
    })
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

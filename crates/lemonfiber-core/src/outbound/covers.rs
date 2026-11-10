//! The hosts the core fetched a music or book cover from, kept so the account of what
//! leaves this machine names each one.

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::COVER_HOSTS;

/// Every host a cover was fetched from.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Hosts {
    /// Each host, once, in order.
    #[serde(default)]
    hosts: Vec<String>,
}

/// Every host a cover was fetched from, each once, in order; none where nothing was kept.
pub(crate) fn recorded(ctx: &Ctx) -> Vec<String> {
    crate::app::targets::beside_env(ctx, COVER_HOSTS)
        .and_then(|path| crate::settings::kept::read::<Hosts>(&path).ok())
        .map(|kept| kept.hosts)
        .unwrap_or_default()
}

/// Note `host` as one a cover is fetched from, before it is; nothing where it could not be
/// noted.
pub(crate) fn record(ctx: &Ctx, host: &str) -> Option<()> {
    let path = crate::app::targets::beside_env(ctx, COVER_HOSTS)?;
    crate::settings::kept::changed(&path, &mut |kept: &mut Hosts| {
        if !kept.hosts.iter().any(|one| one == host) {
            kept.hosts.push(host.to_owned());
            kept.hosts.sort_unstable();
        }
        Ok(())
    })
    .ok()
}

#[cfg(test)]
mod tests;

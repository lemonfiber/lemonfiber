//! The household's own ntfy, which lemonfiber signs in to with the account its plugin
//! configured rather than with a token the operator hands over.

use url::{Host, Url};

use crate::app::Ctx;

use super::backends::{Backend, Held};
use crate::model::BackendKind;

/// The plugin, and its service, that bring the household's own ntfy.
const NTFY: &str = "ntfy";

/// The secret of that plugin its account's password is minted as.
const PASSWORD: &str = "password";

/// The account the plugin configures ntfy to let lemonfiber in as.
const ACCOUNT: &str = "lemonfiber";

/// What `backend` is reached with: what was handed over for it, or where it is the
/// household's own ntfy and nothing was, the account its plugin configured.
pub(crate) fn reached_with(ctx: &Ctx, backend: &Backend, held: Held) -> Held {
    if held.token.as_deref().is_some_and(|token| !token.is_empty()) {
        return held;
    }
    Held {
        token: login(ctx, backend).or(held.token),
        ..held
    }
}

/// The start of every topic the household's own ntfy lets lemonfiber post to and read.
pub(crate) const TOPICS: &str = "lemonfiber-";

/// The topic the household's own ntfy is posted to where the operator names none.
pub(crate) const ALERTS: &str = "lemonfiber-alerts";

/// `lemonfiber:<password>`, where `backend` is the household's own ntfy and the secret store
/// holds the password minted for it.
fn login(ctx: &Ctx, backend: &Backend) -> Option<String> {
    if !household(ctx, backend.kind, &backend.url) {
        return None;
    }
    let password =
        crate::app::targets::stored_secret(ctx, &crate::settings::named(NTFY, PASSWORD))?;
    Some(format!("{ACCOUNT}:{password}"))
}

/// Whether a back-end speaking `kind` at `url` is the household's own ntfy: ntfy at a
/// loopback address on the port this machine publishes the ntfy plugin's service on.
pub(crate) fn household(ctx: &Ctx, kind: BackendKind, url: &str) -> bool {
    if kind != BackendKind::Ntfy {
        return false;
    }
    let Some(url) = Url::parse(url).ok() else {
        return false;
    };
    let here = match url.host() {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if !here {
        return false;
    }
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return false;
    };
    crate::app::targets::fillers_here(ctx, &manifest)
        .service(NTFY)
        .filter(|filler| filler.brought_by() == Some(NTFY))
        .is_some_and(|ntfy| {
            ntfy.published.is_some() && ntfy.published == url.port_or_known_default()
        })
}

#[cfg(test)]
mod tests;

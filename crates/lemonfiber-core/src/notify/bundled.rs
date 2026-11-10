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

/// `lemonfiber:<password>`, where `backend` is ntfy at the port this machine publishes the
/// ntfy plugin's own service on, and the secret store holds the password minted for it.
fn login(ctx: &Ctx, backend: &Backend) -> Option<String> {
    if backend.kind != BackendKind::Ntfy {
        return None;
    }
    let url = Url::parse(&backend.url).ok()?;
    let here = match url.host()? {
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    };
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let fillers = crate::app::targets::fillers_here(ctx, &manifest);
    let ntfy = fillers
        .service(NTFY)
        .filter(|filler| filler.brought_by() == Some(NTFY))?;
    if !here || ntfy.published != url.port_or_known_default() {
        return None;
    }
    let password =
        crate::app::targets::stored_secret(ctx, &crate::settings::named(NTFY, PASSWORD))?;
    Some(format!("{ACCOUNT}:{password}"))
}

#[cfg(test)]
mod tests;

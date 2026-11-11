//! Where the stack's services reach the core to tell it what happened, and the
//! certificate they pin there.

use url::Url;

use crate::app::Ctx;

/// The name a container reaches this machine by.
pub const HOST: &str = "host.docker.internal";

/// Where on the web surface a service tells the core what happened.
pub const PATH: &str = "/api/notify/events";

/// Where a service tells the core what happened, and the certificate it pins there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Told {
    /// The whole address an event is posted to.
    pub address: String,
    /// SHA-256 over the DER encoding of the certificate the web surface presents,
    /// lower-case hex.
    pub pin: String,
}

/// Where the stack's services tell the core what happened: the address the operator
/// wrote down, or `host.docker.internal` at the port the web surface was last served
/// encrypted on where a container reaches it.
///
/// # Errors
///
/// Why no service can be told one.
pub(crate) fn address(ctx: &Ctx) -> Result<Told, String> {
    let kept_in = ctx
        .settings
        .companion
        .as_deref()
        .ok_or_else(|| UNKEPT.to_owned())?;
    let base = if let Some(written) = ctx.settings.events_address.as_deref() {
        written_base(written)?
    } else {
        let served = crate::companion::served::last(kept_in)
            .filter(|served| served.encrypted)
            .ok_or_else(unserved)?;
        if !served.network && !ctx.environment.resolves_host_gateway() {
            return Err(on_loopback());
        }
        format!("https://{HOST}:{}", served.port)
    };
    let pin = crate::certificate::kept_or_made(ctx.seams.confined.as_ref(), kept_in)
        .map_err(|why| format!("the certificate the web interface presents cannot be read: {why}"))?
        .fingerprint;
    Ok(Told {
        address: format!("{base}{PATH}"),
        pin,
    })
}

/// The address the operator wrote, without a trailing slash, where it is an `https`
/// address with a host and nothing a service could be misled by.
fn written_base(written: &str) -> Result<String, String> {
    Url::parse(written)
        .ok()
        .filter(|url| {
            url.scheme() == "https"
                && url.has_host()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && matches!(url.path(), "" | "/")
        })
        .map(|_| written.trim_end_matches('/').to_owned())
        .ok_or_else(|| {
            format!(
                "{} is {written}, which is not an https address of the web interface with \
                 nothing after its host and port",
                crate::config::EVENTS_ADDRESS_KEY
            )
        })
}

/// Why no service is told an address where there is nowhere to keep a certificate.
const UNKEPT: &str = "there is no configuration directory to keep the web interface's \
                      certificate in, so no service is asked to tell the core what happened";

/// Why no service is told an address where the web surface was served encrypted on this
/// machine's loopback alone, on an engine whose containers do not reach it there.
fn on_loopback() -> String {
    format!(
        "the web interface is served encrypted on this machine alone, and on this engine a \
         container reaches this machine only on the network, so no service is asked to tell \
         the core what happened; serve it encrypted on your network, or set {}",
        crate::config::EVENTS_ADDRESS_KEY
    )
}

/// Why no service is told an address where nothing has served the web surface where a
/// container reaches it.
fn unserved() -> String {
    format!(
        "the web interface has not been served encrypted, so no \
         service is asked to tell the core what happened; serve it encrypted on a port that \
         stays the same, or set {}",
        crate::config::EVENTS_ADDRESS_KEY
    )
}

#[cfg(test)]
mod tests;

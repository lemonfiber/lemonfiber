//! The keys the stack's own services hold on the media server, each filed under the name
//! of the service that holds it, so an operator reading the server's own key list can
//! tell whose each one is.

use crate::ports::service::{AppKeys, Failure};

/// The name lemonfiber itself would file a key under. Nothing in the stack holds one.
pub const OURS: &str = "lemonfiber";

/// The name the decline service's key is filed under.
pub const DECLINE_APP: &str = "lemonfiber-decline";

/// The name the request gate's key is filed under.
pub const GATE_APP: &str = "lemonfiber-request-gate";

/// Revoke the key filed under lemonfiber's own name, where there is one, and answer
/// whether there was.
///
/// No part of the stack reads one: the dashboard is published to the household network,
/// which is not somewhere a key that administers the server is kept. A key on the server
/// is a credential nothing needs, so it is taken off rather than left valid.
///
/// # Errors
///
/// Returns [`Failure`] where the server is unreachable, answers its key list unreadably,
/// or refuses the revocation.
pub async fn revoke_ours(keys: &dyn AppKeys) -> Result<bool, Failure> {
    let Some(ours) = keys.filed_as(OURS).await?.into_iter().next() else {
        return Ok(false);
    };
    keys.revoke(&ours).await?;
    Ok(true)
}

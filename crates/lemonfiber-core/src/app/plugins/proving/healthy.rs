//! What an install waits for before it asks a service it started anything: the health it
//! declares, and, before a first run through an adapter, the adapter's word that its
//! upstream answers, as a recording waits for both.

use std::time::SystemTime;

use crate::app::Ctx;
use crate::ports::http::{Method, Request};

/// Wait, until `deadline`, for the health path `placed` declares to pass at `address`,
/// where it is placed and declares one.
pub(super) async fn passed(
    ctx: &Ctx,
    placed: Option<&crate::plugin::Placed>,
    address: &str,
    deadline: SystemTime,
) {
    let Some(path) = placed
        .and_then(|placed| placed.health.as_deref())
        .filter(|path| lemonfiber_plugin::refusing::carried::is_route(path))
    else {
        return;
    };
    let request = Request {
        method: Method::Get,
        url: format!("{address}{path}"),
        headers: Vec::new(),
        body: None,
        pinned: None,
    };
    while ctx.seams.clock.now() < deadline
        && !ctx
            .seams
            .http
            .send(&request)
            .await
            .is_ok_and(|answered| crate::plugin::health_passes(answered.status))
    {
        tokio::time::sleep(super::POLL).await;
    }
}

/// Wait, until `deadline`, for `adapter` to say the upstream it fronts answers, as a
/// recording waits before the first run it performs through that adapter.
pub(super) async fn upstream_answering(
    ctx: &Ctx,
    adapter: &lemonfiber_contract::Contracted,
    deadline: SystemTime,
) {
    while ctx.seams.clock.now() < deadline && !adapter.ready().await.is_ok_and(|ready| ready) {
        tokio::time::sleep(super::POLL).await;
    }
}

#[cfg(test)]
mod tests;

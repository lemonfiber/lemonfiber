//! The health a service an install started declares, waited for before its proofs are
//! asked, as a recording is taken only once that health passes.

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

#[cfg(test)]
mod tests;

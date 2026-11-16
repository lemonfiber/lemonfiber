//! A request proof asked of a service at the port the engine publishes it on.

use crate::plugin::judging::{judge, live, method};
use crate::plugin::Verdict;
use crate::ports::http::Request;

use super::super::super::Ctx;
use super::POLL;

/// What asking one proof of one service came to.
///
/// A service that has not answered *yet* is asked again until the budget runs out,
/// because a container Compose has just created is not a service that is listening —
/// the gap is seconds and an install that took the first refusal would fail on every
/// image that takes a moment to open its socket. A service that answers something is
/// never asked twice: what it said is the evidence, and asking again until it says
/// something else is not proving, it is waiting for luck.
pub(crate) async fn answering(
    ctx: &Ctx,
    asked: &lemonfiber_plugin::Request,
    expect: &lemonfiber_plugin::Expect,
    address: &str,
    deadline: std::time::SystemTime,
) -> Verdict {
    let method = match askable(asked, address) {
        Ok(method) => method,
        Err(unaskable) => return unaskable,
    };
    let request = Request {
        method,
        url: format!("{address}{}", asked.path),
        headers: asked.sent(),
        body: None,
        pinned: None,
        deadline: None,
    };
    loop {
        match ctx.seams.http.send(&request).await {
            Ok(response) => {
                let faults = judge(expect, &live(&response));
                return if faults.is_empty() {
                    Verdict::Passed
                } else {
                    Verdict::Failed { faults }
                };
            }
            // Checked after the asking rather than before it, so a budget of nothing
            // still reports what the service said rather than reporting that it was
            // never asked.
            Err(unreachable) if ctx.seams.clock.now() >= deadline => {
                return Verdict::Unproven {
                    why: format!("{} did not answer: {}", unreachable.url, unreachable.reason),
                }
            }
            Err(_) => tokio::time::sleep(POLL).await,
        }
    }
}

/// The method `asked` is put to the service at `at` with, or why it is not put: a method
/// the transport cannot carry, which would be a proof about a question nobody declared were
/// it sent as something else, or a path that is no route on that service and could name
/// another host once joined to it, which the reader refused when the manifest was read and
/// is asked again where the join happens.
pub(super) fn askable(
    asked: &lemonfiber_plugin::Request,
    at: &str,
) -> Result<crate::ports::http::Method, Verdict> {
    let method = method(&asked.method).ok_or_else(|| Verdict::Unproven {
        why: format!(
            "{} is not a method lemonfiber can send, so the proof was never put",
            asked.method
        ),
    })?;
    if lemonfiber_plugin::refusing::carried::is_route(&asked.path) {
        Ok(method)
    } else {
        Err(Verdict::Unproven {
            why: format!(
                "{} is not a route on {at}, so the proof was never put",
                asked.path
            ),
        })
    }
}

#[cfg(test)]
mod tests;

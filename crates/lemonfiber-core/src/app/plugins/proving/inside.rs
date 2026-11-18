//! A request proof of a service that publishes no port on the engine's host, asked from
//! inside its own network namespace: the fence image's `fence ask`, run once in a
//! throwaway container joined to the namespace of the service's fence where it is fenced
//! and of the service itself otherwise, and judged as an answer at a published port is.
//! Nothing is published for it, so the service stays reachable from the stack's network
//! alone.

use crate::plugin::judging::{judge, live};
use crate::plugin::{Placed, Verdict};
use crate::ports::http::Response;

use super::super::super::Ctx;
use super::POLL;

/// The header naming what an answer's body is, as `fence ask` prints it.
const CONTENT_TYPE: &str = "content-type";

/// One proof's request and what it expects, and the fence image this build pins to ask
/// it with, where it pins one.
pub(super) struct Asked<'a> {
    /// The request the proof declares.
    pub(super) request: &'a lemonfiber_plugin::Request,
    /// What the answer must be.
    pub(super) expect: &'a lemonfiber_plugin::Expect,
    /// The fence image that asks it, where this build pins one.
    pub(super) image: Option<&'a str>,
}

/// What asking `placed` the proof `asked` from inside its own network namespace came to,
/// asked again while nothing answers there until `deadline`.
pub(super) async fn answering(
    ctx: &Ctx,
    placed: &Placed,
    asked: &Asked<'_>,
    deadline: std::time::SystemTime,
) -> Verdict {
    let service = &placed.service;
    let (request, expect) = (asked.request, asked.expect);
    if let Err(unaskable) = super::asking::askable(request, &format!("{service}'s own network")) {
        return unaskable;
    }
    let Some(image) = asked.image else {
        return Verdict::Unproven {
            why: format!(
                "{service} publishes no port, and this build's bundle pins no fence to ask it \
                 from inside its own network"
            ),
        };
    };
    let Some(port) = placed.listens else {
        return Verdict::Unproven {
            why: format!("{service} publishes no port and declares none it listens on"),
        };
    };
    let method = request.method.to_ascii_uppercase();
    loop {
        let answered = match joined(ctx, placed).await {
            Ok(namespace) => crate::app::project::asked_inside(
                ctx,
                image,
                &namespace,
                (&method, port, &request.path),
            )
            .await
            .and_then(|printed| read(printed.as_slice())),
            Err(why) => Err(why),
        };
        match answered {
            Ok(response) => {
                let faults = judge(expect, &live(&response));
                return if faults.is_empty() {
                    Verdict::Passed
                } else {
                    Verdict::Failed { faults }
                };
            }
            Err(why) if ctx.seams.clock.now() >= deadline => {
                return Verdict::Unproven {
                    why: format!("{service} did not answer from inside its own network: {why}"),
                }
            }
            Err(_) => tokio::time::sleep(POLL).await,
        }
    }
}

/// The id of the running container holding `placed`'s network namespace: its fence's
/// where it is fenced or held inside the stack, its own otherwise.
async fn joined(ctx: &Ctx, placed: &Placed) -> Result<String, String> {
    let holder = if placed
        .fields
        .network
        .as_ref()
        .is_some_and(|network| network.fenced || network.internal)
    {
        crate::fence::of(&placed.service)
    } else {
        placed.service.clone()
    };
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|_| format!("the engine would not say where {holder} runs"))?;
    containers
        .iter()
        .find(|container| container.service == holder)
        .map(|container| container.id.clone())
        .ok_or_else(|| format!("{holder} is not running"))
}

/// The answer `printed` holds as `fence ask` prints one: its status on the first line, its
/// `content-type` on the second, and its body after them.
fn read(printed: &[u8]) -> Result<Response, String> {
    let mut lines = printed.splitn(3, |byte| *byte == b'\n');
    let status = lines
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .and_then(|line| line.trim().parse::<u16>().ok())
        .ok_or("what the fence printed opens with no status")?;
    let content_type = lines
        .next()
        .map(|line| String::from_utf8_lossy(line).trim().to_owned())
        .ok_or("what the fence printed names no content type")?;
    let body = String::from_utf8_lossy(lines.next().unwrap_or_default()).into_owned();
    Ok(Response {
        status,
        headers: (!content_type.is_empty())
            .then(|| (CONTENT_TYPE.to_owned(), content_type))
            .into_iter()
            .collect(),
        body,
    })
}

#[cfg(test)]
mod tests;

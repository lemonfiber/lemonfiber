//! What one message to one back-end is, as each kind is spoken, and whether it was seen
//! arriving where the back-end keeps a history the core can read.

use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

use super::backends::{Backend, Held};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::endpoint::AUTHORIZATION;
use crate::model::{BackendKind, DeliveryOutcome};
use crate::ports::http::{Method, Request, JSON};

/// How long one back-end is waited for on one message.
pub const WAITED_FOR: Duration = Duration::from_secs(10);

/// The most of a back-end's refusal that is kept.
const REASON_MOST: usize = 200;

/// What every message is titled with.
const TITLE: &str = "lemonfiber";

/// How many of a gotify server's newest messages are looked through for one sent.
const GOTIFY_NEWEST: &str = "50";

/// One message to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    /// What kind of alert it carries.
    pub kind: Notification,
    /// What it says.
    pub summary: String,
    /// Whether it matters enough to be loud.
    pub urgent: bool,
    /// When it is sent, in seconds since the Unix epoch.
    pub at: u64,
}

/// What became of sending one message: the back-end's answer, and the id it gave the
/// message where its history can be read for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sent {
    /// What became of it.
    pub outcome: DeliveryOutcome,
    /// The id the back-end gave it, where it gave one.
    pub id: Option<String>,
    /// Whether the back-end refused what it was reached with.
    pub refused_key: bool,
}

/// An ntfy or gotify answer naming what it stored.
#[derive(Deserialize)]
struct Stored {
    /// The id it gave the message.
    id: serde_json::Value,
}

/// A gotify server's page of messages.
#[derive(Deserialize)]
struct Page {
    /// The messages on it.
    #[serde(default)]
    messages: Vec<Stored>,
}

/// Send `message` to `backend`, reached with `held`.
pub(crate) async fn send(ctx: &Ctx, backend: &Backend, held: &Held, message: &Message) -> Sent {
    let switch = crate::outbound::switch(crate::outbound::Reach::delivering(backend.kind));
    if !ctx.settings.reaching.allows(switch) {
        return failed(&format!("{switch} switches it off"));
    }
    let Some(request) = request(backend, held, message) else {
        return failed("it cannot be spoken to this way");
    };
    let answered = tokio::time::timeout(WAITED_FOR, ctx.seams.http.send(&request)).await;
    match answered {
        Ok(Ok(response)) if (200..300).contains(&response.status) => Sent {
            outcome: DeliveryOutcome::Sent,
            id: serde_json::from_str::<Stored>(&response.body)
                .ok()
                .map(|stored| said(&stored.id)),
            refused_key: false,
        },
        Ok(Ok(response)) => Sent {
            refused_key: matches!(response.status, 401 | 403),
            ..failed(&format!("{} {}", response.status, response.body))
        },
        Ok(Err(why)) => failed(&why.to_string()),
        Err(_) => failed("it did not answer in time"),
    }
}

/// Whether the message `id` is in `backend`'s history, where that history can be read.
pub(crate) async fn arrived(
    ctx: &Ctx,
    backend: &Backend,
    held: &Held,
    id: &str,
    since: u64,
) -> bool {
    let Some(request) = history(backend, held, since) else {
        return false;
    };
    let Ok(Ok(response)) = tokio::time::timeout(WAITED_FOR, ctx.seams.http.send(&request)).await
    else {
        return false;
    };
    (200..300).contains(&response.status) && seen(backend.kind, &response.body, id)
}

/// Whether the message `id` is in `history`, as a back-end of `kind` writes its history.
fn seen(kind: BackendKind, history: &str, id: &str) -> bool {
    match kind {
        BackendKind::Ntfy => history
            .lines()
            .filter_map(|line| serde_json::from_str::<Stored>(line).ok())
            .any(|stored| said(&stored.id) == id),
        BackendKind::Gotify => serde_json::from_str::<Page>(history)
            .is_ok_and(|page| page.messages.iter().any(|stored| said(&stored.id) == id)),
        BackendKind::Webhook | BackendKind::Apprise | BackendKind::Unifiedpush => false,
    }
}

/// The request that sends `message` to `backend`.
fn request(backend: &Backend, held: &Held, message: &Message) -> Option<Request> {
    let mut headers = Vec::new();
    let (url, body) = match backend.kind {
        BackendKind::Ntfy => {
            headers.push(("Title".to_owned(), TITLE.to_owned()));
            headers.push(("Tags".to_owned(), tag(message.kind).to_owned()));
            if message.urgent {
                headers.push(("Priority".to_owned(), "high".to_owned()));
            }
            headers.extend(bearer(held.token.as_deref()));
            (
                format!("{}/{}", backend.url, backend.topic.as_deref()?),
                message.summary.clone(),
            )
        }
        BackendKind::Gotify => {
            headers.push(("X-Gotify-Key".to_owned(), held.token.clone()?));
            headers.push(("Content-Type".to_owned(), JSON.to_owned()));
            let body = json!({
                "title": TITLE,
                "message": message.summary,
                "priority": if message.urgent { 8 } else { 5 },
            });
            (format!("{}/message", backend.url), body.to_string())
        }
        BackendKind::Webhook => {
            headers.push(("Content-Type".to_owned(), JSON.to_owned()));
            headers.extend(bearer(held.token.as_deref()));
            let body = json!({
                "notification": message.kind,
                "summary": message.summary,
                "at": message.at,
            });
            (backend.url.clone(), body.to_string())
        }
        BackendKind::Apprise => {
            headers.push(("Content-Type".to_owned(), JSON.to_owned()));
            let body = json!({
                "title": TITLE,
                "body": message.summary,
                "type": if message.urgent { "failure" } else { "info" },
            });
            (backend.url.clone(), body.to_string())
        }
        BackendKind::Unifiedpush => return None,
    };
    Some(Request {
        method: Method::Post,
        url,
        headers,
        body: Some(body),
        pinned: None,
    })
}

/// The request that reads `backend`'s history from `since`, where it keeps one the core
/// can read.
fn history(backend: &Backend, held: &Held, since: u64) -> Option<Request> {
    let (url, headers) = match backend.kind {
        BackendKind::Ntfy => (
            format!(
                "{}/{}/json?poll=1&since={since}",
                backend.url,
                backend.topic.as_deref()?
            ),
            bearer(held.token.as_deref()).into_iter().collect(),
        ),
        BackendKind::Gotify => (
            format!("{}/message?limit={GOTIFY_NEWEST}", backend.url),
            vec![("X-Gotify-Key".to_owned(), held.reader.clone()?)],
        ),
        BackendKind::Webhook | BackendKind::Apprise | BackendKind::Unifiedpush => return None,
    };
    Some(Request {
        method: Method::Get,
        url,
        headers,
        body: None,
        pinned: None,
    })
}

/// The header that hands `token` over, where there is one: a bearer token, or a login as
/// `user:password` handed over as one.
fn bearer(token: Option<&str>) -> Option<(String, String)> {
    let token = token.filter(|token| !token.is_empty())?;
    let value = if token.contains(':') {
        format!("Basic {}", STANDARD.encode(token))
    } else {
        format!("Bearer {token}")
    };
    Some((AUTHORIZATION.to_owned(), value))
}

/// The ntfy tag a notification kind is shown with.
const fn tag(kind: Notification) -> &'static str {
    match kind {
        Notification::Problem => "warning",
        Notification::Completion => "white_check_mark",
        Notification::Update => "arrow_up",
        Notification::Backup => "floppy_disk",
        Notification::Request => "inbox_tray",
        Notification::Member => "busts_in_silhouette",
        Notification::Ready => "tada",
    }
}

/// An id however the back-end wrote it.
fn said(id: &serde_json::Value) -> String {
    id.as_str().map_or_else(|| id.to_string(), str::to_owned)
}

/// A failure, in the back-end's own words, cut short and with no credential in them.
fn failed(reason: &str) -> Sent {
    let withheld = crate::error::withheld::without_credentials(reason.trim());
    Sent {
        outcome: DeliveryOutcome::Failed(withheld.chars().take(REASON_MOST).collect()),
        id: None,
        refused_key: false,
    }
}

#[cfg(test)]
mod tests;

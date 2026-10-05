//! A member's household, kept for a few seconds once it has been read.
//!
//! The household read is the operator's: it signs in to the media server and the
//! request service, reads every member and both libraries, and writes what the house
//! is shown. A member's read is that same work narrowed to them, so a member asking
//! over and over would drive the operator's whole reading as often as they asked. What
//! one asking answered is kept for [`KEPT_FOR`] and handed back to the same member
//! until then.
//!
//! Kept per member and after the command was narrowed, so what one member is handed
//! is only ever what was read for them. Whether they may ask at all is settled before
//! this is reached, on every call, and is never kept.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use axum::http::StatusCode;
use axum::response::Response;
use lemonfiber_core::app::{Command, Ctx};
use tokio::sync::Mutex;

use super::{enveloped, rendered};

/// How long a member's household read is handed back before it is read again.
///
/// Ten seconds: long enough that a page reloading in a loop costs one reading every
/// ten seconds rather than one per load, and short enough that a request they just
/// made is there when they look again.
pub const KEPT_FOR: Duration = Duration::from_secs(10);

/// One reading, as it was answered.
struct Answer {
    /// Until when it is handed back.
    until: SystemTime,
    /// The status it was answered with.
    status: StatusCode,
    /// The envelope, where it could be rendered.
    body: Option<String>,
}

/// The household each member read last.
#[derive(Default)]
pub struct Kept {
    /// By the member's id. Behind one lock held for the whole reading, so members
    /// asking at once wait for one reading rather than starting one each.
    held: Mutex<HashMap<String, Answer>>,
}

impl Kept {
    /// What `command` answers for `member`, read again only where the last answer
    /// has run out.
    pub async fn read(&self, ctx: &Ctx, member: &str, command: Command) -> Response {
        let now = ctx.seams.clock.now();
        let mut held = self.held.lock().await;
        held.retain(|_, answer| answer.until > now);
        if let Some(answer) = held.get(member) {
            return enveloped(answer.status, answer.body.clone());
        }
        let (status, body) = rendered(ctx, command).await;
        if let Some(until) = now.checked_add(KEPT_FOR) {
            held.insert(
                member.to_owned(),
                Answer {
                    until,
                    status,
                    body: body.clone(),
                },
            );
        }
        enveloped(status, body)
    }
}

//! Saying what is owed, and noticing when a channel would not take it.
//!
//! The whole chain meets here. Conditions know what is wrong, a digest decides
//! what is worth interrupting somebody about, the outbox writes it down, and this
//! hands it to whatever channels the operator configured.
//!
//! Two rules make it safe to run every time something is checked.
//!
//! In-app delivery cannot fail, because it is the writing-down itself. Whatever
//! else happens, the operator can find the alert. That is what lets a channel be
//! allowed to refuse without any special handling to avoid losing anything.
//!
//! And a channel that refuses raises a condition of its own — an operator whose
//! alerts have been going nowhere for a week needs telling, and the only place
//! that can come from is here. It is deliberately not sent through channels: a
//! notification about notifications failing, delivered by the thing that failed,
//! is either a loop or a lie.

use crate::alert::{Digest, Outbox};
use crate::condition::{Conditions, Fault};
use crate::error::Severity;
use crate::health::Reach;
use crate::notify::Channel;

use crate::app::Ctx;

/// The prefix a channel's own condition is filed under, so the ones about
/// delivery can be told from the ones about the stack.
pub const CHANNEL_CHECK: &str = "notify.channel";

/// The kind of event a refusing channel raises — one kind however many channels
/// refuse, so twelve dead channels are one thing wrong and not twelve.
pub(crate) const CHANNEL_REFUSED: &str = "notify.channel.refused";

/// What one round of notifying did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notified {
    /// What was said, if anything.
    pub digest: Digest,
    /// The channels that would not take it, by name.
    pub refused: Vec<String>,
    /// Whether it was held back for the hours the operator asked not to be woken in.
    ///
    /// Held, not dropped: the outbox has it either way, so a held alert is one they
    /// find when they next look rather than one nobody ever sees.
    pub held: bool,
}

impl Notified {
    /// Whether anything was worth saying at all.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.digest.is_empty()
    }
}

/// Whether the hour holds this digest back.
///
/// False whenever anything in it overrides a quiet period, so a critical alert is never
/// the thing a window swallows — which is the whole of what the window has to get right.
fn held(ctx: &Ctx, digest: &Digest) -> bool {
    let Some(window) = &ctx.settings.quiet else {
        return false;
    };
    !digest.overrides_quiet() && window.holds(ctx.clock.now())
}

/// Say whatever the conditions now warrant, through every channel given.
///
/// `reach` is how far the stack got, because a stack the operator deliberately
/// stopped is not a stack that broke: every service being down is what was asked
/// for, and reporting it teaches them that stopping the stack means a page of
/// alerts.
///
/// Returns what was said and which channels refused it. Both the outbox and the
/// conditions are updated: the outbox because the operator has now been told, and
/// the conditions because a channel refusing is itself something being wrong.
pub async fn notify(
    ctx: &Ctx,
    reach: Reach,
    conditions: &mut Conditions,
    outbox: &mut Outbox,
    channels: &[&dyn Channel],
) -> Notified {
    let now = ctx.stamp();
    // What the operator asked to hear about, as answered at setup and changed
    // since. Read here rather than passed in, so no caller can forget it.
    let wants = crate::app::appetite::recorded(ctx);
    let digest = Digest::wanted(reach, &wants, conditions.all(), &|check| outbox.told(check));
    if digest.is_empty() {
        return Notified::default();
    }

    // Written down before anything is attempted. From here the operator can find
    // it whatever the channels do — including when the hour holds it back.
    outbox.owe(digest.alerts.clone());

    // The one window this product holds against a time of day, and the only thing that
    // carries an alert through it is the alert being loud enough to warrant it. A held
    // digest is not split: delivering the emergency now and its context in the morning
    // would be an emergency arriving without what it is about.
    if held(ctx, &digest) {
        return Notified {
            digest,
            held: true,
            ..Notified::default()
        };
    }

    let mut refused = Vec::new();
    for channel in channels {
        let said = channel.deliver(&digest).await;
        let check = format!("{CHANNEL_CHECK}.{}", channel.name());
        match said {
            Ok(()) => conditions.observe(&check, None, &now),
            Err(problem) => {
                let fault = Fault::new(
                    CHANNEL_REFUSED,
                    Severity::Warning,
                    &problem.reason,
                    "nothing is reaching this channel, so an alert raised while it stays down is \
                     waiting on the screen rather than arriving anywhere",
                    "check the channel's configuration; the alert is kept in-app either way",
                );
                conditions.observe(&check, Some(&fault), &now);
                refused.push(problem.channel);
            }
        }
    }

    // In-app delivery is the writing-down, and it has already happened — so the
    // operator has been told, whatever the channels did, and the same fault will
    // not be reported to them again.
    outbox.delivered(&|check| {
        conditions
            .get(check)
            .map_or(0, |condition| condition.recurrences)
    });

    Notified {
        digest,
        refused,
        held: false,
    }
}

#[cfg(test)]
mod tests;

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
use crate::ports::notify::Channel;

use super::Ctx;

/// The prefix a channel's own condition is filed under, so the ones about
/// delivery can be told from the ones about the stack.
pub const CHANNEL_CHECK: &str = "notify.channel";

/// What one round of notifying did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notified {
    /// What was said, if anything.
    pub digest: Digest,
    /// The channels that would not take it, by name.
    pub refused: Vec<String>,
}

impl Notified {
    /// Whether anything was worth saying at all.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.digest.is_empty()
    }
}

/// Say whatever the conditions now warrant, through every channel given.
///
/// Returns what was said and which channels refused it. Both the outbox and the
/// conditions are updated: the outbox because the operator has now been told, and
/// the conditions because a channel refusing is itself something being wrong.
pub async fn notify(
    ctx: &Ctx,
    conditions: &mut Conditions,
    outbox: &mut Outbox,
    channels: &[&dyn Channel],
) -> Notified {
    let now = ctx.stamp();
    let digest = Digest::of(conditions.all(), &|check| outbox.told(check));
    if digest.is_empty() {
        return Notified::default();
    }

    // Written down before anything is attempted. From here the operator can find
    // it whatever the channels do.
    outbox.owe(digest.alerts.clone());

    let mut refused = Vec::new();
    for channel in channels {
        let said = channel.deliver(&digest).await;
        let check = format!("{CHANNEL_CHECK}.{}", channel.name());
        match said {
            Ok(()) => conditions.observe(&check, None, &now),
            Err(problem) => {
                let fault = Fault::new(
                    Severity::Warning,
                    &problem.reason,
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

    Notified { digest, refused }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::{notify, Notified, CHANNEL_CHECK};
    use crate::alert::{Digest, Outbox};
    use crate::condition::{Conditions, Fault};
    use crate::config::Settings;
    use crate::error::Severity;
    use crate::platform::Environment;
    use crate::ports::notify::{Channel, Undelivered};
    use crate::test_support::{spoke, stack, Reporting, Scripted as ScriptedRunner};

    /// A channel that records what it was given, or refuses everything.
    struct Scripted {
        name: &'static str,
        refuses: bool,
        got: Mutex<Vec<Digest>>,
    }

    impl Scripted {
        fn taking(name: &'static str) -> Self {
            Self {
                name,
                refuses: false,
                got: Mutex::new(Vec::new()),
            }
        }

        fn refusing(name: &'static str) -> Self {
            Self {
                refuses: true,
                ..Self::taking(name)
            }
        }

        fn delivered(&self) -> usize {
            self.got.lock().map(|got| got.len()).unwrap_or_default()
        }
    }

    #[async_trait]
    impl Channel for Scripted {
        fn name(&self) -> &str {
            self.name
        }

        async fn deliver(&self, digest: &Digest) -> Result<(), Undelivered> {
            if self.refuses {
                return Err(Undelivered {
                    channel: self.name.to_owned(),
                    reason: "nothing answered".to_owned(),
                });
            }
            if let Ok(mut got) = self.got.lock() {
                got.push(digest.clone());
            }
            Ok(())
        }
    }

    /// A context that needs nothing but a clock — notifying reaches no service.
    fn plain_ctx() -> crate::app::Ctx {
        crate::app::Ctx::new(
            std::sync::Arc::new(ScriptedRunner(Ok(spoke("")))),
            std::sync::Arc::new(Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
    }

    /// A store with one thing wrong.
    fn stalled() -> Conditions {
        let mut conditions = Conditions::new();
        conditions.observe(
            "queue.stalled",
            Some(&Fault::new(
                Severity::Warning,
                "two downloads have not moved",
                "check the indexer is answering",
            )),
            "1000",
        );
        conditions
    }

    #[tokio::test]
    async fn something_newly_wrong_reaches_the_channels_and_the_history() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let channel = Scripted::taking("discord");

        let said = notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        assert!(!said.is_quiet());
        assert!(said.refused.is_empty());
        assert_eq!(channel.delivered(), 1);
        assert_eq!(outbox.history().len(), 1, "and it is readable in the app");
    }

    #[tokio::test]
    async fn the_same_fault_is_not_reported_twice() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let channel = Scripted::taking("discord");

        notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;
        let again = notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        assert!(again.is_quiet(), "nothing new to say");
        assert_eq!(channel.delivered(), 1, "and nothing sent");
    }

    #[tokio::test]
    async fn a_channel_that_refuses_loses_nothing() {
        // The case the whole design is for: the channel is down, which is often the
        // same outage the operator needed telling about.
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let channel = Scripted::refusing("discord");

        let said = notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        assert_eq!(said.refused, vec!["discord".to_owned()]);
        assert_eq!(outbox.history().len(), 1, "the operator can still find it");
    }

    #[tokio::test]
    async fn a_channel_that_refuses_becomes_something_wrong_in_its_own_right() {
        // Alerts going nowhere for a week is worth knowing, and here is the only
        // place that can notice it.
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let channel = Scripted::refusing("discord");

        notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        let raised = conditions.get(&format!("{CHANNEL_CHECK}.discord"));
        assert!(raised.is_some_and(crate::condition::Condition::is_raised));
        assert_eq!(raised.map(|c| c.summary.as_str()), Some("nothing answered"));
    }

    #[tokio::test]
    async fn a_channel_that_starts_working_again_stops_being_wrong() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        notify(
            &ctx,
            &mut conditions,
            &mut outbox,
            &[&Scripted::refusing("discord")],
        )
        .await;

        // Something new to say, and this time the channel takes it.
        conditions.observe(
            "disk.full",
            Some(&Fault::new(
                Severity::Error,
                "no room left",
                "delete something, or move the library",
            )),
            "2000",
        );
        notify(
            &ctx,
            &mut conditions,
            &mut outbox,
            &[&Scripted::taking("discord")],
        )
        .await;

        let recovered = conditions.get(&format!("{CHANNEL_CHECK}.discord"));
        assert!(recovered.is_some_and(|c| !c.is_raised()));
    }

    #[tokio::test]
    async fn one_channel_refusing_does_not_stop_another_taking_it() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let working = Scripted::taking("email");
        let broken = Scripted::refusing("discord");

        let said = notify(&ctx, &mut conditions, &mut outbox, &[&broken, &working]).await;

        assert_eq!(said.refused, vec!["discord".to_owned()]);
        assert_eq!(working.delivered(), 1);
    }

    #[tokio::test]
    async fn a_healthy_stack_says_nothing_to_anybody() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (Conditions::new(), Outbox::new());
        let channel = Scripted::taking("discord");

        let said = notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        assert!(said.is_quiet());
        assert_eq!(said, Notified::default());
        assert_eq!(channel.delivered(), 0);
        assert!(outbox.history().is_empty());
    }

    #[tokio::test]
    async fn a_resolution_is_said_to_whoever_heard_the_onset() {
        let ctx = plain_ctx();
        let (mut conditions, mut outbox) = (stalled(), Outbox::new());
        let channel = Scripted::taking("discord");
        notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        conditions.observe("queue.stalled", None, "2026-08-09T11:00:00Z");
        let over = notify(&ctx, &mut conditions, &mut outbox, &[&channel]).await;

        assert!(!over.is_quiet(), "it resolving is news");
        assert_eq!(channel.delivered(), 2);
        assert_eq!(outbox.history().len(), 2);
    }
}

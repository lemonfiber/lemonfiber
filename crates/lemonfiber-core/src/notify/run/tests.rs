use std::sync::Mutex;

use async_trait::async_trait;

use super::{notify, Notified, CHANNEL_CHECK};
use crate::alert::{Digest, Outbox};
use crate::condition::{Conditions, Fault};
use crate::error::Severity;
use crate::health::Reach;
use crate::notify::{Channel, Undelivered};
use crate::test_support::{a_context, spoke, Reporting, Scripted as ScriptedRunner};

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
    a_context()
        .runner(std::sync::Arc::new(ScriptedRunner(Ok(spoke("")))))
        .engine(std::sync::Arc::new(Reporting::absent()))
        .build()
}

/// A store with one thing wrong.
fn stalled() -> Conditions {
    let mut conditions = Conditions::new();
    conditions.observe(
        "queue.stalled",
        Some(&Fault::new(
            "queue.stalled",
            Severity::Warning,
            "two downloads have not moved",
            "nothing is arriving for them",
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

    let said = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

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

    notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;
    let again = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

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

    let said = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

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

    notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

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
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&Scripted::refusing("discord")],
    )
    .await;

    // Something new to say, and this time the channel takes it.
    conditions.observe(
        "disk.full",
        Some(&Fault::new(
            "storage.full",
            Severity::Error,
            "no room left",
            "nothing can be written until something goes",
            "delete something, or move the library",
        )),
        "2000",
    );
    notify(
        &ctx,
        Reach::Running,
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

    let said = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&broken, &working],
    )
    .await;

    assert_eq!(said.refused, vec!["discord".to_owned()]);
    assert_eq!(working.delivered(), 1);
}

#[tokio::test]
async fn a_healthy_stack_says_nothing_to_anybody() {
    let ctx = plain_ctx();
    let (mut conditions, mut outbox) = (Conditions::new(), Outbox::new());
    let channel = Scripted::taking("discord");

    let said = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

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
    notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

    conditions.observe("queue.stalled", None, "2026-08-09T11:00:00Z");
    let over = notify(
        &ctx,
        Reach::Running,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

    assert!(!over.is_quiet(), "it resolving is news");
    assert_eq!(channel.delivered(), 2);
    assert_eq!(outbox.history().len(), 2);
}

#[tokio::test]
async fn a_stack_the_operator_stopped_sends_nothing_and_owes_nothing() {
    // Not merely unsent: nothing is written down as owed either, or the alerts
    // would arrive in a rush the moment the stack came back up.
    let ctx = plain_ctx();
    let (mut conditions, mut outbox) = (stalled(), Outbox::new());
    let channel = Scripted::taking("discord");

    let said = notify(
        &ctx,
        Reach::Stopped,
        &mut conditions,
        &mut outbox,
        &[&channel],
    )
    .await;

    assert!(said.is_quiet());
    assert_eq!(channel.delivered(), 0);
    assert!(!outbox.owes_anything());
}

/// 2026-01-15 at the given UTC hour — deep winter, so a northern zone is one hour
/// ahead and the two do not agree about what time it is.
fn winter(hour: u64) -> std::sync::Arc<lemonfiber_fixtures::ports::Stopped> {
    lemonfiber_fixtures::ports::Stopped::at(1_768_435_200 + hour * 3_600)
}

/// A context whose operator asked not to be woken between ten and seven.
fn asleep_at(hour: u64) -> crate::app::Ctx {
    let settings = crate::config::Settings {
        quiet: crate::alert::Quiet::parse("22:00-07:00", "UTC"),
        ..crate::config::Settings::default()
    };
    a_context()
        .runner(std::sync::Arc::new(ScriptedRunner(Ok(spoke("")))))
        .engine(std::sync::Arc::new(Reporting::absent()))
        .clock(winter(hour))
        .settings(settings)
        .build()
}

/// A store with one thing badly wrong.
fn critical() -> Conditions {
    let mut conditions = Conditions::new();
    conditions.observe(
        "vpn.leak",
        Some(&Fault::new(
            "vpn.leak",
            Severity::Critical,
            "traffic left outside the tunnel",
            "this connection's address is visible to every peer",
            "stop the client",
        )),
        "1",
    );
    conditions
}

/// The hours nobody wants waking for hold what can wait.
#[tokio::test]
async fn a_warning_inside_the_window_is_held_rather_than_sent() {
    let channel = Scripted::taking("phone");
    let said = notify(
        &asleep_at(3),
        Reach::Running,
        &mut stalled(),
        &mut Outbox::default(),
        &[&channel],
    )
    .await;

    assert!(said.held, "held");
    assert_eq!(channel.delivered(), 0, "nothing was delivered");
}

/// Held is not dropped: the operator finds it when they next look.
#[tokio::test]
async fn a_held_alert_is_still_written_down() {
    let mut outbox = Outbox::default();
    let channel = Scripted::taking("phone");
    let _ = notify(
        &asleep_at(3),
        Reach::Running,
        &mut stalled(),
        &mut outbox,
        &[&channel],
    )
    .await;

    assert!(outbox.owes_anything(), "the outbox has it");
}

/// The requirement itself: a quiet hour is never what swallows an emergency.
#[tokio::test]
async fn a_critical_alert_inside_the_window_is_sent_anyway() {
    let channel = Scripted::taking("phone");
    let said = notify(
        &asleep_at(3),
        Reach::Running,
        &mut critical(),
        &mut Outbox::default(),
        &[&channel],
    )
    .await;

    assert!(!said.held, "not held");
    assert_eq!(channel.delivered(), 1, "delivered");
}

/// Outside the window nothing is held at all.
#[tokio::test]
async fn a_warning_outside_the_window_is_sent() {
    let channel = Scripted::taking("phone");
    let said = notify(
        &asleep_at(12),
        Reach::Running,
        &mut stalled(),
        &mut Outbox::default(),
        &[&channel],
    )
    .await;

    assert!(!said.held, "not held");
    assert_eq!(channel.delivered(), 1, "delivered");
}

/// And an operator who asked for no window is woken as before.
#[tokio::test]
async fn no_window_holds_nothing() {
    let channel = Scripted::taking("phone");
    let said = notify(
        &plain_ctx(),
        Reach::Running,
        &mut stalled(),
        &mut Outbox::default(),
        &[&channel],
    )
    .await;

    assert!(!said.held, "not held");
    assert_eq!(channel.delivered(), 1, "delivered");
}

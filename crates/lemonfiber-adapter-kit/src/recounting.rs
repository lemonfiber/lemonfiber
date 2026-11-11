//! Telling the core what an adapter's upstream did.
//!
//! An adapter hands a [`Recounter`] the upstream's [`History`] and, where the upstream's own
//! notification connection can reach it, that [`Connection`]. Asked `events`, the
//! recounter keeps a cursor at the upstream's newest happening and, from then on, sends
//! every happening after it to the core, oldest first, moving the cursor after each send
//! the core answered. It reads the history every `every` seconds where it polls; where it
//! is pushed to, a push at its secret path makes it read the history at once, and it reads
//! it on its own start and once a day besides. What a push carries is never sent on: only
//! what the upstream's history holds is.

mod kept;
mod round;
mod told;

use std::path::PathBuf;
use std::sync::{Arc, Mutex as Held};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_contract::adapter::EVENTS_KEY_FILE;
use lemonfiber_ports::narration::Narrator;
use lemonfiber_ports::random::Random;
use lemonfiber_ports::service::{Event, Failure, Heard, Hearing, Recounting, EVERY, EVERY_DEFAULT};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use kept::Kept;
pub use kept::EVENTS_FILE;
use told::Told;

/// Where beneath an adapter's address its upstream pushes, before the secret.
pub const PUSHED: &str = "/events";

/// How many random bytes a push path's secret holds.
const SECRET_BYTES: usize = 32;

/// How long a recounter hearing by push waits for a push before it reads the history
/// anyway.
const DAILY: Duration = Duration::from_secs(24 * 60 * 60);

/// How long a recounter hearing by push waits after a reading that did not finish before
/// it reads again.
const RETRY: Duration = Duration::from_secs(EVERY_DEFAULT as u64);

/// One happening in an upstream's history: where it stands, and what it tells the core,
/// where it tells it anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The upstream's own position of it, as the cursor is kept.
    pub position: String,
    /// What it tells the core, or nothing where it is a happening the core is not told.
    pub event: Option<Event>,
}

/// An upstream's own history or activity, read by position.
#[async_trait]
pub trait History: Send + Sync {
    /// Where the upstream's newest happening stands, or where its history begins where it
    /// holds none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the upstream is unreachable or refuses.
    async fn newest(&self) -> Result<String, Failure>;

    /// The next page of happenings after `cursor`, oldest first: empty where none
    /// follows, nothing where the upstream no longer holds `cursor`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the upstream is unreachable or refuses.
    async fn after(&self, cursor: &str) -> Result<Option<Vec<Entry>>, Failure>;
}

/// An upstream's own notification connection, pointed at the adapter.
#[async_trait]
pub trait Connection: Send + Sync {
    /// Point the one connection this adapter keeps in its upstream at `at`, replacing any
    /// it pointed before, where the upstream's connection is configured or configurable
    /// without installing anything, enabling a paid feature or turning on a notification
    /// type that carries what anybody watches. Answers whether it did.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the upstream is unreachable or refuses.
    async fn connected(&self, at: &str) -> Result<bool, Failure>;

    /// Remove the connection this adapter keeps in its upstream, where there is one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the upstream is unreachable or refuses.
    async fn disconnected(&self) -> Result<(), Failure>;
}

/// What an adapter asked `events` runs: one cursor, one task, and the secret its upstream
/// pushes at.
#[derive(Clone)]
pub struct Recounter {
    /// Everything the task shares with the asks.
    shared: Arc<Shared>,
}

/// The upstream's connection, and the address it reaches the adapter at.
#[derive(Clone)]
struct Pushed {
    /// The connection.
    connection: Arc<dyn Connection>,
    /// The adapter's own address as its upstream reaches it, without a trailing slash.
    reached_at: String,
}

/// What a recounter is made of.
struct Parts {
    /// The upstream, as its failures name it.
    service: String,
    /// The adapter's configuration directory.
    configuration: PathBuf,
    /// The upstream's history.
    history: Arc<dyn History>,
    /// The upstream's connection, where it has one.
    pushed: Option<Pushed>,
    /// Where a push path's secret is drawn from.
    random: Arc<dyn Random>,
    /// Where what it says is said.
    narrator: Arc<dyn Narrator>,
}

/// What the task shares with the asks.
pub(crate) struct Shared {
    /// The upstream, as its failures name it.
    service: String,
    /// The adapter's configuration directory.
    configuration: PathBuf,
    /// The upstream's history.
    history: Arc<dyn History>,
    /// The upstream's connection, where it has one.
    pushed: Option<Pushed>,
    /// What is kept and the task running, changed by one ask or one send at a time.
    running: Mutex<Running>,
    /// The secret a push is taken at, read without waiting on a send.
    secret: Held<Option<String>>,
    /// Woken by each push.
    woken: Notify,
    /// Where a push path's secret is drawn from.
    random: Arc<dyn Random>,
    /// Where what it says is said.
    narrator: Arc<dyn Narrator>,
}

/// What is kept and the task running.
#[derive(Default)]
pub(crate) struct Running {
    /// What is kept, where the adapter was asked.
    kept: Option<Kept>,
    /// The task, where one runs.
    task: Option<JoinHandle<()>>,
}

impl Recounter {
    /// A recounter for `service`, reading `history`, keeping its cursor beneath
    /// `configuration` and saying through `narrator` what the adapter's log is told,
    /// polling alone, drawing a push path's secret from the operating system.
    #[must_use]
    pub fn new(
        service: impl Into<String>,
        configuration: impl Into<PathBuf>,
        history: Arc<dyn History>,
        narrator: Arc<dyn Narrator>,
    ) -> Self {
        Self::built(Parts {
            service: service.into(),
            configuration: configuration.into(),
            history,
            pushed: None,
            random: Arc::new(crate::random::Os),
            narrator,
        })
    }

    /// The same recounter, pushed to through `connection` where it can be, at
    /// `reached_at`, the adapter's own address as its upstream reaches it.
    #[must_use]
    pub fn pushed_through(
        self,
        connection: Arc<dyn Connection>,
        reached_at: impl Into<String>,
    ) -> Self {
        Self::built(Parts {
            pushed: Some(Pushed {
                connection,
                reached_at: reached_at.into().trim_end_matches('/').to_owned(),
            }),
            ..self.parts()
        })
    }

    /// The same recounter, drawing a push path's secret from `random`.
    #[must_use]
    pub fn drawing_from(self, random: Arc<dyn Random>) -> Self {
        Self::built(Parts {
            random,
            ..self.parts()
        })
    }

    /// What this recounter was made of.
    fn parts(&self) -> Parts {
        let shared = &self.shared;
        Parts {
            service: shared.service.clone(),
            configuration: shared.configuration.clone(),
            history: Arc::clone(&shared.history),
            pushed: shared.pushed.clone(),
            random: Arc::clone(&shared.random),
            narrator: Arc::clone(&shared.narrator),
        }
    }

    /// A recounter of `parts`, asked nothing yet.
    fn built(parts: Parts) -> Self {
        Self {
            shared: Arc::new(Shared {
                service: parts.service,
                configuration: parts.configuration,
                history: parts.history,
                pushed: parts.pushed,
                running: Mutex::new(Running::default()),
                secret: Held::new(None),
                woken: Notify::new(),
                random: parts.random,
                narrator: parts.narrator,
            }),
        }
    }

    /// Take up what was kept from before the adapter last stopped, sending to the core
    /// with the key it wrote beside the configuration, and read the history at once.
    pub async fn resumed(&self) {
        let shared = &self.shared;
        let kept = match kept::read(&shared.configuration) {
            Ok(Some(kept)) => kept,
            Ok(None) => return,
            Err(why) => {
                return shared
                    .said(&format!("what it kept could not be read: {why}"))
                    .await
            }
        };
        let key = std::fs::read_to_string(shared.configuration.join(EVENTS_KEY_FILE))
            .map(|key| key.trim().to_owned())
            .unwrap_or_default();
        let Some(told) = Told::new(&kept.address, &kept.pin, &key) else {
            return shared
                .said("what it kept or the key it tells the core with will not do, so it waits to be asked again")
                .await;
        };
        let mut running = shared.running.lock().await;
        if running.kept.is_none() {
            self.run(&mut running, told, kept);
        }
    }

    /// Whether `offered` is the secret a push is taken at, and the history is read at
    /// once where it is.
    #[must_use]
    pub fn woken_by(&self, offered: &str) -> bool {
        let held = self
            .shared
            .secret
            .lock()
            .ok()
            .and_then(|secret| secret.clone())
            .unwrap_or_default();
        let woken = crate::same_secret(&held, offered);
        if woken {
            self.shared.woken.notify_one();
        }
        woken
    }

    /// Send to `told` from now on, keeping the cursor kept, or setting it at the
    /// upstream's newest where none is, and answer how the upstream is heard.
    async fn started(
        &self,
        told: Told,
        address: &str,
        pin: &str,
        every: u32,
    ) -> Result<Heard, Failure> {
        let shared = &self.shared;
        let mut running = shared.running.lock().await;
        let before = running
            .kept
            .clone()
            .or_else(|| kept::read(&shared.configuration).ok().flatten());
        let cursor = match &before {
            Some(before) => before.cursor.clone(),
            None => shared.history.newest().await?,
        };
        let secret = self.pushed(before.and_then(|before| before.secret)).await?;
        let kept = Kept {
            address: address.to_owned(),
            pin: pin.to_owned(),
            every,
            hearing: if secret.is_some() {
                Hearing::Push
            } else {
                Hearing::Poll
            },
            cursor,
            secret,
        };
        kept::write(&shared.configuration, &kept)
            .map_err(|why| shared.refused(&format!("the cursor could not be kept: {why}")))?;
        let heard = kept.heard();
        self.run(&mut running, told, kept);
        Ok(heard)
    }

    /// The secret the upstream's connection now pushes at: `kept`, or a fresh one where
    /// none was kept; nothing where the upstream cannot push.
    async fn pushed(&self, kept: Option<String>) -> Result<Option<String>, Failure> {
        let shared = &self.shared;
        let Some(pushed) = &shared.pushed else {
            return Ok(None);
        };
        let Some(secret) = kept.or_else(|| made(shared.random.as_ref())) else {
            shared
                .said("no secret could be made for a push, so it polls")
                .await;
            return Ok(None);
        };
        let at = format!("{}{PUSHED}/{secret}", pushed.reached_at);
        Ok(pushed.connection.connected(&at).await?.then_some(secret))
    }

    /// The task replaced by one sending to `told` from `kept`.
    fn run(&self, running: &mut Running, told: Told, kept: Kept) {
        let shared = &self.shared;
        if let Some(task) = running.task.take() {
            task.abort();
        }
        if let Ok(mut secret) = shared.secret.lock() {
            secret.clone_from(&kept.secret);
        }
        let waiting = round::Waiting::of(&kept);
        running.kept = Some(kept);
        running.task = Some(tokio::spawn(round::heard(
            Arc::clone(shared),
            told,
            waiting,
        )));
    }

    /// Send nothing more, drop the cursor, and remove the connection pointed at the
    /// adapter where there is one.
    async fn stopped(&self) -> Result<(), Failure> {
        let shared = &self.shared;
        let mut running = shared.running.lock().await;
        if let Some(task) = running.task.take() {
            task.abort();
        }
        if let Ok(mut secret) = shared.secret.lock() {
            *secret = None;
        }
        let before = running
            .kept
            .take()
            .or_else(|| kept::read(&shared.configuration).ok().flatten());
        kept::dropped(&shared.configuration)
            .map_err(|why| shared.refused(&format!("the cursor could not be dropped: {why}")))?;
        match (&shared.pushed, before) {
            (
                Some(pushed),
                Some(Kept {
                    hearing: Hearing::Push,
                    ..
                }),
            ) => pushed.connection.disconnected().await,
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl Recounting for Recounter {
    async fn events(
        &self,
        address: Option<&str>,
        pin: Option<&str>,
        key: Option<&str>,
        every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        self.asked(address, pin, key, every).await
    }
}

impl Recounter {
    /// Send from now on as `events` was asked, or stop where it was asked with nothing.
    async fn asked(
        &self,
        address: Option<&str>,
        pin: Option<&str>,
        key: Option<&str>,
        every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        let shared = &self.shared;
        match (address, pin, key) {
            (None, None, None) => self.stopped().await.map(|()| None),
            (Some(address), Some(pin), Some(key)) => {
                let every = every.unwrap_or(EVERY_DEFAULT);
                if !EVERY.contains(&every) {
                    return Err(shared.disallowed(&format!(
                        "every has to be from {} to {} seconds",
                        EVERY.start(),
                        EVERY.end()
                    )));
                }
                let told = Told::new(address, pin, key).ok_or_else(|| {
                    shared.disallowed(
                        "the address has to be https, the pin a SHA-256 and the key spelled as a key is",
                    )
                })?;
                self.started(told, address, pin, every).await.map(Some)
            }
            _ => Err(shared.disallowed(
                "the address, the pin and the key are asked all together or not at all",
            )),
        }
    }
}

impl Shared {
    /// `what` said in the adapter's log.
    async fn said(&self, what: &str) {
        self.narrator
            .say(&format!("telling the core what happened: {what}"))
            .await;
    }

    /// An ask refused for `detail`.
    fn disallowed(&self, detail: &str) -> Failure {
        Failure::Disallowed {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// An ask that could not be carried out, for `detail`.
    fn refused(&self, detail: &str) -> Failure {
        Failure::Refused {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }
}

/// A fresh secret for a push path drawn from `random`, or nothing where no randomness
/// could be had.
fn made(random: &dyn Random) -> Option<String> {
    random
        .bytes(SECRET_BYTES)
        .filter(|bytes| bytes.len() == SECRET_BYTES)
        .map(|bytes| lemonfiber_ports::digest::hex(&bytes))
}

#[cfg(test)]
mod tests;

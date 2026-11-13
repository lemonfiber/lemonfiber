use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use lemonfiber_contract::adapter::EVENTS_KEY_FILE;
use lemonfiber_fixtures::heard::Heard as Narrated;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_pinned::fake::Served;
use lemonfiber_ports::service::{Event, Failure, Heard, Hearing, Recounting};

use super::kept::{self, Kept};
use super::told::tests::{added, address, core, Posts, KEY};
use super::{Connection, Entry, History, Recounter, PUSHED};

pub(crate) const SERVICE: &str = "Upstream";

const REACHED_AT: &str = "http://upstream-adapter:8080/";

/// An upstream history numbered from one, read a page of `PAGE` at a time.
#[derive(Default)]
pub(crate) struct Script {
    entries: Mutex<Vec<Entry>>,
    gone: Mutex<Option<String>>,
    failing: AtomicBool,
    reads: AtomicUsize,
}

const PAGE: usize = 2;

impl Script {
    pub(crate) fn holding(names: &[Option<&str>]) -> Arc<Self> {
        let script = Arc::new(Self::default());
        script.happened(names);
        script
    }

    pub(crate) fn happened(&self, names: &[Option<&str>]) {
        if let Ok(mut entries) = self.entries.lock() {
            for name in names {
                let position = (entries.len() + 1).to_string();
                entries.push(Entry {
                    position,
                    event: name.map(added),
                });
            }
        }
    }

    pub(crate) fn forgets(&self, position: &str) {
        if let Ok(mut gone) = self.gone.lock() {
            *gone = Some(position.to_owned());
        }
    }

    pub(crate) fn fails(&self) {
        self.failing.store(true, Ordering::SeqCst);
    }

    pub(crate) fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    fn refused(&self) -> Result<(), Failure> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable {
                service: SERVICE.to_owned(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl History for Script {
    async fn newest(&self) -> Result<String, Failure> {
        self.refused()?;
        Ok(self
            .entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or_default()
            .to_string())
    }

    async fn after(&self, cursor: &str) -> Result<Option<Vec<Entry>>, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.refused()?;
        if self
            .gone
            .lock()
            .ok()
            .and_then(|gone| gone.clone())
            .as_deref()
            == Some(cursor)
        {
            return Ok(None);
        }
        let from: usize = cursor.parse().unwrap_or_default();
        Ok(Some(
            self.entries
                .lock()
                .map(|entries| entries.iter().skip(from).take(PAGE).cloned().collect())
                .unwrap_or_default(),
        ))
    }
}

/// An upstream connection that can push where `pushes`, keeping where it points.
#[derive(Default)]
struct Hook {
    pushes: bool,
    failing: bool,
    pointed: Mutex<Vec<String>>,
    removed: AtomicUsize,
}

#[async_trait]
impl Connection for Hook {
    async fn connected(&self, at: &str) -> Result<bool, Failure> {
        if self.failing {
            return Err(Failure::Unavailable {
                service: SERVICE.to_owned(),
            });
        }
        if let Ok(mut pointed) = self.pointed.lock() {
            pointed.push(at.to_owned());
        }
        Ok(self.pushes)
    }

    async fn disconnected(&self) -> Result<(), Failure> {
        self.removed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A recounter for `history` keeping beneath `dir`, saying what it says to `heard`.
pub(crate) fn recounter_heard(
    dir: &Path,
    history: &Arc<Script>,
    heard: &Arc<Narrated>,
) -> Recounter {
    Recounter::new(
        SERVICE,
        dir,
        Arc::clone(history) as Arc<dyn History>,
        Arc::clone(heard) as Arc<dyn lemonfiber_ports::narration::Narrator>,
    )
}

/// A recounter for `history` keeping beneath `dir`.
pub(crate) fn recounter(dir: &Path, history: &Arc<Script>) -> Recounter {
    recounter_heard(dir, history, &Arc::default())
}

fn pushed(dir: &Path, history: &Arc<Script>, hook: &Arc<Hook>) -> Recounter {
    recounter(dir, history).pushed_through(Arc::clone(hook) as Arc<dyn Connection>, REACHED_AT)
}

async fn asked(
    recounter: &Recounter,
    served: &Served,
    key: &str,
    every: Option<u32>,
) -> Result<Option<Heard>, Failure> {
    recounter
        .events(Some(&address(served)), Some(&served.pin), Some(key), every)
        .await
}

pub(crate) fn kept_in(dir: &Path) -> Option<Kept> {
    kept::read(dir).ok().flatten()
}

/// Every event the core was sent, by name.
pub(crate) fn names(posts: &Posts) -> Vec<String> {
    posts
        .lock()
        .map(|posts| posts.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|(_, body)| serde_json::from_str::<Event>(body).ok())
        .map(|event| event.name)
        .collect()
}

/// Wait a while for `until` to hold, and say whether it did.
pub(crate) async fn settled(until: impl Fn() -> bool) -> bool {
    for _ in 0..500 {
        if until() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    until()
}

fn cursor_is(dir: &Path, position: &str) -> bool {
    kept_in(dir).is_some_and(|kept| kept.cursor == position)
}

#[tokio::test]
async fn a_first_ask_sets_the_cursor_at_the_newest_and_sends_nothing_older() {
    let dir = Scratch::new("recounter-first-ask");
    let history = Script::holding(&[Some("Old"), Some("Older"), None]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    let heard = asked(&recounter, &served, KEY, Some(90))
        .await
        .ok()
        .flatten();
    assert_eq!(
        heard,
        Some(Heard {
            hearing: Hearing::Poll,
            every: Some(90)
        })
    );
    assert!(cursor_is(dir.path(), "3"));
    assert!(settled(|| history.reads() >= 1).await);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn an_ask_without_an_interval_polls_at_the_default() {
    let dir = Scratch::new("recounter-default-every");
    let history = Script::holding(&[]);
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let heard = asked(&recounter(dir.path(), &history), &served, KEY, None)
        .await
        .ok()
        .flatten();
    assert_eq!(heard.and_then(|heard| heard.every), Some(60));
    assert!(cursor_is(dir.path(), "0"));
}

#[tokio::test]
async fn an_ask_again_keeps_the_cursor_and_sends_what_happened_since_oldest_first() {
    let dir = Scratch::new("recounter-asked-again");
    let history = Script::holding(&[Some("Old")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    history.happened(&[Some("Dune"), None, Some("Arrival")]);
    let other = "fedcba9876543210fedcba9876543210";
    assert!(asked(&recounter, &served, other, None).await.is_ok());
    assert!(settled(|| cursor_is(dir.path(), "4")).await);
    assert_eq!(names(&posts), ["Dune", "Arrival"]);
    let bearers: Vec<String> = posts
        .lock()
        .map(|posts| posts.iter().map(|(bearer, _)| bearer.clone()).collect())
        .unwrap_or_default();
    assert!(bearers
        .iter()
        .all(|bearer| *bearer == format!("Bearer {other}")));
}

#[tokio::test]
async fn a_send_the_core_did_not_answer_leaves_the_cursor_where_it_was() {
    let dir = Scratch::new("recounter-unanswered");
    let history = Script::holding(&[]);
    let Some((served, posts)) = core(StatusCode::INTERNAL_SERVER_ERROR) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    history.happened(&[Some("Dune")]);
    let mut kept = kept_ask(&served);
    kept.cursor = "0".to_owned();
    assert!(kept::write(dir.path(), &kept).is_ok());
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    assert!(settled(|| !names(&posts).is_empty()).await);
    assert!(cursor_is(dir.path(), "0"));
}

#[tokio::test]
async fn a_stop_drops_the_cursor_and_a_later_ask_starts_from_the_newest() {
    let dir = Scratch::new("recounter-stopped");
    let history = Script::holding(&[Some("Old")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    assert!(settled(|| history.reads() >= 1).await);
    assert_eq!(
        recounter.events(None, None, None, None).await.ok(),
        Some(None)
    );
    assert!(kept_in(dir.path()).is_none());
    history.happened(&[Some("While stopped")]);
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    assert!(cursor_is(dir.path(), "2"));
    assert!(settled(|| history.reads() >= 2).await);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn an_ask_naming_some_but_not_all_or_an_interval_out_of_range_is_refused() {
    let dir = Scratch::new("recounter-refused");
    let history = Script::holding(&[]);
    let recounter = recounter(dir.path(), &history);
    let pin = "a".repeat(64);
    let address = "https://core.invalid/api/notify/events";
    let refused = |answer: Result<Option<Heard>, Failure>| matches!(answer, Err(Failure::Disallowed { service, .. }) if service == SERVICE);
    assert!(refused(
        recounter.events(Some(address), None, Some(KEY), None).await
    ));
    assert!(refused(
        recounter
            .events(Some(address), Some(&pin), Some(KEY), Some(14))
            .await
    ));
    assert!(refused(
        recounter
            .events(Some(address), Some(&pin), Some(KEY), Some(3601))
            .await
    ));
    assert!(refused(
        recounter
            .events(Some("http://core.invalid"), Some(&pin), Some(KEY), None)
            .await
    ));
    assert!(kept_in(dir.path()).is_none());
}

#[tokio::test]
async fn an_upstream_that_cannot_be_read_on_a_first_ask_keeps_nothing() {
    let dir = Scratch::new("recounter-unread");
    let history = Script::holding(&[]);
    history.fails();
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let answer = asked(&recounter(dir.path(), &history), &served, KEY, None).await;
    assert!(matches!(answer, Err(Failure::Unavailable { .. })));
    assert!(kept_in(dir.path()).is_none());
}

#[tokio::test]
async fn a_cursor_that_cannot_be_kept_or_dropped_refuses_the_ask_and_runs_nothing() {
    let dir = Scratch::new("recounter-unkeepable");
    let file = dir.path().join("a-file");
    assert!(std::fs::write(&file, "not a directory").is_ok());
    let history = Script::holding(&[Some("Old")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(&file, &history);
    let refused = |answer: Result<Option<Heard>, Failure>, what: &str| matches!(answer, Err(Failure::Refused { service, detail }) if service == SERVICE && detail.contains(what));
    assert!(refused(
        asked(&recounter, &served, KEY, None).await,
        "the cursor could not be kept"
    ));
    assert!(refused(
        recounter.events(None, None, None, None).await,
        "the cursor could not be dropped"
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(history.reads(), 0);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn an_upstream_that_pushes_is_pointed_at_a_secret_path_and_heard_by_push() {
    let dir = Scratch::new("recounter-pushed");
    let history = Script::holding(&[Some("Old")]);
    let hook = Arc::new(Hook {
        pushes: true,
        ..Hook::default()
    });
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = pushed(dir.path(), &history, &hook);
    let heard = asked(&recounter, &served, KEY, None).await.ok().flatten();
    assert_eq!(
        heard,
        Some(Heard {
            hearing: Hearing::Push,
            every: None
        })
    );
    let secret = kept_in(dir.path())
        .and_then(|kept| kept.secret)
        .unwrap_or_default();
    assert_eq!(secret.len(), 64);
    let pointed = hook
        .pointed
        .lock()
        .map(|pointed| pointed.clone())
        .unwrap_or_default();
    assert_eq!(
        pointed,
        [format!("http://upstream-adapter:8080{PUSHED}/{secret}")]
    );
    assert!(settled(|| history.reads() >= 1).await);

    history.happened(&[Some("Dune"), Some("Arrival"), Some("Heat")]);
    assert!(!recounter.woken_by("forged"));
    assert!(!recounter.woken_by(""));
    assert!(recounter.woken_by(&secret));
    assert!(settled(|| cursor_is(dir.path(), "4")).await);
    assert_eq!(names(&posts), ["Dune", "Arrival", "Heat"]);

    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    assert_eq!(
        kept_in(dir.path()).and_then(|kept| kept.secret),
        Some(secret.clone())
    );

    assert!(recounter.events(None, None, None, None).await.is_ok());
    assert_eq!(hook.removed.load(Ordering::SeqCst), 1);
    assert!(!recounter.woken_by(&secret));
}

#[tokio::test]
async fn a_push_with_nothing_new_in_the_history_sends_nothing() {
    let dir = Scratch::new("recounter-push-nothing");
    let history = Script::holding(&[Some("Old")]);
    let hook = Arc::new(Hook {
        pushes: true,
        ..Hook::default()
    });
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = pushed(dir.path(), &history, &hook);
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    assert!(settled(|| history.reads() >= 1).await);
    let secret = kept_in(dir.path())
        .and_then(|kept| kept.secret)
        .unwrap_or_default();
    assert!(recounter.woken_by(&secret));
    assert!(settled(|| history.reads() >= 2).await);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn an_upstream_that_cannot_push_is_polled_and_its_connection_left_alone_at_a_stop() {
    let dir = Scratch::new("recounter-cannot-push");
    let history = Script::holding(&[]);
    let hook = Arc::new(Hook::default());
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = pushed(dir.path(), &history, &hook);
    let heard = asked(&recounter, &served, KEY, None).await.ok().flatten();
    assert_eq!(heard.map(|heard| heard.hearing), Some(Hearing::Poll));
    assert_eq!(kept_in(dir.path()).and_then(|kept| kept.secret), None);
    assert!(recounter.events(None, None, None, None).await.is_ok());
    assert_eq!(hook.removed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_upstream_connection_that_fails_fails_the_ask() {
    let dir = Scratch::new("recounter-hook-fails");
    let history = Script::holding(&[]);
    let hook = Arc::new(Hook {
        failing: true,
        ..Hook::default()
    });
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let answer = asked(&pushed(dir.path(), &history, &hook), &served, KEY, None).await;
    assert!(matches!(answer, Err(Failure::Unavailable { .. })));
    assert!(kept_in(dir.path()).is_none());
}

/// What an ask to `served` keeps, polling, with its cursor at the start.
fn kept_ask(served: &Served) -> Kept {
    Kept {
        address: address(served),
        pin: served.pin.clone(),
        every: 60,
        hearing: Hearing::Poll,
        cursor: "0".to_owned(),
        secret: None,
    }
}

#[tokio::test]
async fn a_restart_takes_up_what_was_kept_and_sends_what_happened_while_down() {
    let dir = Scratch::new("recounter-resumed");
    let history = Script::holding(&[Some("Old"), Some("Dune"), Some("Arrival")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let mut kept = kept_ask(&served);
    kept.cursor = "1".to_owned();
    assert!(kept::write(dir.path(), &kept).is_ok());
    let _ = std::fs::write(dir.path().join(EVENTS_KEY_FILE), format!("{KEY}\n"));
    recounter(dir.path(), &history).resumed().await;
    assert!(settled(|| cursor_is(dir.path(), "3")).await);
    assert_eq!(names(&posts), ["Dune", "Arrival"]);
}

#[tokio::test]
async fn a_kit_served_with_a_recounter_takes_up_what_was_kept_before_it_answers() {
    let dir = Scratch::new("recounter-served");
    let history = Script::holding(&[Some("Old"), Some("Dune")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let mut kept = kept_ask(&served);
    kept.cursor = "1".to_owned();
    assert!(kept::write(dir.path(), &kept).is_ok());
    let _ = std::fs::write(dir.path().join(EVENTS_KEY_FILE), KEY);
    let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a loopback port is always free to bind");
    };
    let kit = crate::tests::kit().recounted_by(recounter(dir.path(), &history));
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(crate::serving(kit, listener, async {
        let _ = stopped.await;
    }));
    assert!(settled(|| cursor_is(dir.path(), "2")).await);
    assert_eq!(names(&posts), ["Dune"]);
    let _ = stop.send(());
    assert!(matches!(server.await, Ok(Ok(()))));
}

#[tokio::test]
async fn a_restart_with_nothing_kept_or_no_key_runs_nothing() {
    let dir = Scratch::new("recounter-not-resumed");
    let history = Script::holding(&[Some("Old")]);
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    recounter.resumed().await;
    assert!(kept::write(dir.path(), &kept_ask(&served)).is_ok());
    recounter.resumed().await;
    let _ = std::fs::write(dir.path().join(kept::EVENTS_FILE), "not kept");
    recounter.resumed().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(history.reads(), 0);
}

#[tokio::test]
async fn the_kit_takes_a_push_at_the_secret_path_alone() {
    let dir = Scratch::new("recounter-routed");
    let history = Script::holding(&[]);
    let hook = Arc::new(Hook {
        pushes: true,
        ..Hook::default()
    });
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = pushed(dir.path(), &history, &hook);
    assert!(asked(&recounter, &served, KEY, None).await.is_ok());
    let secret = kept_in(dir.path())
        .and_then(|kept| kept.secret)
        .unwrap_or_default();
    let kit = crate::tests::kit().recounted_by(recounter);
    let at = |secret: &str| format!("{PUSHED}/{secret}");
    let status = |answer: (StatusCode, String, String)| answer.0;
    assert_eq!(
        status(crate::tests::asked(kit.clone(), "POST", &at(&secret), None).await),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        status(crate::tests::asked(kit.clone(), "POST", &at("forged"), None).await),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(crate::tests::asked(crate::tests::kit(), "POST", &at(&secret), None).await),
        StatusCode::NOT_FOUND
    );
}

/// Randomness the platform could not give.
struct Exhausted;

impl lemonfiber_ports::random::Random for Exhausted {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        None
    }
}

#[tokio::test]
async fn with_no_randomness_for_a_secret_it_polls_and_points_nothing_at_itself() {
    let dir = Scratch::new("recounter-no-randomness");
    let history = Script::holding(&[]);
    let hook = Arc::new(Hook {
        pushes: true,
        ..Hook::default()
    });
    let Some((served, _)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = pushed(dir.path(), &history, &hook).drawing_from(Arc::new(Exhausted));
    let heard = asked(&recounter, &served, KEY, None).await.ok().flatten();
    assert_eq!(heard.map(|heard| heard.hearing), Some(Hearing::Poll));
    assert!(hook.pointed.lock().is_ok_and(|pointed| pointed.is_empty()));
}

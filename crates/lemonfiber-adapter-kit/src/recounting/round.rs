//! The task a recounter runs: read the history after the cursor, send each happening in
//! order, move the cursor after each one the core answered, then wait.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_ports::service::Hearing;

use super::kept::{self, Kept};
use super::told::Told;
use super::{Entry, Shared, DAILY, RETRY};

/// What the task waits for between readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// The interval it polls at.
    Every(Duration),
    /// A push, or the daily reading.
    Pushed,
}

impl Waiting {
    /// What a task sending from `kept` waits for.
    pub(crate) fn of(kept: &Kept) -> Self {
        match kept.hearing {
            Hearing::Poll => Self::Every(Duration::from_secs(u64::from(kept.every))),
            Hearing::Push => Self::Pushed,
        }
    }
}

/// Read and send, then wait, until the task is replaced or stopped.
pub(crate) async fn heard(shared: Arc<Shared>, told: Told, waiting: Waiting) {
    loop {
        let finished = round(&shared, &told).await;
        match waiting {
            Waiting::Every(every) => tokio::time::sleep(every).await,
            Waiting::Pushed => {
                let wait = if finished { DAILY } else { RETRY };
                tokio::select! {
                    () = shared.woken.notified() => {}
                    () = tokio::time::sleep(wait) => {}
                }
            }
        }
    }
}

/// Every happening after the cursor sent, a page at a time; whether it got to the end.
pub(crate) async fn round(shared: &Shared, told: &Told) -> bool {
    loop {
        let Some(cursor) = cursor(shared).await else {
            return false;
        };
        let page = match shared.history.after(&cursor).await {
            Ok(Some(page)) => page,
            Ok(None) => return moved(shared, &cursor).await,
            Err(failure) => {
                shared
                    .said(&format!("its history could not be read: {failure}"))
                    .await;
                return false;
            }
        };
        match page.last() {
            None => return true,
            Some(last) if last.position == cursor => return true,
            Some(_) => {}
        }
        for entry in page {
            if !sent(shared, told, entry).await {
                return false;
            }
        }
    }
}

/// The cursor kept now.
async fn cursor(shared: &Shared) -> Option<String> {
    shared
        .running
        .lock()
        .await
        .kept
        .as_ref()
        .map(|kept| kept.cursor.clone())
}

/// `entry` sent where it tells the core anything, and the cursor moved to it where the
/// core answered; whether it was.
async fn sent(shared: &Shared, told: &Told, entry: Entry) -> bool {
    let mut running = shared.running.lock().await;
    let Some(kept) = running.kept.as_mut() else {
        return false;
    };
    if let Some(event) = &entry.event {
        if !told.sent(event).await {
            return false;
        }
    }
    kept.cursor = entry.position;
    kept_again(shared, kept).await
}

/// The cursor moved from `gone`, a position the upstream no longer holds, to its newest
/// happening, and said so; whether it was.
async fn moved(shared: &Shared, gone: &str) -> bool {
    let newest = match shared.history.newest().await {
        Ok(newest) => newest,
        Err(failure) => {
            shared
                .said(&format!("its history could not be read: {failure}"))
                .await;
            return false;
        }
    };
    let mut running = shared.running.lock().await;
    let Some(kept) = running.kept.as_mut() else {
        return false;
    };
    if kept.cursor != gone {
        return false;
    }
    shared
        .said(&format!(
            "its history no longer holds {gone}, so it reads on from its newest, {newest}"
        ))
        .await;
    kept.cursor = newest;
    kept_again(shared, kept).await
}

/// `kept` written again; whether it was, said where it was not.
async fn kept_again(shared: &Shared, kept: &Kept) -> bool {
    match kept::write(&shared.configuration, kept) {
        Ok(()) => true,
        Err(why) => {
            shared
                .said(&format!("the cursor could not be kept: {why}"))
                .await;
            false
        }
    }
}

#[cfg(test)]
mod tests;

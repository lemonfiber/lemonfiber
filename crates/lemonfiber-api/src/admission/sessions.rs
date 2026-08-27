//! The sessions this run has opened, and what ends one.
//!
//! Nothing here outlives the process. A session names somebody admitted to *this*
//! run, and a record that outlived the run would admit somebody to a surface that
//! is no longer serving — the rule the names given to work already follow, for the
//! same reason.
//!
//! Two things end a session, and neither is a request to end it.
//!
//! **It expires**, on an absolute clock rather than on use. A sliding window
//! renews itself for as long as anything keeps touching it, so a browser left open
//! on another tab is indistinguishable from a person still sitting there — and the
//! case this exists for is a phone somebody put down.
//!
//! **A password change voids it.** Each session remembers the verifier it was
//! opened against, and a session opened against a verifier that is no longer the
//! one on disk is not this password's session. That is what makes changing the
//! password a way to end a session somebody else is holding, rather than only a way
//! to stop the next one — and it needs no message passed from wherever the change
//! happened, which matters because the change can happen in another process.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use lemonfiber_core::admission::Credential;
use lemonfiber_core::model::Admitted;
use lemonfiber_core::ports::random::Random;
use tokio::sync::Mutex;

/// Bytes of secret a session is carried by.
///
/// The width the per-run token is, and for the width's own reason: it is the whole
/// of what a request proves itself with, so guessing must not be a strategy.
const WIDTH: usize = 32;

/// How long a session lasts from the moment it is opened.
///
/// Twelve hours: a session opened in the morning does not ask again over lunch, and
/// one opened at midday is gone by the small hours rather than waiting for whoever
/// is next in the house.
pub const LASTS: Duration = Duration::from_secs(12 * 60 * 60);

/// One session: when it ends, and what it was opened against.
struct Session {
    /// The moment it stops being one.
    until: SystemTime,
    /// The credential that was on disk when it was opened.
    against: Credential,
}

/// Every session this run has opened.
#[derive(Default)]
pub struct Sessions {
    /// Held behind a lock rather than shared, because two requests arrive at once
    /// and one of them may be opening a session while the other is spending one.
    held: Mutex<HashMap<String, Session>>,
}

impl Sessions {
    /// Open one against the verifier that is on disk now, or nothing where this
    /// machine will not supply the secret to carry it by.
    ///
    /// The moment it ends is written the way every other instant this product
    /// writes one, so a client reads it with what it already has. A clock too far
    /// past the calendar to place leaves no session rather than one with no ending
    /// written on it.
    pub async fn opened(
        &self,
        random: &dyn Random,
        now: SystemTime,
        against: &Credential,
    ) -> Option<Admitted> {
        let token = crate::guard::minted(random, WIDTH)?;
        let until = now.checked_add(LASTS)?;
        let opened = Admitted::opened(token.clone(), until)?;
        let mut held = self.held.lock().await;
        held.retain(|_, session| session.until > now);
        held.insert(
            token.clone(),
            Session {
                until,
                against: against.clone(),
            },
        );
        Some(opened)
    }

    /// Whether this secret is a session that is still one.
    ///
    /// Against the verifier as it stands now rather than as it stood then: that is
    /// the whole of how a password change reaches a session somebody else is
    /// holding.
    pub async fn holds(
        &self,
        offered: Option<&str>,
        now: SystemTime,
        against: &Credential,
    ) -> bool {
        let Some(offered) = offered else {
            return false;
        };
        let mut held = self.held.lock().await;
        held.retain(|_, session| session.until > now);
        held.get(offered)
            .is_some_and(|session| &session.against == against)
    }
}

//! The challenges a device key signs, each good for one request within one minute.
//!
//! Kept in memory and nowhere else, so a restart ends every one of them. Each is bound to
//! the token that asked for it and to one device key, and is spent by the first request
//! presenting it, whatever that request turns out to prove.

use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;

use crate::keys::{digest, same};
use crate::ports::random::Random;

/// Bytes of randomness in a challenge.
const WIDTH: usize = 32;

/// How long a challenge stands once issued.
pub const STANDS: Duration = Duration::from_secs(60);

/// A challenge issued, as the phone is handed it.
#[derive(Clone, PartialEq, Eq)]
pub struct Issued {
    /// The challenge, in base64url.
    pub challenge: String,
    /// When it stops standing.
    pub expires: SystemTime,
}

/// What is said of an issued challenge: when it lapses, and not the challenge.
impl std::fmt::Debug for Issued {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Issued")
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

/// One challenge as it is held.
struct Held {
    /// The SHA-256 of the challenge, in lower-case hex.
    digest: String,
    /// The SHA-256 of the token that asked for it.
    asker: String,
    /// The device key it was issued for.
    device: String,
    /// When it stops standing.
    until: SystemTime,
}

/// Every challenge standing now.
#[derive(Default)]
pub struct Challenges {
    /// Behind a lock, so two requests presenting one challenge at once spend it once.
    held: Mutex<Vec<Held>>,
}

impl Challenges {
    /// Issue a challenge to whoever holds `token`, for the device key `device`, or
    /// nothing where this machine will not supply the randomness.
    ///
    /// A challenge already standing for the same token and device key is spent by
    /// issuing another, so each asker holds at most one per device key.
    pub async fn issued(
        &self,
        random: &dyn Random,
        token: &str,
        device: &str,
        now: SystemTime,
    ) -> Option<Issued> {
        let bytes = random.bytes(WIDTH).filter(|bytes| bytes.len() == WIDTH)?;
        let challenge = super::encoded(&bytes);
        let until = now.checked_add(STANDS)?;
        let asker = digest(token);
        let mut held = self.held.lock().await;
        held.retain(|one| one.until > now && !(one.asker == asker && one.device == device));
        held.push(Held {
            digest: digest(&challenge),
            asker,
            device: device.to_owned(),
            until,
        });
        Some(Issued {
            challenge,
            expires: until,
        })
    }

    /// Spend `offered`, and say whether it stood for whoever holds `token` and the
    /// device key `device` at `now`.
    ///
    /// Compared over every held challenge's every byte, whatever is found, so how long
    /// this takes says nothing about which challenge matched. A challenge presented is
    /// spent even where it stood for somebody else.
    pub async fn spent(&self, offered: &str, token: &str, device: &str, now: SystemTime) -> bool {
        let offered = digest(offered);
        let asker = digest(token);
        let mut held = self.held.lock().await;
        let found = held.iter().enumerate().fold(None, |found, (at, one)| {
            if same(&one.digest, &offered) && found.is_none() {
                Some(at)
            } else {
                found
            }
        });
        let stood = found.is_some_and(|at| {
            held.get(at).is_some_and(|one| {
                one.until > now && same(&one.asker, &asker) && one.device == device
            })
        });
        if let Some(at) = found {
            held.remove(at);
        }
        held.retain(|one| one.until > now);
        stood
    }
}

#[cfg(test)]
mod tests;

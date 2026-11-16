//! The sign-ins by a code the core waits on, and how many codes each source may ask for.
//!
//! Kept in memory and nowhere else, so a restart ends every one of them. The id the
//! identity source answered for a code is held here against a handle the core drew, and
//! is handed out only to be asked about for the session or device that asked for it.

use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::keys::{digest, same};
use crate::ports::random::Random;

/// Bytes of randomness in a handle.
const WIDTH: usize = 16;

/// How many leading bits of an IPv6 address name the network it came from.
const NETWORK_BITS: u32 = 64;

/// The least wait a refused ask is told of.
const LEAST_WAIT: Duration = Duration::from_secs(1);

/// How many codes each source may have waiting and ask for in a period, and whether a
/// code is finished only from the address that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Codes one source may have waiting at once.
    pub waiting: usize,
    /// Codes one source may ask for in each period.
    pub per_period: usize,
    /// That period.
    pub period: Duration,
    /// Whether a code is finished only from the network address that asked for it.
    pub by_address: bool,
}

/// Who asks: the digest of the credential the request carried, and the address it came
/// from, each where it is known.
#[derive(Clone, PartialEq, Eq)]
pub struct Asker {
    /// The SHA-256 of the session or other credential the request carried.
    session: Option<String>,
    /// The address the request came from.
    from: Option<IpAddr>,
}

impl Asker {
    /// The asker carrying `session`, where it carried one, from the address `from`.
    #[must_use]
    pub fn new(session: Option<&str>, from: Option<IpAddr>) -> Self {
        Self {
            session: session.map(digest),
            from: from.map(|from| from.to_canonical()),
        }
    }

    /// The source the limits count this asker under: its session, else the network its
    /// address is on, else the one source every asker of no known address shares.
    fn source(&self) -> Source {
        match (&self.session, self.from) {
            (Some(session), _) => Source::Session(session.clone()),
            (None, Some(from)) => Source::Network(network(from)),
            (None, None) => Source::Unplaced,
        }
    }

    /// Whether this asker may finish a code `asked` asked for under `limits`.
    fn finishes(&self, asked: &Self, limits: &Limits) -> bool {
        asked
            .session
            .as_ref()
            .is_none_or(|session| self.session.as_ref() == Some(session))
            && (!limits.by_address || self.from == asked.from)
    }
}

/// What a source of asks is counted by.
#[derive(Clone, PartialEq, Eq)]
enum Source {
    /// The SHA-256 of a session.
    Session(String),
    /// The network an address is on.
    Network(IpAddr),
    /// No session and no known address.
    Unplaced,
}

/// The network `from` is on: an IPv4 address itself, an IPv6 address's leading
/// [`NETWORK_BITS`].
fn network(from: IpAddr) -> IpAddr {
    match from {
        IpAddr::V4(_) => from,
        IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) & (u128::MAX << (128 - NETWORK_BITS))).into()),
    }
}

/// Why a code was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotKept {
    /// This machine would not supply the randomness a handle is drawn from.
    Undrawn,
    /// Its source has as many codes waiting as it may, for this long yet.
    Waiting(Duration),
}

/// One code waiting, as it is held.
struct Held {
    /// The SHA-256 of the handle, in lower-case hex.
    handle: String,
    /// The id the identity source answered for the code.
    id: String,
    /// Who asked for it.
    asker: Asker,
    /// When it lapses.
    lapses: SystemTime,
    /// Whether a finish of it is asking the identity source now.
    finishing: bool,
}

/// Every code waiting, and every ask still within its period.
#[derive(Default)]
struct Kept {
    /// The codes waiting.
    codes: Vec<Held>,
    /// Each ask, by its source, and when it was made.
    asked: Vec<(Source, SystemTime)>,
}

impl Kept {
    /// Let go of every code lapsed at `now` and every ask older than `period`.
    fn forget(&mut self, period: Duration, now: SystemTime) {
        self.codes.retain(|code| code.lapses > now);
        self.asked.retain(|(_, at)| {
            now.duration_since(*at).is_ok_and(|since| since < period) || *at > now
        });
    }

    /// How long `source` must wait to have another code waiting, where it has as many as
    /// `limits` lets it.
    fn full(&self, source: &Source, limits: &Limits, now: SystemTime) -> Option<Duration> {
        let lapsing: Vec<SystemTime> = self
            .codes
            .iter()
            .filter(|code| code.asker.source() == *source)
            .map(|code| code.lapses)
            .collect();
        (lapsing.len() >= limits.waiting).then(|| until(lapsing.iter().min().copied(), now))
    }

    /// How long `source` must wait to ask again, where it has asked as often as `limits`
    /// lets it in a period.
    fn spent(&self, source: &Source, limits: &Limits, now: SystemTime) -> Option<Duration> {
        let asked: Vec<SystemTime> = self
            .asked
            .iter()
            .filter(|(by, _)| by == source)
            .map(|(_, at)| *at + limits.period)
            .collect();
        (asked.len() >= limits.per_period).then(|| until(asked.iter().min().copied(), now))
    }
}

/// How long from `now` until `then`, at least [`LEAST_WAIT`].
fn until(then: Option<SystemTime>, now: SystemTime) -> Duration {
    then.and_then(|then| then.duration_since(now).ok())
        .unwrap_or_default()
        .max(LEAST_WAIT)
}

/// The limits the stack `ctx` holds: each as recorded, or its default where none is.
#[must_use]
pub fn limits(ctx: &crate::app::Ctx) -> Limits {
    crate::settings::sign_in_codes(ctx)
}

/// Every code to sign a member in by that the core waits on now.
#[derive(Default)]
pub struct SignInCodes {
    /// Behind a lock, so two requests finishing one code at once spend it once.
    kept: Mutex<Kept>,
}

/// A code claimed by one finish while it asks the identity source: the id to ask with and
/// when the code lapses. Every other finish of the code is refused until it is let go,
/// which dropping it does unless it was spent.
pub struct Finishing<'a> {
    /// The codes it was claimed from.
    codes: &'a SignInCodes,
    /// The SHA-256 of its handle.
    handle: String,
    /// The id the identity source answered for it.
    pub id: String,
    /// When it lapses.
    pub lapses: SystemTime,
}

impl Finishing<'_> {
    /// Let go of the code for good, once it answered a member.
    pub fn spent(self) {
        self.codes
            .held()
            .codes
            .retain(|code| !same(&code.handle, &self.handle));
    }
}

impl Drop for Finishing<'_> {
    fn drop(&mut self) {
        for code in &mut self.codes.held().codes {
            if same(&code.handle, &self.handle) {
                code.finishing = false;
            }
        }
    }
}

impl SignInCodes {
    /// The codes held, whatever a panic elsewhere left behind.
    fn held(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Count an ask by `asker` at `now`, or say how long it must wait: past the codes its
    /// source may have waiting, or past the asks it may make in a period.
    ///
    /// # Errors
    ///
    /// How long the source must wait.
    pub fn asking(&self, asker: &Asker, limits: &Limits, now: SystemTime) -> Result<(), Duration> {
        let source = asker.source();
        let mut kept = self.held();
        kept.forget(limits.period, now);
        let wait = kept
            .full(&source, limits, now)
            .into_iter()
            .chain(kept.spent(&source, limits, now))
            .max();
        if let Some(wait) = wait {
            return Err(wait);
        }
        kept.asked.push((source, now));
        Ok(())
    }

    /// Keep the id `id` for a code lapsing at `lapses` that `asker` asked for, under a
    /// fresh handle drawn from `random`, and answer the handle.
    ///
    /// # Errors
    ///
    /// Where no handle could be drawn, or the asker's source has as many codes waiting
    /// as `limits` lets it.
    pub fn kept(
        &self,
        random: &dyn Random,
        (id, asker, lapses): (String, Asker, SystemTime),
        limits: &Limits,
        now: SystemTime,
    ) -> Result<String, NotKept> {
        let drawn = random
            .bytes(WIDTH)
            .filter(|bytes| bytes.len() == WIDTH)
            .ok_or(NotKept::Undrawn)?;
        let handle = drawn.iter().fold(String::new(), |mut handle, byte| {
            use std::fmt::Write as _;
            let _ = write!(handle, "{byte:02x}");
            handle
        });
        let mut kept = self.held();
        kept.forget(limits.period, now);
        if let Some(wait) = kept.full(&asker.source(), limits, now) {
            return Err(NotKept::Waiting(wait));
        }
        kept.codes.push(Held {
            handle: digest(&handle),
            id,
            asker,
            lapses,
            finishing: false,
        });
        Ok(handle)
    }

    /// The code `handle` names, claimed for one finish, where it waits at `now`, no other
    /// finish holds it, and `asker` may finish it under `limits`.
    ///
    /// Compared over every held handle's every byte, whatever is found, so how long this
    /// takes says nothing about which handle matched.
    pub fn claimed(
        &self,
        handle: &str,
        asker: &Asker,
        limits: &Limits,
        now: SystemTime,
    ) -> Option<Finishing<'_>> {
        let offered = digest(handle);
        let mut kept = self.held();
        kept.forget(limits.period, now);
        let found = kept.codes.iter_mut().fold(None, |found, code| {
            if same(&code.handle, &offered) && found.is_none() {
                Some(code)
            } else {
                found
            }
        })?;
        if found.finishing || !asker.finishes(&found.asker, limits) {
            return None;
        }
        found.finishing = true;
        Some(Finishing {
            codes: self,
            handle: offered,
            id: found.id.clone(),
            lapses: found.lapses,
        })
    }
}

#[cfg(test)]
mod tests;

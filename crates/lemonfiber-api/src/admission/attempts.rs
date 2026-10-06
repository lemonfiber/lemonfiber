//! Counting wrong answers, so guessing costs time without anybody guessing costing
//! everybody else theirs.
//!
//! **Two limits, and each answers what the other cannot.** One is kept per address: a
//! few wrong answers are free, because typing one wrongly is what people do, and after
//! that each one doubles the wait up to a cap. Doubling is what turns a list of ten
//! thousand common passwords into a wait nobody sits through, and the cap is what stops
//! one afternoon of guessing from locking anybody out for a week. It reaches only the
//! address that earned it, so one device guessing keeps nobody else waiting.
//!
//! An address is cheap to change on a household network, though, so the second limit is
//! shared: a pool every wrong answer from an address not yet proved at that door draws
//! on, refilled faster than any one address may guess. One device, however it guesses,
//! never empties it. Many addresses at once can, and then only somebody signing in at a
//! door they have not used from that address waits; an operator or a member back at a
//! door they already opened this run is let through on the per-address limit alone.
//!
//! **A right answer forgives nothing but itself.** Forgetting every wrong answer at a
//! right one would let anybody holding one password guess at another between their own
//! sign-ins, so wrong answers are forgotten with time instead, one for every
//! [`FORGOTTEN_EVERY`] of quiet. Which door was proved matters for the same reason: a
//! member signing in proves the member's door, never the operator's.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;

use super::sessions::LASTS;

/// How many wrong answers cost nothing.
///
/// Three, which is a mistyped password, a forgotten capital, and one more.
const FREE: u32 = 3;

/// The longest wait a wrong answer can earn.
///
/// Five minutes. Long enough that guessing is hopeless — at one attempt per five
/// minutes a list of a thousand takes three and a half days — and short enough that
/// somebody who locked themselves out gets back in after a cup of tea rather than
/// after a support request.
const LONGEST: Duration = Duration::from_secs(5 * 60);

/// How long of quiet forgets one wrong answer.
///
/// The cap, so an address guessing as fast as it is allowed settles at one guess per
/// cap and no faster: each wait it sits out forgets the one wrong answer it is about
/// to add back.
const FORGOTTEN_EVERY: Duration = LONGEST;

/// The most a wait is doubled before the cap decides it.
///
/// Twenty doublings is twelve days, which is past the cap by a very long way, and it
/// is here so the shift is a number the type can hold rather than one that happens to
/// be small enough today.
const BEYOND: u32 = 20;

/// How many wrong answers the shared pool holds when full.
///
/// More than one address can spend before its own waits slow it to one guess per
/// [`LONGEST`]: three free, then eight doubling waits, is eleven in the first eight
/// and a half minutes, against eight refilled in that time.
const POOL: u32 = 30;

/// How often the shared pool gets one wrong answer back.
///
/// Once a minute, five times as fast as one address may guess once its waits are at
/// the cap, so no single address can drain it. What this allows many addresses
/// together is a guess a minute, at most, at any door.
const REFILLED_EVERY: Duration = Duration::from_secs(60);

/// A door a password or a key opens.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Door {
    /// The machine's own password.
    Operator,
    /// A key the operator minted for a program.
    Key,
    /// A household member's, by the name they sign in with, lower-cased the way the
    /// media server compares names.
    Member(String),
}

/// Where an attempt came from: the address it connected from, or nothing for a surface
/// answered without a socket, every request to which counts as one caller.
type Peer = Option<IpAddr>;

/// What one attempt may try, once it has been counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// Where it came from.
    peer: Peer,
    /// Whether the machine's own password may be tried.
    pub operator: bool,
    /// The member door it may try, where it named one and that door is open to it.
    pub member: Option<Door>,
    /// Whether it drew on the shared pool, which a right answer gives back.
    drew: bool,
}

/// What one address has got wrong.
#[derive(Debug, Clone, Copy)]
struct Wrong {
    /// How many, as of `last`.
    count: u32,
    /// When the count last changed.
    last: SystemTime,
}

impl Wrong {
    /// How many are still remembered at `now`, after the quiet since forgot some.
    fn remembered(self, now: SystemTime) -> u32 {
        let quiet = now.duration_since(self.last).unwrap_or_default();
        let forgotten = quiet.as_secs() / FORGOTTEN_EVERY.as_secs();
        self.count
            .saturating_sub(u32::try_from(forgotten).unwrap_or(u32::MAX))
    }

    /// How long is left before another answer is taken from this address.
    fn left(self, now: SystemTime) -> Option<Duration> {
        let since = now.duration_since(self.last).unwrap_or_default();
        owed(self.remembered(now))
            .checked_sub(since)
            .filter(|left| !left.is_zero())
    }
}

/// The wrong answers every address not yet proved at a door draws on.
#[derive(Debug, Default)]
struct Pool {
    /// How many are spent, as of `since`.
    spent: u32,
    /// When the last one came back, or nothing while none is spent.
    since: Option<SystemTime>,
}

impl Pool {
    /// Give back whatever the time since has earned.
    fn refill(&mut self, now: SystemTime) {
        let Some(since) = self.since else {
            return;
        };
        let elapsed = now.duration_since(since).unwrap_or_default();
        let back = elapsed.as_secs() / REFILLED_EVERY.as_secs();
        let back = u32::try_from(back).unwrap_or(u32::MAX);
        self.spent = self.spent.saturating_sub(back);
        self.since = (self.spent > 0)
            .then(|| since.checked_add(REFILLED_EVERY * back))
            .flatten();
    }

    /// Spend one, where one is left.
    fn drawn(&mut self, now: SystemTime) -> bool {
        if self.spent >= POOL {
            return false;
        }
        if self.spent == 0 {
            self.since = Some(now);
        }
        self.spent += 1;
        true
    }

    /// Give one back.
    fn returned(&mut self) {
        self.spent = self.spent.saturating_sub(1);
        if self.spent == 0 {
            self.since = None;
        }
    }

    /// How long until the next one comes back.
    fn next(&self, now: SystemTime) -> Duration {
        self.since.map_or(Duration::ZERO, |since| {
            REFILLED_EVERY.saturating_sub(now.duration_since(since).unwrap_or_default())
        })
    }
}

/// Everything counted, behind one lock.
#[derive(Default)]
struct Counted {
    /// What each address has got wrong.
    wrong: HashMap<Peer, Wrong>,
    /// The pool addresses not yet proved at a door draw on.
    pool: Pool,
    /// The doors each address has been proved at this run, and until when that counts.
    proved: HashMap<(Peer, Door), SystemTime>,
}

impl Counted {
    /// Forget what no longer bears on anything.
    fn tidied(&mut self, now: SystemTime) {
        self.wrong.retain(|_, wrong| wrong.remembered(now) > 0);
        self.proved.retain(|_, until| *until > now);
        self.pool.refill(now);
    }

    /// Whether this address has been proved at this door.
    fn proved(&self, peer: Peer, door: &Door) -> bool {
        self.proved.contains_key(&(peer, door.clone()))
    }

    /// Give back the one attempt `ticket` counted, and whatever it drew on the pool.
    fn given_back(&mut self, ticket: &Ticket) {
        if let Some(wrong) = self.wrong.get_mut(&ticket.peer) {
            wrong.count = wrong.count.saturating_sub(1);
        }
        if ticket.drew {
            self.pool.returned();
        }
    }
}

/// The wrong answers this run has been given.
#[derive(Default)]
pub struct Attempts {
    /// Behind a lock for the reason the sessions are: two answers can arrive at
    /// once, and a count that lost one of them would be a limit that could be
    /// stepped around by knocking twice.
    counted: Mutex<Counted>,
}

impl Attempts {
    /// How long is left before another answer is taken from this address, or nothing
    /// where one is.
    pub async fn waiting(
        &self,
        peer: impl Into<Option<IpAddr>>,
        now: SystemTime,
    ) -> Option<Duration> {
        let counted = self.counted.lock().await;
        counted.wrong.get(&canonical(peer.into()))?.left(now)
    }

    /// Take an attempt, or say how long is left before one is taken.
    ///
    /// The wait is read and the attempt counted under one lock, and counted as wrong
    /// before the answer is checked: a right one takes it back with [`Self::right`].
    /// Read and counted apart, every request arriving at once would pass the wait
    /// together and have its guess checked before any of them was counted.
    ///
    /// A door the shared pool keeps shut is left out of the attempt rather than
    /// refusing it, so a member signing in is not kept waiting because somebody else
    /// is guessing at the operator's password.
    ///
    /// # Errors
    ///
    /// How long is left, where the address has earned a wait or every door it asked
    /// at is shut.
    pub async fn taken(
        &self,
        peer: impl Into<Option<IpAddr>>,
        name: Option<&str>,
        now: SystemTime,
    ) -> Result<Ticket, Duration> {
        let peer = canonical(peer.into());
        let mut counted = self.counted.lock().await;
        counted.tidied(now);
        if let Some(left) = counted.wrong.get(&peer).and_then(|wrong| wrong.left(now)) {
            return Err(left);
        }
        let member = name
            .filter(|name| !name.is_empty())
            .map(|name| Door::Member(name.to_lowercase()));
        let operator_proved = counted.proved(peer, &Door::Operator);
        let member_proved = member
            .as_ref()
            .is_some_and(|door| counted.proved(peer, door));
        let drew =
            (!operator_proved || (member.is_some() && !member_proved)) && counted.pool.drawn(now);
        let ticket = Ticket {
            peer,
            operator: operator_proved || drew,
            member: member.filter(|_| member_proved || drew),
            drew,
        };
        if !ticket.operator && ticket.member.is_none() {
            return Err(counted.pool.next(now).max(Duration::from_secs(1)));
        }
        let count = counted
            .wrong
            .get(&peer)
            .map_or(0, |wrong| wrong.remembered(now));
        counted.wrong.insert(
            peer,
            Wrong {
                count: count.saturating_add(1),
                last: now,
            },
        );
        Ok(ticket)
    }

    /// Take an attempt at a key, or say how long is left before one is taken.
    ///
    /// The same two limits a password meets, and counted the same way: before the key is
    /// looked at, so every key arriving at once meets them together. An address that has
    /// presented a right key this run is let past the shared pool, as one back at a door
    /// it already opened is.
    ///
    /// # Errors
    ///
    /// How long is left, where the address has earned a wait or the shared pool is spent.
    pub async fn taken_at_a_key(
        &self,
        peer: impl Into<Option<IpAddr>>,
        now: SystemTime,
    ) -> Result<Ticket, Duration> {
        let peer = canonical(peer.into());
        let mut counted = self.counted.lock().await;
        counted.tidied(now);
        if let Some(left) = counted.wrong.get(&peer).and_then(|wrong| wrong.left(now)) {
            return Err(left);
        }
        let proved = counted.proved(peer, &Door::Key);
        let drew = !proved && counted.pool.drawn(now);
        if !proved && !drew {
            return Err(counted.pool.next(now).max(Duration::from_secs(1)));
        }
        let count = counted
            .wrong
            .get(&peer)
            .map_or(0, |wrong| wrong.remembered(now));
        counted.wrong.insert(
            peer,
            Wrong {
                count: count.saturating_add(1),
                last: now,
            },
        );
        Ok(Ticket {
            peer,
            operator: false,
            member: None,
            drew,
        })
    }

    /// Take back an attempt that proved `door`, and remember the address proved it.
    ///
    /// Only that attempt is taken back: a right answer at one door is no evidence about
    /// wrong ones anywhere else.
    pub async fn right(&self, ticket: &Ticket, door: Door, now: SystemTime) {
        let mut counted = self.counted.lock().await;
        counted.given_back(ticket);
        if let Some(until) = now.checked_add(LASTS) {
            counted.proved.insert((ticket.peer, door), until);
        }
    }

    /// Take back an attempt that turned out not to be a guess, proving nothing.
    ///
    /// For a key this machine minted and refuses — revoked, or a member's whose account
    /// has gone. It was never a guess, so it is not counted as one; and it opened
    /// nothing, so the address is no nearer the door than it was.
    pub async fn forgiven(&self, ticket: &Ticket) {
        self.counted.lock().await.given_back(ticket);
    }
}

/// One address however it arrived: an IPv4 caller reaching a dual-stack socket is
/// the same caller it is on an IPv4 one.
fn canonical(peer: Peer) -> Peer {
    peer.map(|at| at.to_canonical())
}

/// The wait a run of wrong answers has earned.
///
/// Nothing while they are free, then doubling, and never past the cap. Written over
/// the count rather than accumulated, so the wait is a function of what happened
/// rather than of what was recorded — and cannot drift from it.
fn owed(count: u32) -> Duration {
    match count.saturating_sub(FREE) {
        0 => Duration::ZERO,
        past => Duration::from_secs(1u64 << past.min(BEYOND)).min(LONGEST),
    }
}

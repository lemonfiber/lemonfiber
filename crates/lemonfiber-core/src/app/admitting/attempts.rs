//! The wrong answers this run has been given, and the waits they have earned.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;

use super::sessions::LASTS;

/// How many wrong answers cost nothing.
const FREE: u32 = 3;

/// The longest wait a wrong answer can earn.
const LONGEST: Duration = Duration::from_secs(5 * 60);

/// How long of quiet forgets one wrong answer.
const FORGOTTEN_EVERY: Duration = LONGEST;

/// The most a wait is doubled before the cap decides it.
const BEYOND: u32 = 20;

/// How many wrong answers the shared pool holds when full.
const POOL: u32 = 30;

/// How often the shared pool gets one wrong answer back.
const REFILLED_EVERY: Duration = Duration::from_secs(60);

/// Which door a right answer was given at.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Door {
    Operator,
    Key,
    Member(String),
}

type Peer = Option<IpAddr>;

/// Leave to try one answer, and which doors it may be tried at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ticket {
    peer: Peer,
    pub(crate) operator: bool,
    pub(crate) member: Option<Door>,
    drew: bool,
}

#[derive(Debug, Clone, Copy)]
struct Wrong {
    count: u32,
    last: SystemTime,
}

impl Wrong {
    fn remembered(self, now: SystemTime) -> u32 {
        let quiet = now.duration_since(self.last).unwrap_or_default();
        let forgotten = quiet.as_secs() / FORGOTTEN_EVERY.as_secs();
        self.count
            .saturating_sub(u32::try_from(forgotten).unwrap_or(u32::MAX))
    }

    fn left(self, now: SystemTime) -> Option<Duration> {
        let since = now.duration_since(self.last).unwrap_or_default();
        owed(self.remembered(now))
            .checked_sub(since)
            .filter(|left| !left.is_zero())
    }
}

#[derive(Debug, Default)]
struct Pool {
    spent: u32,
    since: Option<SystemTime>,
}

impl Pool {
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

    fn returned(&mut self) {
        self.spent = self.spent.saturating_sub(1);
        if self.spent == 0 {
            self.since = None;
        }
    }

    fn next(&self, now: SystemTime) -> Duration {
        self.since.map_or(Duration::ZERO, |since| {
            REFILLED_EVERY.saturating_sub(now.duration_since(since).unwrap_or_default())
        })
    }
}

#[derive(Default)]
struct Counted {
    wrong: HashMap<Peer, Wrong>,
    pool: Pool,
    proved: HashMap<(Peer, Door), SystemTime>,
}

impl Counted {
    fn tidied(&mut self, now: SystemTime) {
        self.wrong.retain(|_, wrong| wrong.remembered(now) > 0);
        self.proved.retain(|_, until| *until > now);
        self.pool.refill(now);
    }

    fn proved(&self, peer: Peer, door: &Door) -> bool {
        self.proved.contains_key(&(peer, door.clone()))
    }

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
    counted: Mutex<Counted>,
}

impl Attempts {
    /// How long `peer` must wait before its next answer is looked at, where it must.
    pub async fn waiting(
        &self,
        peer: impl Into<Option<IpAddr>>,
        now: SystemTime,
    ) -> Option<Duration> {
        let counted = self.counted.lock().await;
        counted.wrong.get(&canonical(peer.into()))?.left(now)
    }

    pub(crate) async fn taken(
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

    pub(crate) async fn taken_at_a_key(
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

    pub(crate) async fn right(&self, ticket: &Ticket, door: Door, now: SystemTime) {
        let mut counted = self.counted.lock().await;
        counted.given_back(ticket);
        if let Some(until) = now.checked_add(LASTS) {
            counted.proved.insert((ticket.peer, door), until);
        }
    }

    pub(crate) async fn forgiven(&self, ticket: &Ticket) {
        self.counted.lock().await.given_back(ticket);
    }
}

fn canonical(peer: Peer) -> Peer {
    peer.map(|at| at.to_canonical())
}

fn owed(count: u32) -> Duration {
    match count.saturating_sub(FREE) {
        0 => Duration::ZERO,
        past => Duration::from_secs(1u64 << past.min(BEYOND)).min(LONGEST),
    }
}

#[cfg(test)]
mod tests;

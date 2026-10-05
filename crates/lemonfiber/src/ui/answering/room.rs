//! How many connections one socket holds, counted per address.
//!
//! One ceiling for the socket keeps the process from holding more than it has room
//! for, and does nothing about who holds them: a single device opening every slot and
//! sending nothing leaves the surface answering nobody else. So each address is held to
//! a share of the ceiling, and a few slots are kept for this machine itself, which on a
//! surface offered to the household network arrives on the same socket as everybody
//! else and is the one place the operator can always reach it from.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

/// How the slots are shared out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) struct Shares {
    /// How many connections the socket holds at once, from everybody together.
    pub(in crate::ui) at_once: usize,
    /// How many one address may hold. This machine is held to the ceiling alone.
    pub(in crate::ui) each_peer: usize,
    /// How many of the ceiling only this machine may take.
    pub(in crate::ui) kept_for_here: usize,
}

/// What is held now.
#[derive(Default)]
struct Held {
    /// Every connection held, from anywhere.
    total: usize,
    /// How many each address holds.
    by_peer: HashMap<IpAddr, usize>,
}

/// The slots one socket gives out.
pub(super) struct Room {
    /// How they are shared out.
    shares: Shares,
    /// What is held now.
    held: Mutex<Held>,
}

/// One connection's slot, given back when it is dropped.
pub(super) struct Slot {
    /// Where it was taken from.
    room: Arc<Room>,
    /// Who took it.
    peer: IpAddr,
}

impl Room {
    /// A room sharing its slots out this way.
    pub(super) fn sharing(shares: Shares) -> Arc<Self> {
        Arc::new(Self {
            shares,
            held: Mutex::new(Held::default()),
        })
    }

    /// A slot for a connection from `peer`, or nothing where its share is gone.
    pub(super) fn taken(self: &Arc<Self>, peer: IpAddr) -> Option<Slot> {
        let peer = peer.to_canonical();
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let theirs = held.by_peer.get(&peer).copied().unwrap_or_default();
        let fits = if peer.is_loopback() {
            held.total < self.shares.at_once
        } else {
            held.total + self.shares.kept_for_here < self.shares.at_once
                && theirs < self.shares.each_peer
        };
        if !fits {
            return None;
        }
        held.total += 1;
        held.by_peer.insert(peer, theirs + 1);
        Some(Slot {
            room: Arc::clone(self),
            peer,
        })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut held = self
            .room
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        held.total = held.total.saturating_sub(1);
        let left = held
            .by_peer
            .get(&self.peer)
            .copied()
            .unwrap_or_default()
            .saturating_sub(1);
        if left == 0 {
            held.by_peer.remove(&self.peer);
        } else {
            held.by_peer.insert(self.peer, left);
        }
    }
}

#[cfg(test)]
mod tests;

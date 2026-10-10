//! How often each panel is read afresh, and how long a reading may take.
//!
//! The screen refreshes on [`super::TICK`], and not every source behind it is worth
//! asking that often. A service starting or a download moving is news within the
//! second; the tunnel's exit address, the household's requests and the disk's free
//! space move over minutes, and asking for them every second costs requests to
//! strangers, sign-ins to the media server and reads of every mount on the machine
//! — every second, for a figure that reads the same as it did a moment ago.
//!
//! So each slow panel is read at its own pace, and between readings the screen
//! carries the last one forward unchanged: a panel read at its own pace is current
//! until its next reading is due. Each reading also has a bound, so one source that
//! will not answer marks its own panel and leaves every other one refreshing.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

/// How long a panel's reading may take before the panel is reported unavailable.
///
/// Ten seconds, against reads that answer in well under one on a healthy stack. The
/// whole refresh waits on its slowest panel, so this is also the longest a frozen
/// source can hold the screen.
pub(crate) const PANEL_WITHIN: Duration = Duration::from_secs(10);

/// How long the household's reading may take.
///
/// Longer than any other panel's, because it is several services read one after
/// another — who holds an account, what each member asked for, what each library
/// holds — where every other panel is one or two requests.
pub(crate) const HOUSEHOLD_WITHIN: Duration = Duration::from_secs(30);

/// How often each curator's queue is read afresh.
pub(crate) const QUEUES_EVERY: Duration = Duration::from_secs(5);

/// How often the data volume's free space is read afresh.
pub(crate) const FREE_SPACE_EVERY: Duration = Duration::from_secs(30);

/// How often the hardlink probe writes to the data volume.
///
/// The probe writes, links and removes files, so it is the one reading that keeps a
/// library disk from ever spinning down; whether imports link changes only when the
/// mounts do.
pub(crate) const HARDLINK_EVERY: Duration = Duration::from_secs(10 * 60);

/// How often this machine is asked what it calls itself, for the door.
pub(crate) const DOOR_EVERY: Duration = Duration::from_secs(60);

/// How often the household's requests and standing are read afresh.
pub(crate) const HOUSEHOLD_EVERY: Duration = Duration::from_secs(60);

/// How often the tunnel is asked its exit address and country.
///
/// Every reading is a request to each IP-echo service from inside the tunnel, which
/// is a stranger being told this house's address. The address changes when the
/// tunnel reconnects, which is minutes apart at the very least.
pub(crate) const VPN_EVERY: Duration = Duration::from_secs(5 * 60);

/// A panel read at a pace of its own rather than on every refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Paced {
    /// Each curator's queue.
    Queues,
    /// The data volume's free space.
    FreeSpace,
    /// Whether imports hardlink.
    Hardlink,
    /// What this machine calls itself.
    Door,
    /// The household's requests and standing.
    Household,
    /// The tunnel's exit address, country and egress.
    Vpn,
}

impl Paced {
    /// Every paced panel.
    pub(crate) const ALL: [Self; 6] = [
        Self::Queues,
        Self::FreeSpace,
        Self::Hardlink,
        Self::Door,
        Self::Household,
        Self::Vpn,
    ];

    /// How long a reading of this panel stays current.
    pub(crate) const fn every(self) -> Duration {
        match self {
            Self::Queues => QUEUES_EVERY,
            Self::FreeSpace => FREE_SPACE_EVERY,
            Self::Hardlink => HARDLINK_EVERY,
            Self::Door => DOOR_EVERY,
            Self::Household => HOUSEHOLD_EVERY,
            Self::Vpn => VPN_EVERY,
        }
    }
}

/// When each paced panel was last read.
///
/// The time a reading was begun rather than when it answered, and recorded whether
/// it answered or not: a source that timed out is asked again at its own pace, not
/// on every refresh, which is what would let one dead source cost every refresh its
/// whole bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Readings(BTreeMap<Paced, SystemTime>);

impl Readings {
    /// Whether this panel is due to be read at `now`.
    ///
    /// Due where it has never been read, where its pace has passed, and where the
    /// clock reads earlier than the last reading: a clock set back is no evidence
    /// that the reading is still current.
    pub(crate) fn due(&self, panel: Paced, now: SystemTime) -> bool {
        self.0.get(&panel).is_none_or(|read| {
            now.duration_since(*read)
                .map_or(true, |since| since >= panel.every())
        })
    }

    /// Record that this panel was read at `now`.
    pub(crate) fn read(&mut self, panel: Paced, now: SystemTime) {
        self.0.insert(panel, now);
    }
}

#[cfg(test)]
mod tests;

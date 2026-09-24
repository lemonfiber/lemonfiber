//! What a household may ask for, in the one set of words every surface says it in.
//!
//! Two decisions and one arithmetic. The decision is whether what somebody asks for
//! arrives by itself, arrives up to a limit, or waits for the operator; the arithmetic
//! is what a period has already counted against them and when that count rolls off.
//!
//! **The request service holds both halves and neither is this product's invention.**
//! Whether a request arrives unseen is a permission on the account, and what a period
//! allows is a pair of numbers beside it — read at `GET /user/{id}/quota` and written at
//! `POST /user/{id}/settings/main` in `ghcr.io/seerr-team/seerr:v3.3.0`, which is where
//! these words were checked rather than recalled. So a policy here is a reading of what
//! that service will do, and a policy this file offered that it could not carry out
//! would be a promise made in the wrong building.
//!
//! **The period is a window that rolls, not a month that ends.** The service counts what
//! was asked for in the last so-many days, so nothing resets on a date — the earliest
//! request in the window ages out and one more becomes possible. That is why what is
//! said about a reset is derived from the requests themselves rather than from a
//! calendar: a household told "resets on the first" would be told something the service
//! does not do.
//!
//! One place for the words, because they are said three times: when the policy is
//! chosen, when a member's standing is reported beside their name, and when a request is
//! refused for want of room. Three copies would eventually disagree, and the place they
//! would disagree is the sentence somebody reads when they cannot ask for anything.

mod estimate;
mod expiry;
mod reasons;
mod refusal;
pub(crate) mod run;
mod window;

pub use estimate::{Estimate, FILM_HOURS, SEASON_HOURS};
pub use expiry::Expiry;
pub use reasons::{Reasons, Refused};
pub use refusal::{
    never_asked_here, no_limit_named, no_reason_given, no_such_policy, nobody_called,
    nothing_agreed, nothing_to_decide, sooner_than_the_reminder, unreachable, NEVER_HERE, NOBODY,
    NOTHING_AGREED, NOT_WAITING, NO_LIMIT, NO_REASON, NO_SUCH_POLICY, TOO_SOON, UNREACHABLE,
};
pub use window::{earliest, frees_up, waiting_for, REMINDING_AFTER};

use crate::ports::service::{Asking, Headroom, Left, Quota};

/// What happens to what a household member asks for.
///
/// Three, and they are the three the request service can actually be put into. A
/// household is in one of them because of two settings taken together — whether requests
/// arrive unseen, and whether a period limits how many — so the words here are a reading
/// of that pair rather than a fourth setting kept beside it.
///
/// Choosing per person is not a fourth policy. It is one of these three chosen for one
/// member rather than for the house, which is why what a surface offers is a policy and,
/// separately, who it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    /// Everything asked for arrives, and nothing is counted against anybody.
    Trusted,
    /// Everything asked for arrives until the period's limit is spent, and what
    /// would go past it is refused at the moment of asking.
    WithinALimit,
    /// Everything asked for waits for the operator to rule on it.
    EverythingWaits,
}

impl Policy {
    /// Every policy, in the order they are offered — most trusting first.
    pub const ALL: [Self; 3] = [Self::Trusted, Self::WithinALimit, Self::EverythingWaits];

    /// The policy the request service is in, from the two settings that decide it.
    #[must_use]
    pub const fn of(asking: &Asking) -> Self {
        match (asking.approves_own, asking.quota.is_some()) {
            (false, _) => Self::EverythingWaits,
            (true, true) => Self::WithinALimit,
            (true, false) => Self::Trusted,
        }
    }

    /// The word a surface names this policy by, and a caller writes to choose it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::WithinALimit => "within-a-limit",
            Self::EverythingWaits => "everything-waits",
        }
    }

    /// The policy a written word names, or nothing where no policy goes by it.
    #[must_use]
    pub fn from_label(written: &str) -> Option<Self> {
        let asked = written.trim().to_lowercase();
        Self::ALL.into_iter().find(|policy| policy.label() == asked)
    }

    /// Every word a surface may be given, for a refusal that names them.
    #[must_use]
    pub fn labels() -> String {
        Self::ALL
            .iter()
            .map(|policy| policy.label())
            .collect::<Vec<&str>>()
            .join(", ")
    }

    /// What choosing it comes to, in one line.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Trusted => "everything anybody asks for arrives, however much of it",
            Self::WithinALimit => {
                "everything arrives until somebody has used up their limit for the \
                 period, and what would go past it is refused as they ask"
            }
            Self::EverythingWaits => "nothing arrives until you have said yes to it",
        }
    }

    /// Whether choosing this policy needs a limit named alongside it.
    #[must_use]
    pub(crate) const fn needs_a_limit(self) -> bool {
        matches!(self, Self::WithinALimit)
    }

    /// Whether what somebody asks for arrives without anybody seeing it first.
    #[must_use]
    pub(crate) const fn arrives_unseen(self) -> bool {
        matches!(self, Self::Trusted | Self::WithinALimit)
    }
}

/// Where one member stands against what a period allows them.
///
/// Four, and the one worth having is the middle: somebody told only when they have run
/// out has been told too late to do anything but wait, which is the answer this whole
/// feature exists to avoid handing anybody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AskingStanding")]
pub enum Standing {
    /// Nothing is counted against them.
    Unlimited,
    /// There is room, and comfortably.
    WithinQuota,
    /// A fifth of the period's limit or less is left, and at least one of it.
    NearQuota,
    /// Nothing is left, so the next thing they ask for is refused as they ask.
    QuotaExhausted,
}

impl Standing {
    /// Where one kind's count leaves a member.
    ///
    /// Near is a fifth of the limit or less with at least one still to spend; a limit
    /// small enough that a fifth of it rounds to nought reads as near from its last one.
    #[must_use]
    pub const fn of(left: Left) -> Self {
        let Some(limit) = left.limit else {
            return Self::Unlimited;
        };
        match left.remaining() {
            None | Some(0) => Self::QuotaExhausted,
            Some(remaining) if remaining.saturating_mul(5) <= limit || remaining == 1 => {
                Self::NearQuota
            }
            Some(_) => Self::WithinQuota,
        }
    }

    /// Where a member stands taken over both counts, which is the worse of the two.
    ///
    /// The worse rather than an average: a household whose films are untouched and
    /// whose television is spent cannot ask for the next episode, and a line saying
    /// they are within their limit would be true of a request they cannot make.
    #[must_use]
    pub fn across(headroom: Headroom) -> Self {
        let (films, television) = (Self::of(headroom.films), Self::of(headroom.television));
        if films.rank() >= television.rank() {
            films
        } else {
            television
        }
    }

    /// How pressing this standing is, so two can be compared.
    const fn rank(self) -> u8 {
        match self {
            Self::Unlimited => 0,
            Self::WithinQuota => 1,
            Self::NearQuota => 2,
            Self::QuotaExhausted => 3,
        }
    }

    /// The plain phrase this reads as beside a member's name.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::Unlimited => "no limit",
            Self::WithinQuota => "within their limit",
            Self::NearQuota => "close to their limit",
            Self::QuotaExhausted => "has used their limit",
        }
    }

    /// Whether this is a standing the member has to be told about unprompted.
    #[must_use]
    pub const fn worth_saying(self) -> bool {
        matches!(self, Self::NearQuota | Self::QuotaExhausted)
    }
}

/// How a period reads, in the words a household says it in.
///
/// Seven days is a week and thirty is a month, because that is what somebody choosing
/// one meant; every other length is said as the days it is, rather than rounded into a
/// word that would be a different promise.
#[must_use]
pub fn period(days: u32) -> String {
    match days {
        1 => "a day".to_owned(),
        7 => "a week".to_owned(),
        30 => "a month".to_owned(),
        days => format!("{days} days"),
    }
}

/// How a limit reads, in one line: how many, over what period.
#[must_use]
pub fn limit(quota: Quota) -> String {
    // Two on an overflow, which reads as a plural — the branch is unreachable on any
    // machine this runs on, and a singular there would be the wrong one to guess.
    let many = crate::plural::s(usize::try_from(quota.requests).unwrap_or(2));
    format!("{} request{many} {}", quota.requests, within(quota.days))
}

/// How the window a count runs over reads, as a phrase to hang a limit on.
fn within(days: u32) -> String {
    match days {
        7 => "a week".to_owned(),
        30 => "a month".to_owned(),
        days => format!("every {}", period(days)),
    }
}

#[cfg(test)]
mod tests;

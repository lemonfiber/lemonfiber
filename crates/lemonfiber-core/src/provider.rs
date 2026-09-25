//! What the accounts the stack depends on have left — and whether that is knowable.
//!
//! A working login on an exhausted account authenticates perfectly and downloads
//! nothing. The stack is fine, every service is green, and the operator reasonably
//! concludes the software is broken: they restart things, re-run setup, and ask why
//! lemonfiber stopped working. The account is the problem, and only the account
//! knows it.
//!
//! So validity is necessary and insufficient, and this is the pure judgment over
//! the two facts that decide an account's health: whether it answered at all, and
//! what it has left. The distinctions are the whole point — a provider that refuses
//! a password, one that never answered, and one that answered and has nothing left
//! are three different problems with three different remedies, and collapsing them
//! is what sends an operator restarting services when their account needs topping up.
//!
//! Where an account exposes nothing usable about its capacity, that is reported as
//! unknown rather than estimated. An inferred figure treated as authoritative is
//! worse than an honest gap: it is the same false confidence, dressed as a
//! measurement. Nothing here reaches a provider; reading them is a separate concern.

pub mod trouble;

use serde::{Deserialize, Serialize};

use crate::validate::Validation;

/// How much warning a capacity that is running out earns, in days.
///
/// A week is long enough to top up an account or wait out a billing cycle, and short
/// enough that an account with months in it is not nagged about.
pub(crate) const NOTICE_DAYS: u64 = 7;

/// How much warning a subscription that is ending earns, in days.
///
/// Longer than the capacity horizon because a lapsed subscription is not topped up in
/// an afternoon: it is a payment, sometimes on an account whose card has expired, and
/// the operator may be away for a week of it.
pub(crate) const RENEWAL_NOTICE_DAYS: u64 = 14;

/// The share of an allowance, in percent, under which what is left is called low when
/// there is no observed consumption to project from.
///
/// The weaker signal of the two, and deliberately the fallback: a tenth of a large
/// account is weeks of headroom, so this only speaks where nothing has moved.
pub(crate) const LOW_WATER_PERCENT: u64 = 10;

/// What asking a provider established, before any arithmetic about capacity.
///
/// Several answers rather than "worked" and "failed", because they mean entirely
/// different things: a refusal is the provider saying the credential is wrong, and a
/// silence is it saying nothing at all. Reporting a timeout as a bad password sends
/// the operator to re-enter a credential that was fine.
///
/// And nobody having asked is its own answer, kept apart from a good one. A client's
/// records say what an account has left without anything being asked of the provider,
/// and those figures are worth reporting — but they are not evidence that the account
/// answers today, and a reading must not pass one off as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// It answered and accepted the credential.
    Answered,
    /// It authenticated and said it will not serve right now.
    Limited,
    /// It answered and said the account has nothing left to serve.
    Depleted,
    /// It answered and refused the credential.
    Refused,
    /// Nothing usable came back, so nothing is established either way.
    Silent,
    /// Nothing was asked of it — what is known comes from a client's own records.
    Unasked,
}

impl From<&Validation> for Answer {
    /// Proving a credential already answers this question, so the two vocabularies
    /// meet here once rather than at each place a provider is read — which is also
    /// what keeps a timeout from ever being read as a rejection.
    fn from(validation: &Validation) -> Self {
        match validation {
            Validation::Valid { .. } => Self::Answered,
            Validation::Degraded { .. } => Self::Limited,
            Validation::Rejected { .. } => Self::Refused,
            Validation::Unreachable { .. } => Self::Silent,
        }
    }
}

/// Whether a provider's allowance comes back on its own.
///
/// The axis that decides what empty means, and a property of the provider rather than
/// of any figure read from it — which is why it is stated once per provider and holds
/// even when the provider gives no figures at all. A block account is bought once and
/// spent down, so empty is `exhausted` and the remedy is to top it up; an indexer's
/// daily call limit refills, so empty is `capped` and the remedy is to wait. Telling
/// an operator to buy more of something that returns at midnight is the same failure
/// as telling them to wait for something that never will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Renewal {
    /// Bought once and spent down — a Usenet block account's data.
    Bought,
    /// Refills on its own — an indexer's daily API or grab limit.
    Refills,
}

impl Renewal {
    /// What an empty allowance of this kind amounts to.
    #[must_use]
    pub(crate) const fn when_empty(self) -> Health {
        match self {
            Self::Bought => Health::Exhausted,
            Self::Refills => Health::Capped,
        }
    }
}

/// How fast an allowance is being spent, from what was actually observed.
///
/// A rate rather than a total, because "how much is left" only becomes "when does it
/// stop" once you know how fast it is going. Constructed from a measured window, so a
/// projection can always name what it was projected from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Burn {
    per_day: u64,
}

impl Burn {
    /// The rate `spent` over `days` amounts to, or nothing where there is no rate to
    /// take: a window of no days measures nothing, and a window in which nothing
    /// moved is not a slow rate but an absence of one — projecting from it would put
    /// a date on an account nobody is using.
    #[must_use]
    pub const fn over(spent: u64, days: u64) -> Option<Self> {
        if days == 0 || spent == 0 {
            return None;
        }
        Some(Self {
            per_day: spent / days,
        })
    }

    /// How long `remaining` lasts at this rate, rounded down so the answer is the
    /// pessimistic one. A rate under a whole unit a day rounds to zero and leaves
    /// nothing to project from, rather than dividing by it.
    #[must_use]
    pub(crate) const fn days_for(self, remaining: u64) -> Option<u64> {
        if self.per_day == 0 {
            return None;
        }
        Some(remaining / self.per_day)
    }
}

/// An account's allowance, as far as it is knowable.
///
/// The cap is optional because usage is knowable without it and often is: an indexer
/// answers every query without ever publishing how many it allows. Usage with no cap
/// is a real observation and is kept as one — what it is not is a capacity judgment,
/// and it is never turned into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowance {
    /// How much of it has been spent, in whatever the allowance counts.
    pub used: u64,
    /// The whole of it, where the provider or its client states one.
    pub cap: Option<u64>,
    /// The observed rate it is going at, where consumption has been seen.
    pub burn: Option<Burn>,
}

impl Allowance {
    /// What is left, where a cap is known. Saturating, because a provider that
    /// reports more used than the cap has overshot rather than gone negative — and
    /// overshot is empty.
    #[must_use]
    pub fn remaining(&self) -> Option<u64> {
        self.cap.map(|cap| cap.saturating_sub(self.used))
    }

    /// Whether it is provably spent — a known cap with nothing left. An unknown cap
    /// is never spent, because nothing is known about it either way.
    #[must_use]
    pub fn spent(&self) -> bool {
        self.remaining() == Some(0)
    }

    /// How many days it lasts at the observed rate, where both what is left and a
    /// rate to spend it at are known.
    #[must_use]
    pub(crate) fn days_left(&self) -> Option<u64> {
        let remaining = self.remaining()?;
        self.burn?.days_for(remaining)
    }

    /// Whether what is left is under the low-water share of the whole — the weaker
    /// signal, for when nothing has moved and there is no rate to project from.
    fn low(&self) -> bool {
        let (Some(cap), Some(remaining)) = (self.cap, self.remaining()) else {
            return false;
        };
        // Multiplied out rather than divided, so a small allowance is not rounded
        // into or out of the warning by integer division.
        remaining.saturating_mul(100) < cap.saturating_mul(LOW_WATER_PERCENT)
    }
}

/// What a provider's account is doing, in the operator's terms.
///
/// One state per provider, because the operator's question is one question. The
/// figures behind it — what is left, when it resets, when it lapses — travel beside
/// it rather than inside it: a state says what to do about the account, and the
/// numbers say why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Health {
    /// Reachable, authenticated, and with capacity to spare.
    Healthy,
    /// Capacity is running out, with time left to act.
    Depleting,
    /// Authenticated with nothing left, and it will not come back on its own.
    Exhausted,
    /// A refilling limit is reached; it comes back when the provider resets it.
    Capped,
    /// The provider answered and refused the credential.
    Invalid,
    /// Nothing answered, so nothing is established.
    Unreachable,
    /// It is reachable and authenticated, and exposes nothing usable about capacity.
    Unknown,
    /// The subscription behind it ends soon.
    Expiring,
}

impl Health {
    /// The state's stored name — the plain term a report names it under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Depleting => "depleting",
            Self::Exhausted => "exhausted",
            Self::Capped => "capped",
            Self::Invalid => "invalid",
            Self::Unreachable => "unreachable",
            Self::Unknown => "unknown",
            Self::Expiring => "expiring",
        }
    }
}

/// Everything observed about one provider, and the verdict it comes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// What asking it established.
    pub answer: Answer,
    /// Whether its allowance comes back on its own.
    pub renewal: Renewal,
    /// Its allowance, where anything about one could be read.
    pub allowance: Option<Allowance>,
    /// Days until the subscription behind it ends, where one is recorded.
    pub expires_in: Option<u64>,
}

impl Reading {
    /// The state this reading comes to.
    ///
    /// What the provider said decides first: an account that never answered has told
    /// you nothing about its capacity, and one that refused the credential has a
    /// problem no top-up fixes. Only then does what is left matter.
    ///
    /// A provider that will not serve right now is only conclusive where its allowance
    /// refills — an indexer saying so is at its daily cap, and there is nothing else it
    /// could mean. A Usenet provider saying so is usually at its *connection* limit,
    /// which is a client configured above the plan rather than an empty block, and is
    /// reported as the configuration mismatch it is. So its word is taken where it is
    /// unambiguous, and its figures are read where it is not.
    ///
    /// A provider that says the account has nothing left is believed over any figures
    /// held about it, because the figures are only ever a client's own record: a block
    /// nobody wrote down, or one topped up somewhere the client never heard about, both
    /// read as capacity that is fine. The account itself is the authority on being empty.
    ///
    /// Silence is the one answer an allowance can outrank. It is a fact about the
    /// connection rather than about the account, and an account whose allowance is
    /// provably gone stops serving whatever the connection does — so where both are
    /// true, the operator is told the one with the remedy that brings downloads back
    /// rather than sent to check a network that was never the problem.
    #[must_use]
    pub fn health(&self) -> Health {
        let empty = self.allowance.is_some_and(|allowance| allowance.spent());
        match self.answer {
            Answer::Silent if !empty => return Health::Unreachable,
            Answer::Refused => return Health::Invalid,
            Answer::Depleted => return self.renewal.when_empty(),
            Answer::Limited if self.renewal == Renewal::Refills => return Health::Capped,
            Answer::Silent | Answer::Limited | Answer::Answered | Answer::Unasked => {}
        }
        if empty {
            return self.renewal.when_empty();
        }
        self.running_out().unwrap_or(Health::Unknown)
    }

    /// The nearer of the two warnings, where either applies.
    ///
    /// Both are "act before it stops", so one has to be chosen, and the one that can
    /// be dated wins: a subscription ends on a known day, while an allowance called
    /// low by its share alone has no date to it. Between two dated deadlines the
    /// nearer one wins, which is the one that will actually bite first.
    fn running_out(&self) -> Option<Health> {
        let empty_in = self
            .allowance
            .and_then(|allowance| allowance.days_left())
            .filter(|days| *days <= NOTICE_DAYS);
        let ends_in = self.expires_in.filter(|days| *days <= RENEWAL_NOTICE_DAYS);

        match (empty_in, ends_in) {
            (Some(empty), Some(ends)) if empty <= ends => Some(Health::Depleting),
            (_, Some(_)) => Some(Health::Expiring),
            (Some(_), None) => Some(Health::Depleting),
            (None, None) => self.settled(),
        }
    }

    /// What a provider with no deadline in sight amounts to: healthy where its
    /// capacity is known and holding, low where its share says so with nothing moving
    /// to project from, and unknown where it publishes no capacity at all.
    fn settled(&self) -> Option<Health> {
        let allowance = self.allowance?;
        if allowance.low() {
            return Some(Health::Depleting);
        }
        allowance.remaining().map(|_| Health::Healthy)
    }
}

#[cfg(test)]
mod tests;

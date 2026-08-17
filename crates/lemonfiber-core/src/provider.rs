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
pub const NOTICE_DAYS: u64 = 7;

/// How much warning a subscription that is ending earns, in days.
///
/// Longer than the capacity horizon because a lapsed subscription is not topped up in
/// an afternoon: it is a payment, sometimes on an account whose card has expired, and
/// the operator may be away for a week of it.
pub const RENEWAL_NOTICE_DAYS: u64 = 14;

/// The share of an allowance, in percent, under which what is left is called low when
/// there is no observed consumption to project from.
///
/// The weaker signal of the two, and deliberately the fallback: a tenth of a large
/// account is weeks of headroom, so this only speaks where nothing has moved.
pub const LOW_WATER_PERCENT: u64 = 10;

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
    pub const fn when_empty(self) -> Health {
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
    pub const fn days_for(self, remaining: u64) -> Option<u64> {
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
    pub fn days_left(&self) -> Option<u64> {
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

    /// Whether the state is one the operator has to do something about.
    ///
    /// `unknown` is deliberately not: a provider that publishes nothing about its
    /// capacity is the ordinary case, not a fault, and treating it as one would make
    /// the whole check noise the operator learns to skip past.
    #[must_use]
    pub const fn wants_attention(self) -> bool {
        !matches!(self, Self::Healthy | Self::Unknown)
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
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    /// An allowance of `cap` with `used` gone, spent at `per_week` over the last week.
    fn block(cap: Option<u64>, used: u64, per_week: u64) -> Allowance {
        Allowance {
            used,
            cap,
            burn: Burn::over(per_week, 7),
        }
    }

    /// A block account that answered, since that is the shape most of these judge.
    fn answered(allowance: Option<Allowance>) -> Reading {
        Reading {
            answer: Answer::Answered,
            renewal: Renewal::Bought,
            allowance,
            expires_in: None,
        }
    }

    /// The distinction the whole feature rests on: what a provider said about the
    /// credential decides reachability, and an account that authenticated but cannot
    /// serve is still an account that answered.
    #[test]
    fn a_refusal_a_silence_and_a_limit_are_three_different_answers() {
        let said = |validation: &Validation| Answer::from(validation);
        assert_eq!(
            said(&Validation::Valid {
                observed: "answered a search — 40 results".to_owned(),
            }),
            Answer::Answered
        );
        assert_eq!(
            said(&Validation::Degraded {
                detail: "the daily request limit is reached".to_owned(),
            }),
            Answer::Limited
        );
        assert_eq!(
            said(&Validation::Rejected {
                detail: "the key was refused".to_owned(),
            }),
            Answer::Refused
        );
        assert_eq!(
            said(&Validation::Unreachable {
                detail: "the connection timed out".to_owned(),
            }),
            Answer::Silent
        );
    }

    #[test]
    fn nothing_moving_is_an_absence_of_a_rate_rather_than_a_slow_one() {
        assert_eq!(Burn::over(0, 7), None);
        assert_eq!(Burn::over(GIB, 0), None);
        assert!(Burn::over(GIB, 7).is_some());
    }

    #[test]
    fn a_rate_under_a_whole_unit_a_day_projects_nothing_rather_than_dividing_by_it() {
        let crawling = Burn::over(3, 7);
        assert!(crawling.is_some(), "three units over a week is a rate");
        assert_eq!(crawling.and_then(|rate| rate.days_for(100)), None);
    }

    #[test]
    fn a_projection_rounds_down_to_the_pessimistic_answer() {
        let weekly = Burn::over(7 * GIB, 7);
        assert_eq!(
            weekly.and_then(|rate| rate.days_for(10 * GIB + GIB / 2)),
            Some(10)
        );
    }

    #[test]
    fn an_account_reporting_more_used_than_it_holds_is_empty_rather_than_negative() {
        let overshot = block(Some(10 * GIB), 12 * GIB, 0);
        assert_eq!(overshot.remaining(), Some(0));
        assert!(overshot.spent());
    }

    #[test]
    fn an_unknown_cap_leaves_what_is_left_unknown_rather_than_spent() {
        let usage_only = block(None, 400, 0);
        assert_eq!(usage_only.remaining(), None);
        assert!(!usage_only.spent());
        assert_eq!(usage_only.days_left(), None);
        assert!(!usage_only.low());
    }

    #[test]
    fn what_is_left_projects_only_where_something_has_been_seen_moving() {
        assert_eq!(block(Some(100 * GIB), 0, 0).days_left(), None);
        assert_eq!(
            block(Some(100 * GIB), 30 * GIB, 7 * GIB).days_left(),
            Some(70)
        );
    }

    /// The reason the share is only the fallback: a tenth of a large account is weeks
    /// of headroom, and the same tenth of a small one is an afternoon.
    #[test]
    fn the_low_water_share_speaks_only_when_the_account_is_genuinely_near_empty() {
        assert!(block(Some(100 * GIB), 95 * GIB, 0).low());
        assert!(!block(Some(100 * GIB), 80 * GIB, 0).low());
    }

    #[test]
    fn nothing_answered_says_nothing_about_capacity() {
        let silent = Reading {
            answer: Answer::Silent,
            expires_in: Some(1),
            ..answered(Some(block(Some(100 * GIB), 0, 0)))
        };
        assert_eq!(silent.health(), Health::Unreachable);
    }

    /// Both are true at once often enough to matter, and only one of them has a remedy
    /// that brings downloads back: an account with nothing left stops serving whatever
    /// the connection to it is doing.
    #[test]
    fn an_allowance_provably_gone_outranks_a_provider_that_says_nothing() {
        let silent_and_empty = Reading {
            answer: Answer::Silent,
            ..answered(Some(block(Some(100 * GIB), 100 * GIB, 0)))
        };
        assert_eq!(silent_and_empty.health(), Health::Exhausted);
    }

    #[test]
    fn a_refused_credential_is_not_a_capacity_problem() {
        let refused = Reading {
            answer: Answer::Refused,
            ..answered(Some(block(Some(100 * GIB), 0, 0)))
        };
        assert_eq!(refused.health(), Health::Invalid);
    }

    /// The distinction that decides the remedy: one is topped up, the other waited out.
    #[test]
    fn an_empty_allowance_that_returns_is_capped_and_one_that_does_not_is_exhausted() {
        let block_account = answered(Some(block(Some(50 * GIB), 50 * GIB, GIB)));
        assert_eq!(block_account.health(), Health::Exhausted);

        let daily_calls = Reading {
            renewal: Renewal::Refills,
            ..answered(Some(block(Some(500), 500, 0)))
        };
        assert_eq!(daily_calls.health(), Health::Capped);
    }

    /// The provider's own word outranks arithmetic over figures read elsewhere: a
    /// limit nobody publishes is invisible to us and plain to it, and a client's
    /// counters can be stale.
    #[test]
    fn a_refusal_to_serve_is_conclusive_only_where_it_can_mean_one_thing() {
        let indexer = Reading {
            answer: Answer::Limited,
            renewal: Renewal::Refills,
            ..answered(Some(block(Some(100 * GIB), 0, 0)))
        };
        assert_eq!(indexer.health(), Health::Capped);

        // A Usenet provider saying so is usually at its connection limit, which is a
        // client configured above the plan rather than a block with nothing left.
        let busy_account = Reading {
            answer: Answer::Limited,
            ..answered(Some(block(Some(100 * GIB), 0, 0)))
        };
        assert_eq!(busy_account.health(), Health::Healthy);

        let spent_account = Reading {
            answer: Answer::Limited,
            ..answered(Some(block(Some(100 * GIB), 100 * GIB, 0)))
        };
        assert_eq!(spent_account.health(), Health::Exhausted);
    }

    /// Figures from a client's records are worth reporting without anything having
    /// been asked of the provider — they just are not evidence that it answers today.
    #[test]
    fn an_account_nobody_asked_is_still_judged_on_what_it_has_left() {
        let unasked = Reading {
            answer: Answer::Unasked,
            ..answered(Some(block(Some(100 * GIB), 95 * GIB, 0)))
        };
        assert_eq!(unasked.health(), Health::Depleting);
    }

    /// The case the design exists for: a seventh of a large account left is not a low
    /// share by any reading, and at the rate of the last week it is five days.
    #[test]
    fn an_account_running_out_within_the_week_is_depleting_however_much_is_left() {
        let going = block(Some(1000 * GIB), 850 * GIB, 210 * GIB);
        assert!(!going.low());
        assert_eq!(going.days_left(), Some(5));
        assert_eq!(answered(Some(going)).health(), Health::Depleting);
    }

    #[test]
    fn an_account_with_months_in_it_is_healthy() {
        assert_eq!(
            answered(Some(block(Some(1000 * GIB), 100 * GIB, 7 * GIB))).health(),
            Health::Healthy
        );
    }

    #[test]
    fn an_account_low_on_share_alone_is_depleting_even_with_nothing_moving() {
        assert_eq!(
            answered(Some(block(Some(100 * GIB), 95 * GIB, 0))).health(),
            Health::Depleting
        );
    }

    #[test]
    fn an_account_that_publishes_no_capacity_is_unknown_rather_than_healthy() {
        assert_eq!(answered(None).health(), Health::Unknown);
        assert_eq!(
            answered(Some(block(None, 400, 0))).health(),
            Health::Unknown
        );
    }

    /// Both deadlines are "act before it stops", so the one that bites first wins —
    /// and an undated one never outranks a date.
    #[test]
    fn the_nearer_deadline_is_the_one_reported() {
        let expiry_first = Reading {
            expires_in: Some(2),
            ..answered(Some(block(Some(100 * GIB), 93 * GIB, 7 * GIB)))
        };
        assert_eq!(expiry_first.health(), Health::Expiring);

        let capacity_first = Reading {
            expires_in: Some(10),
            ..answered(Some(block(Some(100 * GIB), 97 * GIB, 7 * GIB)))
        };
        assert_eq!(capacity_first.health(), Health::Depleting);

        let undated_low = Reading {
            expires_in: Some(10),
            ..answered(Some(block(Some(100 * GIB), 95 * GIB, 0)))
        };
        assert_eq!(undated_low.health(), Health::Expiring);
    }

    #[test]
    fn a_subscription_ending_beyond_the_notice_is_not_warned_about() {
        let far_off = Reading {
            expires_in: Some(RENEWAL_NOTICE_DAYS + 1),
            ..answered(Some(block(Some(100 * GIB), 0, 0)))
        };
        assert_eq!(far_off.health(), Health::Healthy);
    }

    #[test]
    fn a_subscription_ending_soon_is_reported_even_with_no_capacity_to_read() {
        let ending = Reading {
            expires_in: Some(3),
            ..answered(None)
        };
        assert_eq!(ending.health(), Health::Expiring);
    }

    /// The machine-readable names are a contract, so they are pinned rather than left
    /// to whatever the enum happens to serialise as.
    #[test]
    fn every_state_names_itself_the_way_it_is_reported() {
        for (health, name) in [
            (Health::Healthy, "healthy"),
            (Health::Depleting, "depleting"),
            (Health::Exhausted, "exhausted"),
            (Health::Capped, "capped"),
            (Health::Invalid, "invalid"),
            (Health::Unreachable, "unreachable"),
            (Health::Unknown, "unknown"),
            (Health::Expiring, "expiring"),
        ] {
            assert_eq!(health.label(), name);
            assert_eq!(
                serde_json::to_string(&health).unwrap_or_default(),
                format!("\"{name}\"")
            );
        }
    }

    /// A provider that publishes nothing is the ordinary case, not a fault — a check
    /// that flagged every one of them would be noise the operator learns to skip.
    #[test]
    fn only_the_states_with_something_to_do_want_attention() {
        assert!(!Health::Healthy.wants_attention());
        assert!(!Health::Unknown.wants_attention());
        for health in [
            Health::Depleting,
            Health::Exhausted,
            Health::Capped,
            Health::Invalid,
            Health::Unreachable,
            Health::Expiring,
        ] {
            assert!(health.wants_attention());
        }
    }

    #[test]
    fn a_renewal_survives_a_round_trip_through_its_name() {
        for renewal in [Renewal::Bought, Renewal::Refills] {
            let json = serde_json::to_string(&renewal).unwrap_or_default();
            assert_eq!(
                serde_json::from_str::<Renewal>(&json).ok(),
                Some(renewal),
                "{json} should read back"
            );
        }
    }
}

//! What a Usenet account has left, as its download client records it.
//!
//! The client is the only place these figures exist: a provider publishes no quota,
//! so the block that was bought is recorded in the client and every byte pulled is
//! measured there. What the client cannot say is whether the provider answers this
//! morning — a hundred gigabytes pulled last week proves nothing about now — so the
//! reading is marked as one nobody asked the provider to confirm, rather than passing
//! a record off as a reply.

use lemonfiber_manifest::Date;

use crate::bytes::humanize;
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::plural;
use crate::ports::service::UsenetAccount;
use crate::provider::{Allowance, Answer, Burn, Health, Reading, Renewal};

use super::{PROVIDER_EMPTY, PROVIDER_ENDING, PROVIDER_LOW};

/// How many days back a consumption rate is taken over.
///
/// A week smooths the difference between a quiet Tuesday and a season landing at the
/// weekend, which a single day does not, while staying recent enough to describe how
/// the account is being used now rather than last month.
pub(super) const WINDOW_DAYS: u64 = 7;

/// What a Usenet account amounts to, as of `today`.
///
/// The allowance is what has gone against the recorded block rather than the lifetime
/// figure: a block bought halfway through an account's life is only readable against
/// what had already been pulled when it was recorded. With no block recorded, the
/// lifetime figure is still reported — it is a real observation — but nothing is
/// concluded from it, because there is nothing to conclude it against.
pub(super) fn reading(account: &UsenetAccount, today: Date) -> Reading {
    Reading {
        // Nothing was asked of the provider itself: these are the client's records.
        answer: Answer::Unasked,
        renewal: Renewal::Bought,
        allowance: Some(allowance_of(account, today)),
        expires_in: expires_in(account.expires_on, today),
    }
}

/// What has gone against the recorded block, and how fast it is going.
fn allowance_of(account: &UsenetAccount, today: Date) -> Allowance {
    Allowance {
        used: account.quota.map_or(account.downloaded, |quota| {
            account.downloaded.saturating_sub(quota.from)
        }),
        cap: account.quota.map(|quota| quota.cap),
        burn: burn(&account.daily, today),
    }
}

/// Days until the subscription ends, floored at zero: one that ended last week has
/// not stopped mattering, and reporting no expiry for it would be the one reading
/// worse than reporting none at all.
fn expires_in(expires_on: Option<Date>, today: Date) -> Option<u64> {
    expires_on.map(|end| u64::try_from(today.days_until(end)).unwrap_or(0))
}

/// The rate the account is being spent at, over the days the client has a record for.
///
/// Today is left out: it is a part-day, and averaging it in would report a rate lower
/// than the account is really going at, which errs towards telling the operator late.
/// The window is divided by the days actually covered rather than its own width, so a
/// client that has only been running for two of them describes those two rather than
/// spreading them over a week it did not measure.
fn burn(daily: &[(Date, u64)], today: Date) -> Option<Burn> {
    let mut spent = 0_u64;
    let mut earliest: Option<Date> = None;
    for (day, bytes) in daily {
        let Ok(ago) = u64::try_from(day.days_until(today)) else {
            continue;
        };
        if ago == 0 || ago > WINDOW_DAYS {
            continue;
        }
        spent = spent.saturating_add(*bytes);
        earliest = Some(earliest.map_or(*day, |first| first.min(*day)));
    }
    let days = u64::try_from(earliest?.days_until(today)).ok()?;
    Burn::over(spent, days)
}

/// A count of days as a sentence reads it.
fn days_reading(count: u64) -> String {
    // A count too large for a usize is not one, so it takes the plural either way.
    format!(
        "{count} day{}",
        plural::s(usize::try_from(count).unwrap_or(2))
    )
}

/// One account's finding: what it has left, and what to do about it.
///
/// The figures travel with every verdict, passing ones included. "340 GiB left, about
/// three weeks at last week's rate" is what lets an operator judge for themselves
/// whether a warning threshold set by somebody else suits how they use the account.
pub(super) fn finding(account: &UsenetAccount, today: Date) -> Finding {
    let reading = reading(account, today);
    let allowance = allowance_of(account, today);
    let verdict = match reading.health() {
        Health::Exhausted => Verdict::Fail(empty(allowance)),
        Health::Depleting => Verdict::Warn(low(allowance)),
        Health::Expiring => Verdict::Warn(ending(reading.expires_in.unwrap_or(0))),
        // Nothing was asked of the provider, so no answer of its own can arrive here;
        // capacity that is fine, or that says nothing, both read as nothing to do.
        _ => Verdict::Pass {
            note: Some(left(allowance)),
        },
    };
    Finding::in_category(
        Category::Providers,
        &format!("providers.usenet.{}", account.name),
        &account.name,
        verdict,
    )
}

/// What the account has left, in the operator's terms — and where a rate was observed,
/// how long that lasts. An account with no allowance recorded says what it has pulled,
/// which is the only true thing there is to say about it.
fn left(allowance: Allowance) -> String {
    let Some(cap) = allowance.cap else {
        return format!(
            "{} pulled; no allowance is recorded, so there is none to measure it against",
            humanize(allowance.used)
        );
    };
    let of_the_whole = format!(
        "{} left of {}",
        humanize(cap.saturating_sub(allowance.used)),
        humanize(cap)
    );
    match allowance.days_left() {
        None => of_the_whole,
        Some(days) => format!(
            "{of_the_whole} — about {} at the rate of the last week",
            days_reading(days)
        ),
    }
}

/// A block account with nothing left. An error rather than a warning: nothing
/// downloads through it until it is topped up, and no amount of waiting changes that.
fn empty(allowance: Allowance) -> Problem {
    Problem::new(
        PROVIDER_EMPTY,
        Severity::Error,
        "A Usenet account has nothing left",
        "The account authenticates perfectly and can download nothing, which looks exactly like a broken stack from the outside. A block account does not refill on its own.",
        Remedy::new("Top the account up, or point the client at one that has data left"),
    )
    .in_state(State::Guided)
    .with_detail(left(allowance))
}

/// An account running out, while there is still time to act — which is the whole
/// point of saying it now rather than when it has already stopped.
fn low(allowance: Allowance) -> Problem {
    Problem::new(
        PROVIDER_LOW,
        Severity::Warning,
        "A Usenet account is running out",
        "At the rate it is being used, this account runs out shortly. Downloads will stop with nothing else changing, which reads as a fault in the stack rather than an account that needs topping up.",
        Remedy::new("Top the account up before it runs out"),
    )
    .in_state(State::Guided)
    .with_detail(left(allowance))
}

/// A subscription ending soon. Also external: a payment is not something lemonfiber
/// can make, and offering to fix it would send the operator looking for a button that
/// cannot exist.
fn ending(days: u64) -> Problem {
    Problem::new(
        PROVIDER_ENDING,
        Severity::Warning,
        "A Usenet subscription is ending",
        "The subscription behind this account ends on the date recorded for it in the download client. When it lapses the account stops serving, with nothing in the stack having changed.",
        Remedy::new("Renew the subscription, or clear its date in the client if it renews itself"),
    )
    .in_state(State::Guided)
    .with_detail(if days == 0 {
        "the recorded date has passed".to_owned()
    } else {
        format!("{} from now", days_reading(days))
    })
}

#[cfg(test)]
mod tests {
    use crate::ports::service::Recorded;
    use crate::provider::Health;

    use super::{
        burn, finding, reading, Date, UsenetAccount, Verdict, PROVIDER_EMPTY, PROVIDER_ENDING,
        PROVIDER_LOW, WINDOW_DAYS,
    };

    const GIB: u64 = 1 << 30;

    const fn day(year: u16, month: u8, day: u8) -> Date {
        Date { year, month, day }
    }

    fn account(quota: Option<Recorded>, downloaded: u64, daily: Vec<(Date, u64)>) -> UsenetAccount {
        UsenetAccount {
            name: "Block 500".to_owned(),
            enabled: true,
            quota,
            downloaded,
            daily,
            expires_on: None,
        }
    }

    /// The whole point of the recorded baseline: a block bought partway through an
    /// account's life is spent from the point it was recorded, not from the beginning.
    #[test]
    fn what_has_gone_is_counted_from_where_the_block_was_recorded() {
        let bought_later = account(
            Some(Recorded {
                cap: 500 * GIB,
                from: 200 * GIB,
            }),
            260 * GIB,
            Vec::new(),
        );
        let allowance = reading(&bought_later, day(2026, 8, 16)).allowance;
        assert_eq!(allowance.map(|allowance| allowance.used), Some(60 * GIB));
        assert_eq!(
            allowance.and_then(|allowance| allowance.remaining()),
            Some(440 * GIB)
        );
    }

    #[test]
    fn an_account_with_no_block_recorded_still_reports_what_it_has_pulled() {
        let unlimited = account(None, 900 * GIB, Vec::new());
        let reading = reading(&unlimited, day(2026, 8, 16));
        assert_eq!(
            reading
                .allowance
                .map(|allowance| (allowance.used, allowance.cap)),
            Some((900 * GIB, None))
        );
        assert_eq!(
            reading.health(),
            Health::Unknown,
            "usage without a block to judge it against concludes nothing"
        );
    }

    /// Today is a part-day and tomorrow has not happened; averaging either in reports
    /// a rate the account is not going at.
    #[test]
    fn the_rate_covers_the_whole_days_before_today_and_no_others() {
        let today = day(2026, 8, 16);
        let daily = vec![
            (day(2026, 8, 16), 500 * GIB), // today, still running
            (day(2026, 8, 17), 900 * GIB), // a clock skewed ahead
            (day(2026, 8, 15), 3 * GIB),
            (day(2026, 8, 14), GIB),
            (day(2026, 8, 8), 700 * GIB), // eight days back, outside the window
        ];
        assert_eq!(
            burn(&daily, today).and_then(|rate| rate.days_for(8 * GIB)),
            Some(4),
            "four gibibytes over the two days recorded is two a day"
        );
    }

    #[test]
    fn a_client_with_nothing_recorded_in_the_window_has_no_rate_to_project_from() {
        let today = day(2026, 8, 16);
        assert_eq!(burn(&[], today), None);
        assert_eq!(
            burn(&[(day(2026, 1, 1), 400 * GIB)], today),
            None,
            "a record older than the window describes how the account was used then"
        );
        assert_eq!(
            burn(&[(day(2026, 8, 15), 0)], today),
            None,
            "a day nothing moved is not a rate"
        );
    }

    #[test]
    fn the_window_reaches_exactly_as_far_back_as_it_says() {
        let today = day(2026, 8, 16);
        let oldest = day(2026, 8, 9);
        assert_eq!(
            u64::try_from(oldest.days_until(today)).ok(),
            Some(WINDOW_DAYS)
        );
        assert_eq!(
            burn(&[(oldest, 7 * GIB)], today).and_then(|rate| rate.days_for(10 * GIB)),
            Some(10),
            "the far edge of the window is inside it"
        );
    }

    #[test]
    fn a_subscription_that_has_already_lapsed_reports_as_ending_rather_than_as_nothing() {
        let mut lapsed = account(None, 0, Vec::new());
        lapsed.expires_on = Some(day(2026, 8, 1));
        let reading = reading(&lapsed, day(2026, 8, 16));
        assert_eq!(reading.expires_in, Some(0));
        assert_eq!(reading.health(), Health::Expiring);
    }

    #[test]
    fn a_subscription_still_to_come_counts_the_days_to_it() {
        let mut ending = account(None, 0, Vec::new());
        ending.expires_on = Some(day(2026, 9, 1));
        let reading = reading(&ending, day(2026, 8, 16));
        assert_eq!(reading.expires_in, Some(16));
    }

    /// The figures travel with every verdict, so an operator can judge a threshold
    /// somebody else chose against how they actually use the account.
    #[test]
    fn a_healthy_account_still_says_what_it_has_left() {
        let plenty = account(
            Some(Recorded {
                cap: 500 * GIB,
                from: 0,
            }),
            160 * GIB,
            vec![(day(2026, 8, 15), 2 * GIB)],
        );
        let found = finding(&plenty, day(2026, 8, 16));
        assert_eq!(found.check, "providers.usenet.Block 500");
        assert_eq!(found.title, "Block 500");
        assert!(matches!(&found.verdict, Verdict::Pass { note }
            if note.as_deref().is_some_and(|note| note.contains("340.0 GiB left of 500.0 GiB")
                && note.contains("170 days"))));
    }

    #[test]
    fn an_account_with_nothing_left_fails_rather_than_warns() {
        let empty = account(
            Some(Recorded {
                cap: 100 * GIB,
                from: 0,
            }),
            100 * GIB,
            Vec::new(),
        );
        let found = finding(&empty, day(2026, 8, 16));
        assert!(matches!(found.verdict, Verdict::Fail(problem) if problem.code == PROVIDER_EMPTY));
    }

    #[test]
    fn an_account_running_out_warns_while_there_is_still_time_to_act() {
        let going = account(
            Some(Recorded {
                cap: 100 * GIB,
                from: 0,
            }),
            97 * GIB,
            vec![(day(2026, 8, 15), GIB)],
        );
        let found = finding(&going, day(2026, 8, 16));
        assert!(matches!(found.verdict, Verdict::Warn(problem) if problem.code == PROVIDER_LOW));
    }

    #[test]
    fn a_subscription_ending_warns_and_says_how_long_is_left() {
        let mut ending = account(None, 0, Vec::new());
        ending.expires_on = Some(day(2026, 8, 17));
        let found = finding(&ending, day(2026, 8, 16));
        assert!(matches!(
            found.verdict,
            Verdict::Warn(problem)
                if problem.code == PROVIDER_ENDING
                    && problem.detail.as_deref() == Some("1 day from now")
        ));

        let mut lapsed = account(None, 0, Vec::new());
        lapsed.expires_on = Some(day(2026, 8, 1));
        let found = finding(&lapsed, day(2026, 8, 16));
        assert!(
            matches!(found.verdict, Verdict::Warn(problem) if problem.detail.as_deref() == Some("the recorded date has passed"))
        );
    }

    /// An account with no allowance recorded says what it has pulled and nothing more:
    /// there is no figure to judge it against, and inventing one is the whole failure
    /// this feature exists to avoid.
    #[test]
    fn an_account_with_no_allowance_says_only_what_it_has_pulled() {
        let found = finding(&account(None, 42 * GIB, Vec::new()), day(2026, 8, 16));
        assert!(matches!(&found.verdict, Verdict::Pass { note }
            if note.as_deref().is_some_and(|note| note.contains("42.0 GiB pulled")
                && note.contains("no allowance is recorded"))));
    }
}

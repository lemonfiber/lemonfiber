//! What a Usenet account has left, and whether it is still serving.
//!
//! The client is the only place these figures exist: a provider publishes no quota,
//! so the block that was bought is recorded in the client and every byte pulled is
//! measured there. A record of what has gone says nothing about this morning, though —
//! a hundred gigabytes pulled last week is not evidence the account answers now — so the
//! two halves of the client's answer are read as the different things they are.
//!
//! The other half is what happened when the client last spoke to the provider: the
//! connections it is holding, and the words the provider gave it when it refused. That is
//! the only statement a Usenet provider ever makes about an account, and it is what tells
//! a rejected password from an account with nothing left from one that is simply idle —
//! three states that look identical from a queue that is not moving.

use lemonfiber_manifest::Date;

use crate::bytes::humanize;
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::plural;
use crate::ports::service::{Standing, UsenetAccount};
use crate::provider::trouble::Trouble;
use crate::provider::{Allowance, Answer, Burn, Health, Reading, Renewal};

use super::{
    PROVIDER_CROWDED, PROVIDER_EMPTY, PROVIDER_ENDING, PROVIDER_LOW, PROVIDER_REFUSED,
    PROVIDER_SILENT,
};

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
        answer: account.standing.as_ref().map_or(Answer::Unasked, answer),
        renewal: Renewal::Bought,
        allowance: Some(allowance_of(account, today)),
        expires_in: expires_in(account.expires_on, today),
    }
}

/// What the client's last exchange with the account establishes.
///
/// A connection the client is holding open outranks anything recorded earlier. Clients
/// keep the last trouble they hit until something replaces it, so an account fixed an
/// hour ago still carries the message that sent the operator to fix it — and a ready
/// connection is proof the provider is taking the credential at this moment.
///
/// Holding none proves nothing by itself: a client with an empty queue holds no
/// connections to a perfectly good account, and reading idleness as a fault would raise
/// one against every account on a quiet afternoon. So where the provider's words place
/// nothing, the account is only called silent once the client has also dropped it, which
/// is the client's own verdict on it now rather than a message it never cleared.
fn answer(standing: &Standing) -> Answer {
    if standing.ready > 0 {
        return Answer::Answered;
    }
    match standing.trouble.as_deref().map(Trouble::of) {
        Some(Trouble::Crowded) => Answer::Limited,
        Some(Trouble::Refused) => Answer::Refused,
        Some(Trouble::Spent) => Answer::Depleted,
        Some(Trouble::Unplaced) | None if !standing.serving => Answer::Silent,
        Some(Trouble::Unplaced) | None => Answer::Unasked,
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

/// What one account amounts to: its own state, and — where the provider has said the
/// client is asking more of it than the plan allows — the configuration mismatch that is
/// a separate problem with a separate remedy, and would be buried inside the other.
pub(super) fn findings(account: &UsenetAccount, today: Date) -> Vec<Finding> {
    let mut findings = vec![finding(account, today)];
    findings.extend(crowded(account));
    findings
}

/// One account's finding: what it has left, whether it is serving, and what to do about
/// it.
///
/// The figures travel with every verdict, passing ones included. "340 GiB left, about
/// three weeks at last week's rate" is what lets an operator judge for themselves
/// whether a warning threshold set by somebody else suits how they use the account.
fn finding(account: &UsenetAccount, today: Date) -> Finding {
    let reading = reading(account, today);
    let allowance = allowance_of(account, today);
    let said = recorded(account);
    let verdict = match reading.health() {
        Health::Exhausted => Verdict::Fail(empty(allowance, said)),
        Health::Invalid => Verdict::Fail(refused(said)),
        Health::Unreachable => Verdict::Warn(silent(said)),
        Health::Depleting => Verdict::Warn(low(allowance)),
        Health::Expiring => Verdict::Warn(ending(reading.expires_in.unwrap_or(0))),
        // Capacity that is fine and capacity that says nothing both read as nothing to
        // do — the second is the ordinary case for an account with no block recorded.
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

/// The words the client last recorded against the account, where it recorded any.
fn recorded(account: &UsenetAccount) -> Option<&str> {
    account.standing.as_ref()?.trouble.as_deref()
}

/// The client set to open more connections than the account allows.
///
/// Raised from the provider's own refusal rather than from any published limit: no
/// Usenet provider states its connection count anywhere that can be read, so a provider
/// saying it has been asked for too many is the only evidence of a mismatch there is.
///
/// Not raised while the client is holding every connection it is set to open, because an
/// account allowing all of them is not refusing any — the message is then one the client
/// has simply not had reason to overwrite.
fn crowded(account: &UsenetAccount) -> Option<Finding> {
    let standing = account.standing.as_ref()?;
    let words = standing.trouble.as_deref()?;
    if Trouble::of(words) != Trouble::Crowded || standing.ready >= standing.configured {
        return None;
    }
    Some(Finding::in_category(
        Category::Providers,
        &format!("providers.usenet.{}.connections", account.name),
        &format!("{} connections", account.name),
        Verdict::Warn(too_many(standing.configured, words)),
    ))
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
fn empty(allowance: Allowance, said: Option<&str>) -> Problem {
    Problem::new(
        PROVIDER_EMPTY,
        Severity::Error,
        "A Usenet account has nothing left",
        "The account authenticates perfectly and can download nothing, which looks exactly like a broken stack from the outside. A block account does not refill on its own.",
        Remedy::new("Top the account up, or point the client at one that has data left"),
    )
    .in_state(State::Guided)
    .with_detail(beside(left(allowance), said))
}

/// An account whose credential the provider refused.
///
/// An error, and one no capacity figure softens: the account downloads nothing however
/// much data it has left, and the stack shows no fault of its own because it has none.
/// Told apart from an account that never answered because the remedies share nothing —
/// this one is fixed here, in a minute, by the operator.
fn refused(said: Option<&str>) -> Problem {
    Problem::new(
        PROVIDER_REFUSED,
        Severity::Error,
        "A Usenet account is refusing the login",
        "The provider answered the download client and rejected the credentials it offered. Nothing downloads through this account until they are right, and every service in the stack stays green while that is true.",
        Remedy::new(
            "Check the account's username and password in the download client, and that the subscription behind it is still active",
        ),
    )
    .in_state(State::Guided)
    .with_detail(beside("the login was rejected".to_owned(), said))
}

/// An account that has stopped answering at all.
///
/// A warning rather than an error, and deliberately not a rejected credential: the
/// remedy is outside the stack and often outside the household, and a provider that is
/// down this minute is usually back within the hour. Sending an operator to re-enter a
/// password that was always correct is the mistake this distinction exists to prevent.
fn silent(said: Option<&str>) -> Problem {
    Problem::new(
        PROVIDER_SILENT,
        Severity::Warning,
        "A Usenet account is not answering",
        "The download client has stopped using this account because it could not reach it. That is the provider being down or the connection to it failing, rather than anything about the account itself — which is why it is worth telling apart from a rejected login before anything is changed.",
        Remedy::new(
            "Check the provider's status page and this machine's connection; the client picks the account up again on its own once it answers",
        ),
    )
    .in_state(State::Guided)
    .with_detail(beside(
        "the download client has taken it out of rotation".to_owned(),
        said,
    ))
}

/// A client set to open more connections than the account allows.
///
/// A warning: downloads still run on the connections the provider does allow, slower,
/// while the client's log fills with refusals that read as a failing provider rather
/// than as a number set too high in one field.
fn too_many(configured: u64, words: &str) -> Problem {
    Problem::new(
        PROVIDER_CROWDED,
        Severity::Warning,
        "A Usenet account is set to more connections than it allows",
        "The provider is refusing the connections beyond what the plan includes. Downloads still run on the ones it allows, and the refusals it sends back look like an unreliable provider rather than a setting that is one too high.",
        Remedy::new(
            "Lower the connection count for this account in the download client to what the plan includes",
        ),
    )
    .in_state(State::Guided)
    .with_detail(beside(
        format!(
            "the client is set to {configured} connection{}",
            plural::s(usize::try_from(configured).unwrap_or(2))
        ),
        Some(words),
    ))
}

/// A detail with the client's own words beside it, where it recorded any.
///
/// Quoted rather than summarised: the sentence is the provider's, it is what the
/// operator will find in their client's log, and a report that paraphrases it leaves
/// them matching two different accounts of the same failure.
///
/// Every detail is scanned for credentials on its way to a person. That scan reads what
/// stands before a separator as a name only where it is one — a single word, no spaces —
/// so a clause introducing a quotation is left alone whatever words it is built from, and
/// the provider's own sentence reaches the operator rather than a redaction of it.
fn beside(detail: String, said: Option<&str>) -> String {
    match said {
        None => detail,
        Some(words) => format!("{detail} — the download client recorded: {words}"),
    }
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
mod tests;

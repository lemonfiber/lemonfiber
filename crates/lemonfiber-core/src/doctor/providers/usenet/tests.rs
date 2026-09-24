use crate::ports::service::Recorded;
use crate::provider::Health;

use super::{
    burn, finding, findings, reading, Date, Standing, UsenetAccount, Verdict, PROVIDER_CROWDED,
    PROVIDER_EMPTY, PROVIDER_ENDING, PROVIDER_LOW, PROVIDER_REFUSED, PROVIDER_SILENT, WINDOW_DAYS,
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
        standing: None,
    }
}

/// An account as the client is finding it: connections held of connections set, and
/// whatever the provider last said.
fn served(ready: u64, serving: bool, trouble: Option<&str>) -> Standing {
    Standing {
        ready,
        configured: 8,
        serving,
        trouble: trouble.map(str::to_owned),
    }
}

/// The account with a block half spent, so nothing about its capacity decides
/// anything and what the provider said is the whole of the verdict. An account the
/// client reports no standing for takes `None`.
fn holding(standing: impl Into<Option<Standing>>) -> UsenetAccount {
    let mut account = account(
        Some(Recorded {
            cap: 500 * GIB,
            from: 0,
        }),
        100 * GIB,
        Vec::new(),
    );
    account.standing = standing.into();
    account
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

/// The distinction the whole live view exists for: a refused login is fixed here in
/// a minute, and a provider that is not answering cannot be fixed here at all.
#[test]
fn a_refused_login_and_a_provider_that_says_nothing_are_different_findings() {
    let refused = holding(served(
        0,
        true,
        Some("Failed login for server news.example.com [481 Authentication failed]"),
    ));
    // The detail keeps the provider's own sentence: a report that quoted a redaction
    // back at the operator would be worse than one that quoted nothing.
    let found = finding(&refused, day(2026, 8, 16));
    assert!(matches!(&found.verdict, Verdict::Fail(problem)
        if problem.code == PROVIDER_REFUSED
            && problem.detail.as_deref().is_some_and(|detail| detail.contains("481"))));

    let dropped = holding(served(
        0,
        false,
        Some("Cannot connect to server news.example.com [timed out]"),
    ));
    let found = finding(&dropped, day(2026, 8, 16));
    assert!(
        matches!(&found.verdict, Verdict::Warn(problem) if problem.code == PROVIDER_SILENT),
        "a client that could not reach it must never report a rejected credential"
    );

    // A client does not always have words to record — some failures it counts
    // rather than describes — and having dropped the account is the statement.
    let wordless = holding(served(0, false, None));
    let found = finding(&wordless, day(2026, 8, 16));
    assert!(matches!(&found.verdict, Verdict::Warn(problem)
        if problem.code == PROVIDER_SILENT
            && problem.detail.as_deref() == Some("the download client has taken it out of rotation")));
}

/// A client with an empty queue holds no connections to a perfectly good account,
/// and it keeps the last message it saw until something replaces it. Reading either
/// as a fault raises one against every account on a quiet afternoon.
#[test]
fn an_idle_account_is_not_a_failing_one() {
    let idle = holding(served(0, true, None));
    assert!(matches!(
        finding(&idle, day(2026, 8, 16)).verdict,
        Verdict::Pass { .. }
    ));

    // Words nothing places, recorded by a client still using the account, conclude
    // nothing at all — least of all that the account is the problem.
    let stale = holding(served(
        0,
        true,
        Some("Cannot connect to server news.example.com [timed out]"),
    ));
    assert!(matches!(
        finding(&stale, day(2026, 8, 16)).verdict,
        Verdict::Pass { .. }
    ));
}

/// A held connection is the one proof there is that the credential works now, and it
/// outranks a message the client recorded before the operator fixed it.
#[test]
fn a_connection_being_held_outranks_what_was_recorded_earlier() {
    let working = holding(served(
        4,
        true,
        Some("Failed login for server news.example.com [481 Authentication failed]"),
    ));
    assert!(matches!(
        finding(&working, day(2026, 8, 16)).verdict,
        Verdict::Pass { .. }
    ));
}

/// The provider's own word about being empty is believed over the figures, which
/// only ever say what one client happens to have recorded.
#[test]
fn an_account_the_provider_says_is_empty_fails_without_a_block_recorded() {
    let mut spent = account(None, 40 * GIB, Vec::new());
    spent.standing = Some(served(0, true, Some("502 No credits left on this account")));
    let found = finding(&spent, day(2026, 8, 16));
    assert!(matches!(&found.verdict, Verdict::Fail(problem)
        if problem.code == PROVIDER_EMPTY
            && problem.detail.as_deref().is_some_and(|detail| detail.contains("No credits"))));
}

/// A number set one too high is not a failing account, and burying it inside the
/// account's own verdict is how it stays unfixed for a year.
#[test]
fn asking_for_more_connections_than_the_plan_allows_is_its_own_finding() {
    let crowded = holding(served(
        2,
        true,
        Some("Too many connections to server news.example.com [502 Too many connections]"),
    ));
    let found = findings(&crowded, day(2026, 8, 16));
    assert_eq!(found.len(), 2);
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Pass { .. })
    ));
    assert_eq!(
        found.get(1).map(|finding| finding.check.as_str()),
        Some("providers.usenet.Block 500.connections")
    );
    assert!(
        matches!(found.get(1).map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.code == PROVIDER_CROWDED
            && problem.detail.as_deref().is_some_and(|detail| detail.contains("8 connections")))
    );
}

/// The same refusal read while the client happens to be holding nothing — an empty
/// queue at three in the morning. The number it is set to is still one too high, and
/// the account behind it is still perfectly good.
#[test]
fn a_crowded_account_reads_the_same_when_the_client_is_idle() {
    let idle = holding(served(
        0,
        true,
        Some("Too many connections to server news.example.com [502 Too many connections]"),
    ));
    let found = findings(&idle, day(2026, 8, 16));
    assert_eq!(found.len(), 2);
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Pass { .. })
    ));
    assert!(
        matches!(found.get(1).map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.code == PROVIDER_CROWDED)
    );
}

/// An account allowing every connection the client opens is refusing none of them,
/// so the message is one the client has had no reason to overwrite.
#[test]
fn a_client_holding_every_connection_it_opens_is_not_reported_as_crowded() {
    let all_up = holding(served(
        8,
        true,
        Some("Too many connections to server news.example.com [502 Too many connections]"),
    ));
    assert_eq!(findings(&all_up, day(2026, 8, 16)).len(), 1);
    assert_eq!(findings(&holding(None), day(2026, 8, 16)).len(), 1);
    assert_eq!(
        findings(&holding(served(0, true, None)), day(2026, 8, 16)).len(),
        1
    );
}

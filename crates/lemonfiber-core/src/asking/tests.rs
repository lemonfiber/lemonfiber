use super::{limit, period, Policy, Standing};
use crate::ports::service::{Asking, Headroom, Left, Quota};

/// One kind's count, held to a limit or held to nothing.
const fn counted(limit: Option<u32>, used: u32) -> Left {
    Left {
        limit,
        used,
        days: Some(7),
    }
}

/// Each pair of settings the request service can be in reads as its own policy.
#[test]
fn each_pair_of_settings_reads_as_its_own_policy() {
    let quota = Some(Quota {
        requests: 5,
        days: 7,
    });
    assert_eq!(
        Policy::of(&Asking {
            approves_own: true,
            quota: None
        }),
        Policy::Trusted
    );
    assert_eq!(
        Policy::of(&Asking {
            approves_own: true,
            quota
        }),
        Policy::WithinALimit
    );
    assert_eq!(
        Policy::of(&Asking {
            approves_own: false,
            quota
        }),
        Policy::EverythingWaits
    );
}

/// A limit set on a household that approves nothing automatically still waits.
///
/// The limit is real and the service keeps counting, but nothing arrives on it —
/// so reporting that household as living within a limit would name the half of
/// its arrangement that decides the least.
#[test]
fn a_limit_beside_no_automatic_approval_still_reads_as_waiting() {
    assert_eq!(
        Policy::of(&Asking {
            approves_own: false,
            quota: Some(Quota {
                requests: 1,
                days: 1
            })
        }),
        Policy::EverythingWaits
    );
}

/// Every policy round-trips through the word a surface names it by.
#[test]
fn every_policy_round_trips_through_its_own_word() {
    for policy in Policy::ALL {
        assert_eq!(Policy::from_label(policy.label()), Some(policy));
        assert!(!policy.means().is_empty());
        assert!(Policy::labels().contains(policy.label()));
    }
}

/// A word typed loosely still reaches the policy it names.
#[test]
fn a_word_typed_loosely_still_reaches_its_policy() {
    assert_eq!(Policy::from_label("  TRUSTED "), Some(Policy::Trusted));
    assert_eq!(Policy::from_label("generous"), None);
}

/// Only the policy that lives inside a limit asks for one, and only the two that
/// let things through say so.
#[test]
fn only_the_policy_that_lives_in_a_limit_asks_for_one() {
    assert!(Policy::WithinALimit.needs_a_limit());
    assert!(!Policy::Trusted.needs_a_limit());
    assert!(!Policy::EverythingWaits.needs_a_limit());
    assert!(Policy::Trusted.arrives_unseen());
    assert!(Policy::WithinALimit.arrives_unseen());
    assert!(!Policy::EverythingWaits.arrives_unseen());
}

/// Each amount left reads as its own standing.
#[test]
fn each_amount_left_reads_as_its_own_standing() {
    assert_eq!(Standing::of(counted(None, 90)), Standing::Unlimited);
    assert_eq!(Standing::of(counted(Some(20), 2)), Standing::WithinQuota);
    assert_eq!(Standing::of(counted(Some(20), 16)), Standing::NearQuota);
    assert_eq!(
        Standing::of(counted(Some(20), 20)),
        Standing::QuotaExhausted
    );
}

/// A limit too small for a fifth to mean anything still reads as near from its
/// last one, rather than jumping from comfortable to spent.
#[test]
fn a_small_limit_still_warns_before_it_is_spent() {
    assert_eq!(Standing::of(counted(Some(2), 1)), Standing::NearQuota);
    assert_eq!(Standing::of(counted(Some(3), 1)), Standing::WithinQuota);
}

/// A limit lowered under what is spent is spent, not something worse.
#[test]
fn a_limit_lowered_under_what_is_spent_reads_as_spent() {
    assert_eq!(Standing::of(counted(Some(1), 9)), Standing::QuotaExhausted);
}

/// Taken across both counts, the worse of the two is what stands.
///
/// A household with every film still available and no season left cannot ask for
/// the next episode, and a line saying they are within their limit would be true
/// of a request they cannot make.
#[test]
fn the_worse_of_the_two_counts_is_what_stands() {
    let mixed = Headroom {
        films: counted(Some(20), 0),
        television: counted(Some(4), 4),
    };

    assert_eq!(Standing::across(mixed), Standing::QuotaExhausted);
    assert_eq!(
        Standing::across(Headroom {
            films: mixed.television,
            television: mixed.films,
        }),
        Standing::QuotaExhausted
    );
}

/// Nothing counted against either half is no limit at all.
#[test]
fn nothing_counted_against_either_half_is_no_limit() {
    assert_eq!(Standing::across(Headroom::default()), Standing::Unlimited);
}

/// Every standing reads as a plain phrase, and the two worth acting on say so.
#[test]
fn every_standing_reads_as_a_plain_phrase() {
    for standing in [
        Standing::Unlimited,
        Standing::WithinQuota,
        Standing::NearQuota,
        Standing::QuotaExhausted,
    ] {
        let phrase = standing.phrase();
        assert!(!phrase.is_empty(), "{standing:?} says nothing");
        assert!(phrase.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
    }
    assert!(Standing::NearQuota.worth_saying());
    assert!(Standing::QuotaExhausted.worth_saying());
    assert!(!Standing::WithinQuota.worth_saying());
    assert!(!Standing::Unlimited.worth_saying());
}

/// A standing serialises under its own name, which is what a browser reads.
#[test]
fn a_standing_serialises_under_its_own_name() {
    assert_eq!(
        serde_json::to_string(&Standing::NearQuota).unwrap_or_default(),
        r#""near-quota""#
    );
    assert_eq!(
        serde_json::to_string(&Policy::WithinALimit).unwrap_or_default(),
        r#""within-a-limit""#
    );
}

/// The two periods a household means by name are said by name, and the rest as
/// the days they are.
#[test]
fn the_periods_a_household_names_are_said_by_name() {
    assert_eq!(period(1), "a day");
    assert_eq!(period(7), "a week");
    assert_eq!(period(30), "a month");
    assert_eq!(period(14), "14 days");
}

/// A limit reads as a sentence rather than as two numbers.
#[test]
fn a_limit_reads_as_a_sentence() {
    assert_eq!(
        limit(Quota {
            requests: 5,
            days: 7
        }),
        "5 requests a week"
    );
    assert_eq!(
        limit(Quota {
            requests: 1,
            days: 14
        }),
        "1 request every 14 days"
    );
    assert_eq!(
        limit(Quota {
            requests: 3,
            days: 30
        }),
        "3 requests a month"
    );
}

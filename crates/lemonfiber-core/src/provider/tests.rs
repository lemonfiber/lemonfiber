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

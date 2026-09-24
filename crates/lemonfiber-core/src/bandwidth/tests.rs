use super::{
    cap::{Cap, Metered, Reached, WhenExceeded, CRAWL},
    capacity::{Capacity, Source},
    holding::{Answer, Held, Holding},
    limit::Limit,
    respite::Respite,
    rhythm::{Period, Rhythm},
    weigh, Declared, Measured, Reading, Resolved, Restraint, UNTOUCHED,
};

/// A moment every case here reads against.
const NOW: u64 = 1_790_812_800;

/// Ten megabytes down, one up, measured today.
fn a_line() -> Capacity {
    Capacity {
        down: 10 * 1024 * 1024,
        up: 1024 * 1024,
        source: Source::Observed,
        taken: NOW,
        through_tunnel: false,
    }
}

/// A stack holding the stack to half the line, with the house awake.
fn a_household() -> Measured {
    Measured {
        declared: Declared {
            down: Some(Limit::Share(50)),
            up: Some(Limit::Share(25)),
            rhythm: Rhythm::read("07:00-23:00"),
            cap: None,
            capacity: Some(a_line()),
            respite: None,
            stopped: false,
        },
        now: NOW,
        zone: Some("Europe/Amsterdam".to_owned()),
        clients: vec![client(Some(Period::Active))],
        metered: None,
        applied: false,
    }
}

/// A client that would not answer at all.
fn silent() -> Holding {
    Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Silent {
            said: "connection refused".to_owned(),
        },
        pulling: None,
    }
}

/// A client answering that it is holding, on the side of the day given.
fn client(period: Option<Period>) -> Holding {
    Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Held {
            down: Held::of(Some(1_000), Some(1_000), Some(500), true),
            up: Held::of(Some(100), Some(100), Some(50), true),
            period,
        },
        pulling: None,
    }
}

#[test]
fn a_share_is_weighed_against_the_line_that_direction_was_measured_at() {
    // Down against the downlink and up against the uplink. One figure for
    // both would make every upload share several times what was asked for,
    // because a home connection is asymmetric.
    let shared = weigh(&a_household());
    assert_eq!(shared.down.resolved, Resolved::At(5 * 1024 * 1024));
    assert_eq!(shared.up.resolved, Resolved::At(256 * 1024));
    assert!(
        shared.down.says.contains("10.0 MiB/s"),
        "{}",
        shared.down.says
    );
    assert!(shared.up.says.contains("1.0 MiB/s"), "{}", shared.up.says);
}

#[test]
fn an_upload_limit_is_never_reported_without_what_it_costs_the_ratio() {
    let shared = weigh(&a_household());
    let ratio = shared.ratio.unwrap_or_default();
    assert!(ratio.contains("ratio"), "{ratio}");
    assert!(ratio.contains("standing"), "{ratio}");
    assert!(
        ratio.contains("stopped one does not"),
        "throttling is offered rather than stopping, and says why: {ratio}"
    );
}

#[test]
fn no_upload_limit_has_no_consequence_to_state() {
    let mut measured = a_household();
    measured.declared.up = None;
    assert_eq!(weigh(&measured).ratio, None);
}

#[test]
fn what_is_outside_every_limit_here_is_always_said() {
    // Both of them, on every report. An operator's two fears about a
    // bandwidth feature are that it throttles the household's own viewing
    // and that it meddles with the machine; leaving either to be inferred is
    // how a report gets read as doing them.
    let shared = weigh(&Measured::default());
    assert_eq!(shared.untouched.len(), UNTOUCHED.len());
    assert!(!shared.untouched.is_empty());
    let said = shared.untouched.join(" ");
    assert!(said.contains("watching from your own library"), "{said}");
    assert!(said.contains("never goes out over the line"), "{said}");
    assert!(
        said.contains("does not shape the machine's traffic"),
        "{said}"
    );
}

#[test]
fn which_side_of_the_day_it_is_is_read_from_the_clients_rather_than_a_clock() {
    // Nothing in this product knows the household's local time of day. The
    // client's own scheduler does, so the answer is a measurement.
    assert_eq!(weigh(&a_household()).restraint, Restraint::ScheduledActive);

    let mut asleep = a_household();
    asleep.clients = vec![client(Some(Period::Quiet))];
    assert_eq!(weigh(&asleep).restraint, Restraint::ScheduledQuiet);

    let mut unscheduled = a_household();
    unscheduled.clients = vec![client(None)];
    assert_eq!(weigh(&unscheduled).restraint, Restraint::Limited);
}

#[test]
fn where_two_clients_disagree_the_constrained_answer_wins() {
    // A report saying the house was asleep while one client was still
    // throttled would be describing neither client.
    let mut mixed = a_household();
    mixed.clients = vec![client(Some(Period::Quiet)), client(Some(Period::Active))];
    assert_eq!(weigh(&mixed).restraint, Restraint::ScheduledActive);
}

#[test]
fn a_client_that_would_not_answer_has_no_opinion_about_the_hour() {
    let mut unreachable = a_household();
    unreachable.clients = vec![silent()];
    assert_eq!(weigh(&unreachable).restraint, Restraint::Limited);
}

#[test]
fn a_stack_with_nothing_declared_is_unlimited_and_says_so() {
    let shared = weigh(&Measured::default());
    assert_eq!(shared.restraint, Restraint::Unlimited);
    assert!(shared.means.contains("whatever the line has"));
    assert_eq!(shared.down.limit, Limit::Unlimited);
    assert!(shared.capacity.is_none());
    assert!(shared.cautions.is_empty());
    assert!(shared.rhythm.is_none());
    assert!(shared.reached.is_none());
    assert!(shared.respite_says.is_none());
}

#[test]
fn a_cap_that_is_spent_outranks_everything_else_true_of_the_line() {
    // It is the one with a bill behind it.
    let mut metered = a_household();
    metered.declared.cap = Some(Cap {
        monthly: 100,
        exceeded: WhenExceeded::Pause,
    });
    metered.metered = Some(Metered::of("2026-09", 100, 0, Vec::new()));
    let shared = weigh(&metered);
    assert_eq!(shared.restraint, Restraint::CapExceeded);
    assert_eq!(shared.reached, Some(Reached::Exceeded));
    assert!(shared.restraint.worth_saying());

    metered.metered = Some(Metered::of("2026-09", 95, 0, Vec::new()));
    assert_eq!(weigh(&metered).restraint, Restraint::CapWarning);

    metered.metered = Some(Metered::of("2026-09", 1, 0, Vec::new()));
    assert_eq!(weigh(&metered).restraint, Restraint::ScheduledActive);
}

/// The same household, with a cap of a hundred bytes already spent.
fn a_spent_month(exceeded: WhenExceeded) -> Measured {
    let mut spent = a_household();
    spent.declared.cap = Some(Cap {
        monthly: 100,
        exceeded,
    });
    spent.metered = Some(Metered::of("2026-09", 100, 0, Vec::new()));
    spent
}

#[test]
fn a_spent_cap_that_chose_a_crawl_is_what_the_report_shows_in_force() {
    // The figures shown are the ones the clients were handed, off the same
    // function, so the report cannot describe a limit nothing is keeping.
    let shared = weigh(&a_spent_month(WhenExceeded::Throttle));
    assert_eq!(shared.down.resolved, Resolved::At(CRAWL));
    assert_eq!(shared.up.resolved, Resolved::At(CRAWL));
    assert!(
        shared.acting.is_some_and(
            |said| said.contains("crawl") && said.contains("rather than the ones you declared")
        ),
        "and it says the figures are not the declared ones"
    );
}

#[test]
fn a_crawl_never_speeds_up_a_limit_that_was_already_slower() {
    // The one direction a spent cap may never move anything.
    let mut slow = a_spent_month(WhenExceeded::Throttle);
    slow.declared.down = Some(Limit::Absolute(1_024));
    assert_eq!(weigh(&slow).down.resolved, Resolved::At(1_024));
}

#[test]
fn a_share_of_a_line_nothing_measured_still_becomes_a_crawl_at_a_spent_cap() {
    // Tightening, which is the only direction this may err in: a share that
    // resolved to nothing would otherwise leave a spent month unlimited.
    let mut unmeasured = a_spent_month(WhenExceeded::Throttle);
    unmeasured.declared.capacity = None;
    assert_eq!(weigh(&unmeasured).down.resolved, Resolved::At(CRAWL));
}

#[test]
fn the_other_two_answers_to_a_spent_cap_leave_every_figure_where_it_was() {
    for choice in [WhenExceeded::Pause, WhenExceeded::Continue] {
        let shared = weigh(&a_spent_month(choice));
        assert_eq!(
            shared.down.resolved,
            Resolved::At(5 * 1024 * 1024),
            "{choice:?}"
        );
        assert!(shared.acting.is_some(), "{choice:?}");
    }
    assert!(
        weigh(&a_household()).acting.is_none(),
        "and a month nothing is over says nothing about a cap"
    );
}

#[test]
fn a_cap_with_nothing_counting_against_it_is_not_a_verdict() {
    // Declaring a cap on a stack whose clients cannot be read is not the same
    // as being inside it, and must not report as being inside it.
    let mut declared = a_household();
    declared.declared.cap = Some(Cap {
        monthly: 100,
        exceeded: WhenExceeded::Continue,
    });
    assert_eq!(weigh(&declared).reached, None);
}

#[test]
fn an_override_outranks_the_schedule_because_it_is_what_is_happening_now() {
    let mut lifted = a_household();
    lifted.declared.respite = Some(Respite {
        until: NOW + 30 * 60,
    });
    let shared = weigh(&lifted);
    assert_eq!(shared.restraint, Restraint::Overridden);
    assert!(shared
        .respite_says
        .is_some_and(|said| said.contains("come back on their own")));
}

#[test]
fn an_override_that_ran_out_stops_outranking_anything_and_still_reports() {
    let mut expired = a_household();
    expired.declared.respite = Some(Respite {
        until: NOW - 60 * 60,
    });
    let shared = weigh(&expired);
    assert_eq!(shared.restraint, Restraint::ScheduledActive);
    assert!(shared
        .respite_says
        .is_some_and(|said| said.contains("came back")));
}

#[test]
fn what_is_worth_knowing_about_a_stale_reading_travels_with_it() {
    let mut old = a_household();
    old.declared.capacity = Some(Capacity {
        taken: NOW - super::capacity::GOES_STALE_AFTER - 1,
        through_tunnel: true,
        ..a_line()
    });
    let shared = weigh(&old);
    assert_eq!(shared.cautions.len(), 2, "{:?}", shared.cautions);
}

#[test]
fn only_a_client_nothing_was_holding_back_has_measured_the_line() {
    let free = Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Held {
            down: Held::of(None, None, Some(20 * 1024 * 1024), true),
            up: Held::of(None, None, Some(2 * 1024 * 1024), true),
            period: None,
        },
        pulling: None,
    };
    assert!(super::observed(&[free], NOW, false)
        .is_some_and(|seen| seen.down == 20 * 1024 * 1024 && seen.up == 2 * 1024 * 1024));

    // A rate measured under a limit is a measurement of the limit. Recording
    // it would talk a throttled stack down to a tenth of its own connection,
    // and then to a tenth of that.
    assert_eq!(super::observed(&[client(None)], NOW, false), None);
    assert_eq!(super::observed(&[], NOW, false), None);
    assert_eq!(super::observed(&[silent()], NOW, false), None);
}

#[test]
fn a_direction_with_no_figure_to_give_a_client_has_none() {
    assert_eq!(Reading::of(Limit::Unlimited, Some(100)).bytes(), None);
    assert_eq!(Reading::of(Limit::Share(50), None).bytes(), None);
    assert_eq!(Reading::of(Limit::Share(50), Some(100)).bytes(), Some(50));
}

#[test]
fn only_a_line_that_is_going_wrong_interrupts_somebody_asking_about_something_else() {
    assert!(!Restraint::Unlimited.worth_saying());
    assert!(!Restraint::Limited.worth_saying());
    assert!(!Restraint::ScheduledActive.worth_saying());
    assert!(!Restraint::ScheduledQuiet.worth_saying());
    assert!(Restraint::Overridden.worth_saying());
    assert!(Restraint::CapWarning.worth_saying());
    assert!(Restraint::CapExceeded.worth_saying());
}

#[test]
fn every_state_this_module_can_reach_says_what_it_means() {
    for state in [
        Restraint::Unlimited,
        Restraint::Limited,
        Restraint::ScheduledActive,
        Restraint::ScheduledQuiet,
        Restraint::Overridden,
        Restraint::CapWarning,
        Restraint::CapExceeded,
    ] {
        let means = state.means();
        assert!(!means.is_empty(), "{state:?}");
    }
}

#[test]
fn a_declared_limit_of_none_at_all_is_not_a_limited_line() {
    let declared = Declared {
        down: Some(Limit::Unlimited),
        up: Some(Limit::Unlimited),
        ..Declared::default()
    };
    assert!(!declared.limited());
    assert!(Declared {
        down: Some(Limit::Share(50)),
        ..Declared::default()
    }
    .limited());
}

#[test]
fn every_code_this_module_raises_belongs_to_it() {
    for code in [
        super::NOTHING_MEASURED,
        super::NO_ZONE,
        super::UNREADABLE,
        super::NOTHING_TO_LIMIT,
    ] {
        // Bound rather than called inside the message: an argument to a
        // passing assertion is never evaluated, and a line nothing evaluates
        // is one the coverage gate counts against a file that looks tested.
        let named = code.as_str();
        assert!(named.starts_with("RATE-"), "{named} is this feature's own");
    }
}

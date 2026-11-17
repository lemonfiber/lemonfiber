use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use lemonfiber_fixtures::ports::Chance;

use super::{Asker, Limits, NotKept, SignInCodes};

/// The moment every test here runs at.
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// The limits a stack holds where nothing is recorded.
const LIMITS: Limits = Limits {
    waiting: 3,
    per_period: 10,
    period: Duration::from_secs(15 * 60),
    by_address: false,
};

/// How long a code lapses after it is asked for.
const LAPSES: Duration = Duration::from_secs(600);

fn address(said: &str) -> Option<IpAddr> {
    said.parse().ok()
}

/// The handle a code asked for by `asker` is kept under, the id `id` behind it.
fn issued(codes: &SignInCodes, asker: &Asker, id: &str, random: &Chance) -> String {
    codes
        .kept(
            random,
            (id.to_owned(), asker.clone(), now() + LAPSES),
            &LIMITS,
            now(),
        )
        .unwrap_or_default()
}

/// The id behind the code `handle` names and when it lapses, where `asker` may claim it
/// under `limits` at `at`, let go again at once.
fn waiting(
    codes: &SignInCodes,
    handle: &str,
    asker: &Asker,
    limits: &Limits,
    at: SystemTime,
) -> Option<(String, SystemTime)> {
    codes
        .claimed(handle, asker, limits, at)
        .map(|claimed| (claimed.id.clone(), claimed.lapses))
}

#[test]
fn a_code_is_finished_by_its_handle_for_the_session_that_asked_and_by_no_other() {
    let codes = SignInCodes::default();
    let first = Asker::new(Some("first-session"), address("192.0.2.7"));
    let handle = issued(&codes, &first, "adapter-id", &Chance::turning(1));

    assert_eq!(handle.len(), 32);
    assert_eq!(
        waiting(&codes, &handle, &first, &LIMITS, now()),
        Some(("adapter-id".to_owned(), now() + LAPSES))
    );
    for other in [
        Asker::new(Some("second-session"), address("192.0.2.7")),
        Asker::new(None, address("192.0.2.7")),
    ] {
        assert_eq!(waiting(&codes, &handle, &other, &LIMITS, now()), None);
    }
    assert_eq!(
        waiting(&codes, "not-a-handle", &first, &LIMITS, now()),
        None
    );
}

#[test]
fn a_code_asked_for_without_a_session_is_finished_by_its_handle_from_anywhere_unless_bound_to_the_address(
) {
    let codes = SignInCodes::default();
    let phone = Asker::new(None, address("192.0.2.7"));
    let handle = issued(&codes, &phone, "adapter-id", &Chance::turning(1));
    let moved = Asker::new(None, address("198.51.100.9"));
    let strict = Limits {
        by_address: true,
        ..LIMITS
    };

    assert!(waiting(&codes, &handle, &moved, &LIMITS, now()).is_some());
    assert_eq!(waiting(&codes, &handle, &moved, &strict, now()), None);
    assert!(waiting(&codes, &handle, &phone, &strict, now()).is_some());
}

#[test]
fn a_code_is_let_go_once_spent_or_lapsed() {
    let codes = SignInCodes::default();
    let phone = Asker::new(None, address("192.0.2.7"));
    let spent = issued(&codes, &phone, "spent", &Chance::turning(1));
    let lapsing = issued(&codes, &phone, "lapsing", &Chance::turning(9));

    if let Some(claimed) = codes.claimed(&spent, &phone, &LIMITS, now()) {
        claimed.spent();
    }

    assert_eq!(waiting(&codes, &spent, &phone, &LIMITS, now()), None);
    assert_eq!(
        waiting(&codes, &lapsing, &phone, &LIMITS, now() + LAPSES),
        None
    );
}

#[test]
fn one_source_flooding_meets_its_own_limit_while_another_is_still_issued_codes() {
    let codes = SignInCodes::default();
    let flooding = Asker::new(None, address("192.0.2.7"));
    let random = Chance::turning(1);
    for id in ["one", "two", "three"] {
        assert_eq!(codes.asking(&flooding, &LIMITS, now()), Ok(()));
        issued(&codes, &flooding, id, &random);
    }

    assert_eq!(codes.asking(&flooding, &LIMITS, now()), Err(LAPSES));
    assert_eq!(
        codes.kept(
            &random,
            ("four".to_owned(), flooding.clone(), now() + LAPSES),
            &LIMITS,
            now()
        ),
        Err(NotKept::Waiting(LAPSES))
    );
    let other = Asker::new(None, address("198.51.100.9"));
    assert_eq!(codes.asking(&other, &LIMITS, now()), Ok(()));
    assert!(!issued(&codes, &other, "theirs", &random).is_empty());
}

#[test]
fn a_source_asking_past_its_period_waits_until_its_oldest_ask_leaves_it() {
    let codes = SignInCodes::default();
    let asker = Asker::new(Some("a-session"), None);
    let tight = Limits {
        per_period: 2,
        ..LIMITS
    };
    let later = now() + Duration::from_secs(60);

    assert_eq!(codes.asking(&asker, &tight, now()), Ok(()));
    assert_eq!(codes.asking(&asker, &tight, later), Ok(()));
    assert_eq!(
        codes.asking(&asker, &tight, later),
        Err(tight.period.saturating_sub(Duration::from_secs(60)))
    );
    assert_eq!(codes.asking(&asker, &tight, now() + tight.period), Ok(()));
}

#[test]
fn addresses_on_one_ipv6_network_are_one_source_and_no_address_is_one_source_too() {
    let codes = SignInCodes::default();
    let one = Limits {
        per_period: 1,
        ..LIMITS
    };
    let pairs = [
        ("2001:db8:1:2::1", "2001:db8:1:2::ffff"),
        ("192.0.2.7", "::ffff:192.0.2.7"),
    ];
    for (first, second) in pairs {
        assert_eq!(
            codes.asking(&Asker::new(None, address(first)), &one, now()),
            Ok(())
        );
        assert!(codes
            .asking(&Asker::new(None, address(second)), &one, now())
            .is_err());
    }
    assert_eq!(codes.asking(&Asker::new(None, None), &one, now()), Ok(()));
    assert!(codes.asking(&Asker::new(None, None), &one, now()).is_err());
    assert_eq!(
        codes.asking(&Asker::new(None, address("2001:db8:1:3::1")), &one, now()),
        Ok(())
    );
}

#[test]
fn no_code_is_kept_where_no_handle_can_be_drawn() {
    let codes = SignInCodes::default();
    let asker = Asker::new(None, None);

    assert_eq!(
        codes.kept(
            &Chance::exactly(None),
            ("id".to_owned(), asker.clone(), now() + LAPSES),
            &LIMITS,
            now()
        ),
        Err(NotKept::Undrawn)
    );
    assert_eq!(
        codes.kept(
            &Chance::exactly(Some(vec![1; 3])),
            ("id".to_owned(), asker, now() + LAPSES),
            &LIMITS,
            now()
        ),
        Err(NotKept::Undrawn)
    );
}

#[test]
fn a_code_one_finish_has_claimed_is_refused_to_every_other_until_it_is_let_go() {
    let codes = SignInCodes::default();
    let phone = Asker::new(None, address("192.0.2.7"));
    let handle = issued(&codes, &phone, "adapter-id", &Chance::turning(1));

    let first = codes.claimed(&handle, &phone, &LIMITS, now());
    let second = codes.claimed(&handle, &phone, &LIMITS, now());

    assert!(first.is_some());
    assert!(second.is_none());
    drop(first);
    let again = codes.claimed(&handle, &phone, &LIMITS, now());
    assert!(again.is_some());
    if let Some(again) = again {
        again.spent();
    }
    assert!(codes.claimed(&handle, &phone, &LIMITS, now()).is_none());
}

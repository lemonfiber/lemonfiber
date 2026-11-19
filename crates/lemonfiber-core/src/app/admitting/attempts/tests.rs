use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, SystemTime};

use super::{Attempts, Door};

/// The moment these run at.
fn moment() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_180_000)
}

/// A device on the household network, by the last part of its address.
fn a_device(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

/// `seconds` after the moment these run at.
fn later(seconds: u64) -> SystemTime {
    moment() + Duration::from_secs(seconds)
}

/// A wrong answer from `from`, taken and left wrong, or how long is left first.
async fn guessed(attempts: &Attempts, from: IpAddr, at: SystemTime) -> Result<(), Duration> {
    attempts.taken(from, None, at).await.map(|_| ())
}

#[tokio::test]
async fn wrong_answers_are_free_for_a_while_and_then_made_to_wait() {
    let attempts = Attempts::default();

    for _ in 0..3u8 {
        assert_eq!(guessed(&attempts, a_device(7), moment()).await, Ok(()));
    }
    assert_eq!(attempts.waiting(a_device(7), moment()).await, None);

    assert_eq!(guessed(&attempts, a_device(7), moment()).await, Ok(()));
    let owed = attempts.waiting(a_device(7), moment()).await;
    assert!(owed.is_some_and(|left| left > Duration::ZERO), "{owed:?}");
    assert!(guessed(&attempts, a_device(7), moment()).await.is_err());

    assert_eq!(attempts.waiting(a_device(7), later(600)).await, None);
}

#[tokio::test]
async fn a_members_right_answer_forgives_nothing_but_itself() {
    let attempts = Attempts::default();

    for round in 0..3u64 {
        for _ in 0..3u8 {
            let _ = guessed(&attempts, a_device(7), later(round)).await;
        }
        let Ok(ticket) = attempts.taken(a_device(7), Some("Ana"), later(round)).await else {
            continue;
        };
        attempts
            .right(&ticket, Door::Member("ana".to_owned()), later(round))
            .await;
    }

    assert!(
        attempts.waiting(a_device(7), later(2)).await.is_some(),
        "a member signing in forgave their guesses at the operator's password"
    );
}

#[tokio::test]
async fn one_device_guessing_keeps_nobody_else_waiting() {
    let attempts = Attempts::default();

    let mut second = 0;
    while second < 2 * 60 * 60 {
        let at = later(second);
        match guessed(&attempts, a_device(66), at).await {
            Ok(()) => {}
            Err(left) => second += left.as_secs(),
        }
        let stranger = IpAddr::V6(std::net::Ipv6Addr::from(u128::from(second) + 1));
        let Ok(ticket) = attempts.taken(stranger, None, at).await else {
            unreachable!("a device guessing kept another out at second {second}");
        };
        assert!(
            ticket.operator,
            "a stranger was kept from the operator's door"
        );
        attempts.right(&ticket, Door::Operator, at).await;
        second += 1;
    }
}

#[tokio::test]
async fn many_addresses_at_once_shut_the_door_to_strangers_and_nobody_else() {
    let attempts = Attempts::default();
    let Ok(ticket) = attempts.taken(a_device(1), None, moment()).await else {
        unreachable!("the first attempt of the run is taken");
    };
    attempts.right(&ticket, Door::Operator, moment()).await;
    let Ok(ticket) = attempts.taken(a_device(2), Some("Ana"), moment()).await else {
        unreachable!("the second attempt of the run is taken");
    };
    attempts
        .right(&ticket, Door::Member("ana".to_owned()), moment())
        .await;

    let mut refused = None;
    for last in 10..=250u8 {
        if let Err(left) = guessed(&attempts, a_device(last), moment()).await {
            refused = Some(left);
            break;
        }
    }
    assert!(
        refused.is_some_and(|left| left > Duration::ZERO),
        "a stranger was let in however many addresses guessed"
    );

    let operator = attempts.taken(a_device(1), None, moment()).await;
    assert!(
        operator.as_ref().is_ok_and(|ticket| ticket.operator),
        "{operator:?}"
    );
    let member = attempts.taken(a_device(2), Some("ANA"), moment()).await;
    assert!(
        member.as_ref().is_ok_and(
            |ticket| !ticket.operator && ticket.member == Some(Door::Member("ana".to_owned()))
        ),
        "{member:?}"
    );

    assert_eq!(guessed(&attempts, a_device(251), later(60)).await, Ok(()));
}

#[tokio::test]
async fn wrong_answers_are_forgotten_with_quiet() {
    let attempts = Attempts::default();
    let mut second = 0;
    for _ in 0..8u8 {
        while let Err(left) = guessed(&attempts, a_device(7), later(second)).await {
            second += left.as_secs();
        }
    }
    assert!(attempts.waiting(a_device(7), later(second)).await.is_some());

    let rested = second + 60 * 60;
    for _ in 0..3u8 {
        assert_eq!(guessed(&attempts, a_device(7), later(rested)).await, Ok(()));
    }
    assert_eq!(attempts.waiting(a_device(7), later(rested)).await, None);
}

#[tokio::test]
async fn an_address_arriving_as_ipv6_is_the_same_address() {
    let attempts = Attempts::default();
    let IpAddr::V4(device) = a_device(7) else {
        unreachable!("the device is written as IPv4");
    };
    for _ in 0..4u8 {
        let _ = guessed(&attempts, a_device(7), moment()).await;
    }
    assert!(attempts
        .waiting(IpAddr::V6(device.to_ipv6_mapped()), moment())
        .await
        .is_some());
}

/// Answers arriving together are counted before any of them is checked, so the free
/// ones are free once each rather than once per request in flight.
#[tokio::test]
async fn answers_arriving_together_are_each_counted_before_any_is_checked() {
    let attempts = Attempts::default();

    let taken = futures_util::future::join_all(
        (0..10u8).map(|_| attempts.taken(a_device(7), None, moment())),
    )
    .await;

    let let_through = taken.iter().filter(|one| one.is_ok()).count();
    assert!(
        let_through < 10,
        "every one of ten at once was let through: {taken:?}"
    );
    assert!(
        attempts.waiting(a_device(7), moment()).await.is_some(),
        "ten answers at once earned no wait"
    );
}

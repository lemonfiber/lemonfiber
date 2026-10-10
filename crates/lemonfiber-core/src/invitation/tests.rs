use super::{
    closed, comparable, lapsed_unseen, offered, recorded, run_out, Offer, Offered, Offers,
};
use crate::ports::service::{Access, Invited, Member};

fn member(id: &str, name: &str, claimed: bool) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        claimed,
        ..Member::default()
    }
}

fn invited(id: &str, at: &str) -> Invited {
    Invited {
        member: id.to_owned(),
        at: at.to_owned(),
    }
}

fn offer(offered: &str) -> Offer {
    Offer {
        offered: offered.to_owned(),
        lapses: String::new(),
        decline: None,
        claim: None,
    }
}

fn names(spent: &[Offered]) -> Vec<&str> {
    spent.iter().map(|o| o.member.name.as_str()).collect()
}

#[test]
fn only_the_accounts_somebody_could_still_claim_are_invitations() {
    let switched_off = Member {
        access: Access {
            disabled: true,
            ..Access::default()
        },
        ..member("4", "di", false)
    };
    let runs_the_server = Member {
        access: Access {
            administrator: true,
            ..Access::default()
        },
        ..member("5", "admin", false)
    };
    let household = vec![
        member("1", "ana", false),
        member("2", "bo", true),
        member("3", "cy", false),
        switched_off,
        runs_the_server,
    ];
    let records = [invited("1", "2026-08-29T09:00:00.0000000Z")];

    let waiting = offered(household, &records, &Offers::new());

    assert_eq!(
        names(&waiting),
        ["ana", "cy"],
        "an account nobody can claim was read as an invitation"
    );
    assert_eq!(
        waiting.first().and_then(|o| o.at.as_deref()),
        Some("2026-08-29T09:00:00.0000000Z")
    );
    assert!(
        waiting.get(1).is_some_and(|o| o.at.is_none()),
        "an invitation nothing records was given a date anyway"
    );
}

/// The one the sweep used to leave alone: an invitation nobody can date has run out.
///
/// The server's record is bounded and can be pushed along, so an account that fell out
/// of it would otherwise stand claimable for good. A record that is missing, empty, or
/// written in a shape this cannot order is the same answer — nothing dates it.
#[test]
fn an_invitation_that_cannot_be_dated_has_run_out() {
    let household = vec![
        member("1", "ana", false),
        member("2", "bo", false),
        member("3", "cy", false),
    ];
    let records = [invited("2", ""), invited("3", "last tuesday")];

    let waiting = offered(household, &records, &Offers::new());
    let spent = run_out(&waiting, "2026-08-27T09:00:00Z");

    assert_eq!(
        names(&spent.withdrawn),
        ["ana", "bo", "cy"],
        "an invitation nothing could date was left standing"
    );
}

#[test]
fn an_invitation_older_than_the_window_has_run_out() {
    let waiting = vec![
        Offered {
            member: member("1", "ana", false),
            at: Some("2026-08-01T09:00:00.0000000Z".to_owned()),
        },
        Offered {
            member: member("2", "bo", false),
            at: Some("2026-08-29T09:00:00.0000000Z".to_owned()),
        },
    ];

    let spent = run_out(&waiting, "2026-08-27T09:00:00Z");

    assert_eq!(
        names(&spent.withdrawn),
        ["ana"],
        "the wrong invitations were called finished"
    );
    assert_eq!(spent.every().count(), 1);
}

/// An account somebody has been in is switched off when its window closes, not removed.
///
/// A reset returns a member's account to having no password; removing it when nobody
/// claims it again in time would take what they watched with it.
#[test]
fn a_reset_that_runs_out_is_switched_off_rather_than_removed() {
    let been_in = Member {
        last_seen: Some("2026-03-01T10:00:00.0000000Z".to_owned()),
        ..member("1", "ana", false)
    };
    let waiting = vec![
        Offered {
            member: been_in,
            at: Some("2026-08-01T09:00:00Z".to_owned()),
        },
        Offered {
            member: member("2", "bo", false),
            at: None,
        },
    ];

    let spent = run_out(&waiting, "2026-08-27T09:00:00Z");

    assert_eq!(names(&spent.suspended), ["ana"]);
    assert_eq!(names(&spent.withdrawn), ["bo"]);
    assert_eq!(
        spent
            .every()
            .map(|o| o.member.name.as_str())
            .collect::<Vec<_>>(),
        ["bo", "ana"]
    );
}

/// An account offered again is dated by the later offer, not by its making.
///
/// **The window only exists because of this.** A reset writes a second record
/// against an account that already carries one for being made, and the two can be
/// months apart. Read as the first, every reset invitation is expired the instant it
/// is made and the next `invite` takes it back.
///
/// Both entries are what `jellyfin/jellyfin:10.10.3` actually records: `UserCreated`
/// when the account is made, `UserPasswordChanged` when a password moves on or off.
#[test]
fn an_account_offered_again_is_dated_by_the_later_offer() {
    let household = vec![member("1", "ana", false)];
    let records = [
        invited("1", "2026-01-04T09:00:00.0000000Z"),
        invited("1", "2026-08-30T10:00:00.0000000Z"),
    ];

    let waiting = offered(household, &records, &Offers::new());

    assert_eq!(
        waiting.first().and_then(|o| o.at.as_deref()),
        Some("2026-08-30T10:00:00.0000000Z"),
        "the invitation was dated by when the account was made rather than by when \
         it was last offered"
    );
    assert_eq!(
        run_out(&waiting, "2026-08-27T09:00:00Z").every().count(),
        0,
        "an account offered two days ago was taken back as an eight-month-old \
         invitation nobody claimed"
    );
}

/// What this program recorded offering dates an invitation the server no longer does.
///
/// The server's record can be pushed along until an offer falls out of it; the one kept
/// here cannot, so an invitation it covers keeps its window.
#[test]
fn an_offer_recorded_here_dates_what_the_server_forgot() {
    let household = vec![member("1", "ana", false), member("2", "bo", false)];
    let offers: Offers = [
        ("1".to_owned(), offer("2026-08-30T10:00:00Z")),
        ("2".to_owned(), offer("2026-01-04T09:00:00Z")),
    ]
    .into_iter()
    .collect();
    let records = [invited("2", "2026-08-30T11:00:00.0000000Z")];

    let waiting = offered(household, &records, &offers);

    assert_eq!(
        waiting.first().and_then(|o| o.at.as_deref()),
        Some("2026-08-30T10:00:00Z"),
        "an offer recorded here was ignored"
    );
    assert_eq!(
        waiting.get(1).and_then(|o| o.at.as_deref()),
        Some("2026-08-30T11:00:00.0000000Z"),
        "the later of the two dates was not the one taken"
    );
}

/// An offer made at `offered` that runs out at `lapses`.
fn lasting(offered: &str, lapses: &str) -> Offer {
    Offer {
        lapses: lapses.to_owned(),
        ..offer(offered)
    }
}

/// The record keeps the offers still out, and the one just made.
#[test]
fn the_record_keeps_only_the_offers_still_out() {
    let household = vec![member("1", "ana", false), member("2", "bo", true)];
    let offers: Offers = [
        ("1".to_owned(), offer("2026-08-01T10:00:00Z")),
        (
            "2".to_owned(),
            lasting("2026-08-29T10:00:00Z", "2026-08-31T10:00:00Z"),
        ),
        ("3".to_owned(), offer("2026-08-01T10:00:00Z")),
    ]
    .into_iter()
    .collect();

    let kept = recorded(offers, &household, "4", offer("2026-08-30T10:00:00Z"));

    assert_eq!(
        kept.keys().map(String::as_str).collect::<Vec<_>>(),
        ["1", "4"],
        "a claimed or removed account was kept, or the new offer was not"
    );
}

#[test]
fn only_a_moment_written_as_the_server_writes_it_is_ordered() {
    assert!(comparable("2026-08-29T09:00:00.0000000Z"));
    assert!(comparable("2026-08-29T09:00:00Z"));
    assert!(!comparable("2026-08-29T09:00:00"), "no zone");
    assert!(!comparable("2026-08-29Z"), "too short to be an instant");
    assert!(!comparable(""), "nothing at all");
}

/// An account claimed only after its offer ran out is kept on the record, so offering
/// somebody else does not clear it and admit whoever claimed it.
#[test]
fn an_account_claimed_after_its_offer_ran_out_stays_on_the_record() {
    let household = vec![member("2", "bo", true)];
    let offers: Offers = [(
        "2".to_owned(),
        lasting("2026-08-01T10:00:00Z", "2026-08-03T10:00:00Z"),
    )]
    .into_iter()
    .collect();

    let kept = recorded(offers, &household, "4", offer("2026-08-30T10:00:00Z"));

    assert_eq!(
        kept.keys().map(String::as_str).collect::<Vec<_>>(),
        ["2", "4"],
        "an offer claimed after it ran out was cleared by the next one"
    );
}

#[test]
fn an_offer_taken_up_in_time_is_closed_and_one_still_out_is_not() {
    let household = vec![
        member("1", "ana", true),
        member("2", "bo", false),
        member("3", "cy", true),
    ];
    let offers: Offers = [
        (
            "1".to_owned(),
            lasting("2026-08-29T10:00:00Z", "2026-08-31T10:00:00Z"),
        ),
        (
            "2".to_owned(),
            lasting("2026-08-29T10:00:00Z", "2026-08-31T10:00:00Z"),
        ),
        (
            "3".to_owned(),
            lasting("2026-08-01T10:00:00Z", "2026-08-03T10:00:00Z"),
        ),
        (
            "9".to_owned(),
            lasting("2026-08-29T10:00:00Z", "2026-08-31T10:00:00Z"),
        ),
    ]
    .into_iter()
    .collect();

    let open = closed(offers, &household, "2026-08-30T10:00:00Z");

    assert_eq!(
        open.keys().map(String::as_str).collect::<Vec<_>>(),
        ["2", "3"],
        "the record did not keep exactly the unclaimed offer and the one claimed late"
    );
}

#[test]
fn an_offer_still_out_past_its_end_is_one_nobody_was_seen_to_take_up() {
    let offers: Offers = [
        (
            "1".to_owned(),
            lasting("2026-08-01T10:00:00Z", "2026-08-03T10:00:00Z"),
        ),
        (
            "2".to_owned(),
            lasting("2026-08-29T10:00:00Z", "2026-08-31T10:00:00Z"),
        ),
        ("3".to_owned(), offer("2026-08-29T10:00:00Z")),
    ]
    .into_iter()
    .collect();
    let now = "2026-08-30T10:00:00Z";

    assert!(lapsed_unseen(&offers, "1", now), "a lapsed offer");
    assert!(!lapsed_unseen(&offers, "2", now), "an offer still standing");
    assert!(
        lapsed_unseen(&offers, "3", now),
        "an offer whose end cannot be read"
    );
    assert!(
        !lapsed_unseen(&offers, "4", now),
        "an account never offered"
    );
}

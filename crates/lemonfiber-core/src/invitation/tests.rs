use super::{comparable, offered, run_out, Offered};
use crate::ports::service::{Invited, Member};

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

#[test]
fn only_the_accounts_nobody_has_claimed_are_invitations() {
    let household = vec![
        member("1", "ana", false),
        member("2", "bo", true),
        member("3", "cy", false),
    ];
    let records = [invited("1", "2026-08-29T09:00:00.0000000Z")];

    let waiting = offered(household, &records);

    assert_eq!(
        waiting
            .iter()
            .map(|o| o.member.name.as_str())
            .collect::<Vec<_>>(),
        ["ana", "cy"],
        "somebody who has already joined was reported as not having"
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

/// The one that must not fire: an invitation nobody can date stays.
///
/// Withdrawing on a guess takes away an account somebody is about to use. A
/// record that is missing, empty, or written in a shape this cannot order is
/// the same answer — it does not know — and the same behaviour follows.
#[test]
fn an_invitation_that_cannot_be_dated_is_left_standing() {
    let undatable = vec![
        Offered {
            member: member("1", "ana", false),
            at: None,
        },
        Offered {
            member: member("2", "bo", false),
            at: Some(String::new()),
        },
        Offered {
            member: member("3", "cy", false),
            at: Some("last tuesday".to_owned()),
        },
    ];

    assert!(
        run_out(&undatable, "2030-01-01T00:00:00Z").is_empty(),
        "an invitation nobody could date was withdrawn on a guess"
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

    let done = run_out(&waiting, "2026-08-27T09:00:00Z");

    assert_eq!(
        done.iter()
            .map(|o| o.member.name.as_str())
            .collect::<Vec<_>>(),
        ["ana"],
        "the wrong invitations were called finished"
    );
}

/// An account offered again is dated by the later offer, not by its making.
///
/// **The window only exists because of this.** A reset writes a second record
/// against an account that already carries one for being made, and the two can be
/// months apart. Read as the first, every reset invitation is expired the instant it
/// is made and the next `invite` withdraws it — which is not expiry, it is deletion
/// with no window at all.
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

    let waiting = offered(household, &records);

    assert_eq!(
        waiting.first().and_then(|o| o.at.as_deref()),
        Some("2026-08-30T10:00:00.0000000Z"),
        "the invitation was dated by when the account was made rather than by when \
         it was last offered"
    );
    assert!(
        run_out(&waiting, "2026-08-27T09:00:00Z").is_empty(),
        "an account offered two days ago was withdrawn as an eight-month-old \
         invitation nobody claimed"
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

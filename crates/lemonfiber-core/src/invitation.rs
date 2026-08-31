//! What an invitation is, and when it has run out.
//!
//! An invitation is an account with no password on it. That is the whole of it:
//! made by the operator, claimed by whoever sets the first password, and withdrawn
//! if nobody does. Nothing about it is written down here — the media server already
//! holds both halves, so an invitation outlives this program being closed,
//! reinstalled, or run from another machine.
//!
//! **An invitation nobody can date is left alone.** The record of when an account
//! was made is kept apart from the account itself and does not last forever, so an
//! unclaimed account with no such record is one this cannot reason about — somebody
//! made it by hand, or it is older than what the server still remembers. Withdrawing
//! it on a guess would take away an account somebody is about to use, which is worse
//! than leaving one standing a while longer.

use crate::ports::service::{Invited, Member};

/// How long an invitation stands before it is withdrawn.
pub const HOURS_TO_CLAIM: i64 = 48;

/// How far back the record is read when dating invitations.
///
/// **This is not the same moment as [`HOURS_TO_CLAIM`] and must be longer.** The
/// server answers with what happened *since* the moment it is given, and the
/// invitations being looked for are the ones already past their window — so
/// reading from the same moment they are judged against returns only the ones
/// still standing, and nothing is ever found to withdraw.
///
/// Beyond this the record is not read, and an invitation it does not cover is one
/// nothing can date, which is left standing rather than withdrawn on a guess. The
/// server trims its own record eventually, so no window makes that case go away.
pub const HOURS_OF_RECORD: i64 = 24 * 30;

/// An account nobody has claimed, with when it was offered where that is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The account it is about.
    pub member: Member,
    /// When it was made, absent where nothing records it any longer.
    pub at: Option<String>,
}

/// The invitations among a household: the accounts nobody has claimed.
///
/// A claimed account is a member, not an invitation, and is not reported here — the
/// operator asking who has not joined yet is asking a different question from who
/// is in the house.
#[must_use]
pub fn offered(household: Vec<Member>, invited: &[Invited]) -> Vec<Offered> {
    household
        .into_iter()
        .filter(|member| !member.claimed)
        .map(|member| {
            let at = invited
                .iter()
                .find(|record| record.member == member.id)
                .map(|record| record.at.clone());
            Offered { member, at }
        })
        .collect()
}

/// The invitations that have run out, and may be withdrawn.
///
/// `now` and each `at` are compared as the server writes them, which sorts
/// correctly as text because they are fixed-width and end in `Z`. Comparing the
/// strings rather than parsing them keeps this free of a clock and of a date
/// library, and a record this cannot compare is one it declines to act on.
#[must_use]
pub fn run_out<'a>(offered: &'a [Offered], cutoff: &str) -> Vec<&'a Offered> {
    offered
        .iter()
        .filter(|invitation| {
            invitation
                .at
                .as_deref()
                .is_some_and(|made| comparable(made) && made < cutoff)
        })
        .collect()
}

/// Whether a recorded moment is one this can order against another.
///
/// The server writes an ISO-8601 instant ending in `Z`. Anything else — a local
/// time, an empty string, a shape a later version invents — is not something to
/// compare as text, and an invitation carrying one is left standing.
fn comparable(moment: &str) -> bool {
    moment.ends_with('Z') && moment.len() >= "2026-08-29T00:00:00Z".len()
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn only_a_moment_written_as_the_server_writes_it_is_ordered() {
        assert!(comparable("2026-08-29T09:00:00.0000000Z"));
        assert!(comparable("2026-08-29T09:00:00Z"));
        assert!(!comparable("2026-08-29T09:00:00"), "no zone");
        assert!(!comparable("2026-08-29Z"), "too short to be an instant");
        assert!(!comparable(""), "nothing at all");
    }
}

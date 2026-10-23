//! What the whole house is told, put where the house will read it.
//!
//! [`handing_over`](super::handing_over) writes to one member and hands the message to
//! the operator, because most of what a member is owed is about *them* — what their
//! period has left, what they are waiting on, why something of theirs was refused — and
//! there is one page where they ask, seen by everybody. Two sentences are not about
//! anybody in particular: whether the house has room for new things, and the shape of the
//! period a limit is counted over. Those are true of the house, so those can go on the one
//! page, in the household's words: no size, no service and nothing about how the house is
//! run.
//!
//! **The period's shape is the house's fact and its date is one person's.** The request
//! service counts over a window that rolls, and the only sentence it offers about that is
//! `You are allowed to request {limit} {type} every {days} days` — which reads as a day of
//! the week that comes round, so a member who has run out waits for a reset that never
//! happens. When one more becomes possible is *their* earliest request ageing out and
//! belongs in the message handed over; that room comes back a request at a time rather
//! than all at once is true of everybody here alike, and it is the half that stops the
//! waiting.
//!
//! **The wording is not the same wording, and that is the medium rather than a second
//! opinion.** The request service shows a notice as a heading on one line it clips rather
//! than wraps: about seventy characters at a wide window and **twenty-eight** at the
//! width of a telephone, measured in a browser against `ghcr.io/seerr-team/seerr:v3.3.0`
//! rather than guessed. So a notice leads with the fact that changes a decision and lets
//! the qualification be the half that is lost. The message handed to the operator has no
//! width to fit and says the whole thing.

use crate::ports::service::Noticing;

/// What the house is showing everybody who is about to ask for something.
///
/// **No room takes the period's place rather than standing beside it.** Nothing is
/// fetched while there is no room, so how a limit frees up is not the question in front of
/// anybody — and the two lines together read as the one thing the house's own sentence
/// exists to deny, which is that a full house is somebody's allowance.
#[must_use]
pub(super) fn for_the_house(no_room: bool, limited: bool) -> Vec<String> {
    if no_room {
        vec![NO_ROOM.to_owned()]
    } else if limited {
        vec![RETURNS.to_owned()]
    } else {
        Vec::new()
    }
}

/// What the house says while it has no room, in the house's own words and never as
/// somebody's limit.
///
/// The limit is named only to be ruled out: waiting for a period to roll over is the one
/// move a member would otherwise make, and it is the one that cannot help.
const NO_ROOM: &str = "No room for new things right now — not your limit";

/// How the period behaves, for a house where anything is counted over one.
///
/// **No figure, deliberately.** The request service already states how many and over how
/// long, and it states them per person — so a house whose default is a week carries
/// members held to a month, and a number here would be the wrong number for exactly those
/// people. What it never states is the shape, which is the same for all of them.
const RETURNS: &str = "Room returns one at a time, as your old requests age out";

/// Put the house's notices in front of the household, and say so where it did not work.
///
/// Answers with what went wrong rather than failing: this rides along on a reading, and a
/// household list that refused to be read because a notice could not be hung would report
/// nothing about anybody over something nobody asked for.
pub(super) async fn put_where_they_ask(
    requests: &dyn Noticing,
    no_room: bool,
    limited: bool,
    dry_run: bool,
) -> Option<String> {
    if dry_run {
        return None;
    }
    requests
        .set_notices(&for_the_house(no_room, limited))
        .await
        .err()
        .map(|_| {
            "the request service would not carry what the household is told before they \
             ask, so how a limit frees up and whether the disk has room are shown here and \
             not where they ask"
                .to_owned()
        })
}

#[cfg(test)]
pub(super) mod tests;

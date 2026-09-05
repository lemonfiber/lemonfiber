//! Holding the household back while the disk has no room, and letting it go again.
//!
//! [`notices`](super::notices) puts the sentence where the house will read it; this stops
//! the thing the sentence is about. They go up and come down on the one reading, which is
//! the whole of why the block is not a silent one. A request service offers no way to
//! refuse in words of somebody else's choosing — what it offers is a permission, and a
//! permission taken away is a button that is simply gone. Beside a heading at the top of
//! the page saying the disk has no room and that this is nobody's limit, that same missing
//! button is a refusal with a reason.
//!
//! **Not through a quota, deliberately.** The requirement is that a full disk refuses in
//! the disk's own words and never as somebody's allowance, and the service keeps the two
//! apart in its own sentences: `Movie Quota exceeded.` for one, `You do not have
//! permission to make movie requests.` for the other. Reaching for the quota to stop the
//! asking would put the disk's refusal into the words of a limit and send a household off
//! to raise one — the one move that cannot help.
//!
//! **What was taken is written down, because giving it back is not a grant.** What a
//! household may ask for is the operator's to decide and not this program's, so what goes
//! back is the number that came off and nothing else: a member who could only ask for the
//! higher quality gets that, and a permission the operator narrowed meanwhile stays
//! narrowed.
//!
//! **It lets go on the reading it holds on, and on no other.** This program has no clock;
//! the sweeps it does happen on the way past a command somebody typed. A house whose disk
//! fills and empties again while nobody reads the household stays held back until somebody
//! does — the same cadence the notice beside it already runs on, and the reason the two are
//! written in one pass rather than two.

use std::collections::BTreeMap;

use crate::app::{record, targets, Ctx};
use crate::ports::service::{Approving, Holding};

/// What the record is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the name
/// would look exactly like a household nobody had ever held back.
const NAME: &str = "held-back.json";

/// Who has had their asking taken away, and what came off each of them.
///
/// Keyed by the request service's own identifier, because that is what gives it back.
type HeldBack = BTreeMap<String, u64>;

/// What is said where the service would not take the household's asking away.
const STILL_ASKING: &str = "the request service would not stop the household asking while \
                            the disk has no room, so something may still be asked for that \
                            cannot be fetched — read the household again once it answers";

/// What is said where the service would not give it back.
const STILL_HELD: &str = "the request service would not give the household back what it \
                          may ask for now that there is room, so somebody may still find \
                          they cannot ask — read the household again once it answers";

/// What is said where what is held back could not be written down beside the settings.
const NOT_WRITTEN_DOWN: &str = "what the disk is holding back could not be written down \
                                beside the settings, so the next reading will work from an \
                                older answer — check in the request service that everybody \
                                who should be able to ask still can";

/// Hold the household back or let it go, according to where the disk stands.
///
/// Answers with what went wrong rather than failing, for the reason the notice beside it
/// does: this rides along on a reading, and a household list that refused to be read
/// because a permission would not be written would report nothing about anybody.
pub(super) async fn as_the_disk_stands(
    ctx: &Ctx,
    seerr: &dyn Approving,
    members: &[&str],
    no_room: bool,
    dry_run: bool,
) -> Vec<String> {
    if dry_run {
        return Vec::new();
    }
    let was: HeldBack = record::beside(ctx, NAME);
    let (now, mut said) = if no_room {
        taking_away(seerr, members, was.clone()).await
    } else {
        giving_back(seerr, was.clone()).await
    };
    // Written only where it changed. This runs on every reading of the household, and a
    // file rewritten on every glance is one a backup and a watcher both see move for
    // nothing.
    if now != was {
        said.extend(
            record::keep(targets::beside_env(ctx, NAME).as_deref(), &now)
                .err()
                .map(|_| NOT_WRITTEN_DOWN.to_owned()),
        );
    }
    said
}

/// Take the asking from everybody who still has it, adding what came off to the record.
///
/// Merged into what is already written down rather than replacing it: this runs again on
/// every reading while the disk stays full, and the second run finds nothing left to take
/// from anybody the first took from.
async fn taking_away(
    seerr: &dyn Approving,
    members: &[&str],
    mut held: HeldBack,
) -> (HeldBack, Vec<String>) {
    let mut said = Vec::new();
    for member in members {
        match seerr.hold_requests(member).await {
            Ok(came_off) if came_off.anything() => {
                *held.entry((*member).to_owned()).or_default() |= came_off.taken;
            }
            // Nothing to take is nobody held back: an owner, whose permissions this
            // service reads past anyway, and somebody already held back on an earlier
            // reading. Writing either down would be a line to give something back to
            // that was never taken.
            Ok(_) => {}
            Err(_) => said.push(STILL_ASKING.to_owned()),
        }
    }
    said.dedup();
    (held, said)
}

/// Give everybody back exactly what was taken, and forget those who got it.
///
/// Whoever the service would not answer for keeps their line, so the next reading tries
/// again rather than leaving somebody unable to ask for good.
async fn giving_back(seerr: &dyn Approving, held: HeldBack) -> (HeldBack, Vec<String>) {
    let mut still = HeldBack::new();
    let mut said = Vec::new();
    for (member, taken) in held {
        if seerr
            .release_requests(&member, Holding { taken })
            .await
            .is_err()
        {
            still.insert(member, taken);
            said.push(STILL_HELD.to_owned());
        }
    }
    said.dedup();
    (still, said)
}

#[cfg(test)]
mod tests {
    use super::{NOT_WRITTEN_DOWN, STILL_ASKING, STILL_HELD};

    /// Each sentence says which way round it went wrong, and what to do next.
    ///
    /// The two directions are different things for an operator to act on — a household
    /// still able to ask for what cannot be fetched, and a household unable to ask at
    /// all — and a message that could be either leaves them reading permissions to find
    /// out which.
    #[test]
    fn each_sentence_says_which_way_round_it_went_wrong_and_what_to_do() {
        assert!(STILL_ASKING.contains("would not stop the household asking"));
        assert!(STILL_ASKING.contains("read the household again"));
        assert!(STILL_HELD.contains("give the household back"));
        assert!(STILL_HELD.contains("read the household again"));
        assert!(NOT_WRITTEN_DOWN.contains("older answer"));
        assert!(NOT_WRITTEN_DOWN.contains("check in the request service"));
    }
}

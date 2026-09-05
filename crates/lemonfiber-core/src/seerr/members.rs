//! The accounts the request service holds for the household, and what each may do.
//!
//! Its shapes and its numbers rather than this product's: where accounts are read,
//! where they are given, and the one field that says whether what somebody asks for
//! arrives without anybody seeing it.
//!
//! In a file of its own for the reason the records next door are: `seerr.rs` is the
//! client — signing in, pointing the service at things, asking it questions — and this
//! is the vocabulary two of those questions are asked and answered in.
//!
//! **Nothing here is a second copy of a household.** Only the identifier and the
//! permissions are read; what somebody is called and what they may watch are the media
//! server's to say, and a copy kept here would be a copy able to disagree with it.

use serde::Deserialize;

/// Where media-server accounts are given accounts here.
///
/// It reads the media server itself, using the credentials this service was set up
/// with, so what is sent is identifiers and not accounts. **A member it already holds
/// is skipped**, which is what lets every member be sent on every run.
pub(super) const LINK_MEMBERS: &str = "/user/import-from-jellyfin";

/// What this service answers when it holds no account for somebody.
///
/// Its own word for "I have never heard of this person", which for a removal is an
/// answer rather than a fault — and for a restriction is nothing to hold rather than a
/// failure to hold something.
pub(super) const NOT_FOUND: u16 = 404;

/// Where the accounts this service holds are read and removed.
///
/// The lookup is by the **media server's** identifier rather than this service's own,
/// because that is the one lemonfiber holds: it made the account over there. The service
/// normalises it — dashes stripped, lower-cased, and refused unless it is 32 hex
/// characters — so what goes must be a media-server account identifier and nothing else.
pub(super) const MEMBERS: &str = "/user";

/// Where one member's permissions are read and written, under their account.
///
/// The narrow endpoint rather than the whole account: a write that carried the account
/// would put back every field it did not name, which is the defect the media server's
/// own policy write already taught this stack once.
pub(super) const PERMISSIONS: &str = "settings/permissions";

/// The permissions under which what somebody asks for arrives without anybody seeing it.
///
/// Read off `/app/dist/lib/permissions.js` inside the pinned image rather than recalled:
/// `ADMIN`, `AUTO_APPROVE`, and that last one's four narrower forms.
///
/// **`ADMIN` is among them deliberately.** This service treats it as holding every other
/// permission, so an administrator approves their own requests whether or not the
/// approval bits are set — and a reading that left it out would call a household's most
/// powerful account its most restricted one.
const APPROVES_OWN: u64 = 2 | 128 | 256 | 512 | 32_768 | 65_536 | 131_072;

/// The one of them a grant sets, which is the plain `AUTO_APPROVE`.
///
/// One bit rather than the set, because the set is not all one thing. `ADMIN` is in it
/// because an administrator holds every permission, and granting approval by granting
/// that would hand somebody the media server's address along with it. The four narrower
/// forms are about 4K and about one media type; granting the plain one covers what a
/// household means and leaves the narrower answers to whoever wanted them.
const GRANTS_APPROVAL: u64 = 128;

/// The permissions under which somebody may ask for anything here at all.
///
/// Read off the gate in `/app/dist/entity/MediaRequest.js` inside the pinned image
/// rather than off the names in the table beside it: a film is refused unless the
/// account holds `REQUEST` **or** `REQUEST_MOVIE`, a series unless it holds `REQUEST` or
/// `REQUEST_TV`, and the higher quality has three more of its own. So the plain one is
/// not the whole of asking, and taking only it would leave a household still asking
/// through any of the other five.
///
/// The watchlist is the same gate. What it synchronises is filed through the same call,
/// so what is taken here stops a request nobody typed as well as one somebody did.
const ASKS_AT_ALL: u64 = 32 | 1_024 | 2_048 | 4_096 | 262_144 | 524_288;

/// The permission this service treats as holding every other one.
///
/// `ADMIN`. Named apart from the set above because it is not one of the ways to ask —
/// it is the reason taking those away from one account would achieve nothing.
const ADMINISTERS: u64 = 2;

/// One account this service holds, in the spelling it answers with.
#[derive(Deserialize)]
pub(super) struct MemberResource {
    /// The identifier this service tells them apart by.
    #[serde(default)]
    pub(super) id: i64,
    /// What they may do here, as one number with a bit per permission.
    ///
    /// Read because a limit on what somebody may watch says nothing about what they
    /// may ask for, and the two disagreeing is the gap that reading exists to find.
    #[serde(default)]
    pub(super) permissions: u64,
}

/// What one member may do here, as the narrow permissions endpoint answers it.
///
/// Apart from [`MemberResource`] because it is a different document: that one is an
/// account and this one is a single field, and reading a field off a shape built for an
/// account would take a missing account for an account with no permissions at all.
#[derive(Deserialize)]
pub(super) struct PermissionsResource {
    /// What they may do here, as one number with a bit per permission.
    #[serde(default)]
    pub(super) permissions: u64,
}

/// Whether what this member asks for arrives without anybody seeing it.
#[must_use]
pub(super) const fn approves_own(permissions: u64) -> bool {
    permissions & APPROVES_OWN != 0
}

/// The same permissions with the approval taken off, and nothing else changed.
///
/// Only the approval bits come off. A restriction on what somebody may watch is not a
/// reason to stop them raising an issue or seeing their own list, and a write that put
/// back a whole default would take those with it.
#[must_use]
pub(super) const fn without_approval(permissions: u64) -> u64 {
    permissions & !APPROVES_OWN
}

/// The same permissions with the approval put on, and nothing else changed.
///
/// The inverse of the above, and deliberately not its mirror: taking approval off takes
/// every form of it, and putting it on puts one back. Setting the whole set would make
/// an administrator of somebody who was only meant to stop waiting for one.
#[must_use]
pub(super) const fn with_approval(permissions: u64) -> u64 {
    permissions | GRANTS_APPROVAL
}

/// The same permissions with every way of asking taken off, and what came off with them.
///
/// Both halves rather than the first, because what is taken is what will be given back:
/// a set composed later from what this side thinks asking means would hand back a
/// different account from the one it took.
///
/// **An account this service treats as holding every permission is left exactly as it
/// is**, and for two reasons rather than one. The request gate reads that bit first and
/// answers yes whatever else is set, so taking these off the owner would block nothing.
/// And the write would be refused anyway: `POST /user/{id}/settings/permissions` answers
/// `403` for the account it files first and for the account asking, which in this stack
/// are the same account and are the one lemonfiber signs in as — so a reading that tried
/// would report the same failure on every glance at the household, for ever.
#[must_use]
pub(super) const fn without_asking(permissions: u64) -> (u64, u64) {
    if permissions & ADMINISTERS != 0 {
        return (permissions, 0);
    }
    (permissions & !ASKS_AT_ALL, permissions & ASKS_AT_ALL)
}

/// The same permissions with exactly what was taken put back.
///
/// Not the mirror of the above and not a grant: it puts back the number it is given and
/// nothing else, so an account narrowed by hand in the meantime stays narrowed, and one
/// that only ever held the higher quality gets the higher quality back.
#[must_use]
pub(super) const fn with_asking(permissions: u64, taken: u64) -> u64 {
    permissions | taken
}

#[cfg(test)]
mod tests {
    use super::{
        approves_own, with_approval, with_asking, without_approval, without_asking, APPROVES_OWN,
        ASKS_AT_ALL,
    };

    /// Taking the asking away takes every way of asking, not only the plain one.
    ///
    /// The gate reads the plain permission *or* the one for that kind of thing, so an
    /// account left holding either is an account still asking — which is the whole of
    /// what a full disk has to stop.
    #[test]
    fn taking_the_asking_away_takes_every_way_of_asking() {
        // Films only, plus a vote and an issue, which are neither.
        let held = 262_144 | 64 | 4_194_304;

        let (left, taken) = without_asking(held);

        assert_eq!(taken, 262_144, "what came off was not what could ask");
        assert_eq!(
            left,
            64 | 4_194_304,
            "something that is not asking came off"
        );
        assert_eq!(without_asking(left).1, 0, "there was still a way to ask");
    }

    /// An owner is left exactly as they are, because taking a bit off one does nothing.
    #[test]
    fn an_owner_is_left_exactly_as_they_are() {
        // `ADMIN` beside the plain way of asking.
        let held = 2 | 32;

        let (left, taken) = without_asking(held);

        assert_eq!(left, held, "the owner's account was changed");
        assert_eq!(
            taken, 0,
            "something was written down as taken from the owner"
        );
    }

    /// What is given back is exactly what was taken, and nothing composed here.
    ///
    /// A member who could only ask for the higher quality gets the higher quality back,
    /// rather than the plain grant this side would otherwise have decided on.
    #[test]
    fn what_is_given_back_is_what_was_taken() {
        let held = 2_048 | 64;

        let (left, taken) = without_asking(held);

        assert_eq!(with_asking(left, taken), held);
    }

    /// Giving back puts nothing else back with it.
    ///
    /// A permission an operator narrowed while the disk was full stays narrowed: this
    /// puts back a number it was handed rather than restoring an account.
    #[test]
    fn giving_back_restores_nothing_the_operator_took_meanwhile() {
        let narrowed = 4_194_304;

        assert_eq!(with_asking(narrowed, 32), 32 | 4_194_304);
    }

    /// Every bit named as a way of asking is one, and the count is held.
    ///
    /// Counted rather than asserted one at a time, so a bit dropped from the set is a
    /// failure here rather than a household that quietly went on asking.
    #[test]
    fn every_way_of_asking_named_is_one_that_comes_off() {
        let named: Vec<u64> = (0..64)
            .map(|bit| 1_u64 << bit)
            .filter(|bit| ASKS_AT_ALL & bit != 0)
            .collect();

        assert_eq!(named.len(), 6, "{named:?}");
        for bit in named {
            assert_eq!(without_asking(bit), (0, bit), "{bit} did not come off");
        }
    }

    /// An account holding the plain approval bit approves its own requests.
    #[test]
    fn an_account_that_approves_its_own_requests_reads_as_one() {
        // `REQUEST` beside `AUTO_APPROVE`, which is the ordinary shape of an account a
        // household set up without thinking about it.
        assert!(approves_own(32 | 128));
        assert!(!approves_own(32));
    }

    /// An administrator approves their own whether or not the approval bits are set.
    ///
    /// This service treats that permission as holding every other one, so a reading
    /// that looked only at the approval bits would call the owner the most restricted
    /// account in the house.
    #[test]
    fn an_administrator_approves_their_own_without_the_approval_bits() {
        assert!(approves_own(2));
    }

    /// Taking the approval off leaves everything else exactly as it was.
    #[test]
    fn taking_the_approval_off_leaves_the_rest_alone() {
        // `REQUEST`, `VOTE` and `CREATE_ISSUES` beside the approval.
        let held = 32 | 64 | 4_194_304 | 128 | 512;

        let left = without_approval(held);

        assert_eq!(left, 32 | 64 | 4_194_304);
        assert!(!approves_own(left));
    }

    /// Putting the approval on puts one form of it on, and makes nobody an
    /// administrator on the way.
    ///
    /// Taking it off takes every form; putting it on puts one back. A grant that set
    /// the whole set would hand somebody the media server's address along with a
    /// shorter wait.
    #[test]
    fn putting_the_approval_on_makes_nobody_an_administrator() {
        // `REQUEST` and `CREATE_ISSUES`, which is an ordinary member's shape.
        let held = 32 | 4_194_304;

        let granted = with_approval(held);

        assert!(approves_own(granted));
        assert_eq!(granted & 2, 0, "the grant made them an administrator");
        assert_eq!(granted & held, held, "the grant took something away");
        assert!(!approves_own(without_approval(granted)));
    }

    /// Every bit named is one the reading actually turns on.
    ///
    /// Counted rather than asserted one at a time, so a bit dropped from the set is a
    /// failure here rather than a member quietly reported as held back.
    #[test]
    fn every_bit_named_is_one_that_reads_as_approving() {
        let named: Vec<u64> = (0..64)
            .map(|bit| 1_u64 << bit)
            .filter(|bit| APPROVES_OWN & bit != 0)
            .collect();

        assert_eq!(named.len(), 7, "{named:?}");
        for bit in named {
            assert!(approves_own(bit), "{bit} does not read as approving");
        }
    }
}

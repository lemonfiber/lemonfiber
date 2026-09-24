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

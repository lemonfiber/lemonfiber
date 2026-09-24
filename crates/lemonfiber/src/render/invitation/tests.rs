use super::invitation;
use lemonfiber_core::model::{Invitation, InvitationStanding, Linked};

fn offered(withdrawn: Vec<String>) -> Invitation {
    Invitation {
        name: "ana".to_owned(),
        address: "http://192.168.1.20:8096".to_owned(),
        caution: None,
        hours: 48,
        withdrawn,
        rehearsed: false,
        standing: InvitationStanding::Made,
        linked: Linked::Made,
        applied: None,
    }
}

/// An invitation whose address is a number, which a router can hand elsewhere.
fn numbered() -> Invitation {
    Invitation {
        caution: Some(
            "That address is a number, and routers hand out different \
                       ones — so it can stop working."
                .to_owned(),
        ),
        ..offered(Vec::new())
    }
}

/// The same invitation, said by a run that wrote nothing.
fn rehearsed(withdrawn: Vec<String>) -> Invitation {
    Invitation {
        rehearsed: true,
        ..offered(withdrawn)
    }
}

/// The same person, whose invitation was already out and still stands.
fn waiting() -> Invitation {
    Invitation {
        standing: InvitationStanding::Waiting,
        ..offered(Vec::new())
    }
}

/// The same person, who has already set a password.
fn joined() -> Invitation {
    Invitation {
        standing: InvitationStanding::Joined,
        ..offered(Vec::new())
    }
}

/// The same person, whose password has just been taken off.
fn reset() -> Invitation {
    Invitation {
        standing: InvitationStanding::Reset,
        ..offered(Vec::new())
    }
}

/// A reset says what stopped working, and what it costs to leave it.
///
/// Both halves matter and neither is obvious from the other. The first is what the
/// operator is about to be asked — why the old password does not open it. The second
/// is the consequence they have to be able to pass on: this window ends in the
/// account being **removed**, and unlike an invitation nobody took up, that account
/// is one somebody has watched on. "Lapses" is not a strong enough word for that, so
/// this view does not use it here.
#[test]
fn a_reset_says_what_stopped_working_and_what_leaving_it_costs() {
    let said = invitation(&reset()).text();

    assert!(
        said.contains("old password no longer works"),
        "a reset did not say the thing the person will ask about: {said}"
    );
    assert!(
        said.contains("48 hours") && said.contains("the account is removed"),
        "a reset did not say the window or what happens at the end of it: {said}"
    );
    assert!(
        said.contains("set a password"),
        "a reset left out how to claim the account again: {said}"
    );
    assert!(
        said.contains('\u{2588}'),
        "a reset lost the code that makes the address easy to take: {said}"
    );
}

/// An invitation still standing is that message again, not a refusal.
///
/// Offering somebody twice is a thing operators do — the first message went
/// unanswered, or they forgot — and what they want is the thing to send, which is
/// exactly what was true the first time.
#[test]
fn an_invitation_still_standing_is_handed_over_again() {
    let said = invitation(&waiting()).text();

    assert!(said.contains("still stands"), "{said}");
    assert!(said.contains("http://192.168.1.20:8096"), "{said}");
    assert!(
        said.contains('\u{2588}'),
        "the message to send again lost the code that makes it easy: {said}"
    );
}

/// Somebody already in the house is told so, and asked for nothing.
///
/// A code to scan and an instruction to set a first password are both telling
/// somebody to do again what they have already done.
#[test]
fn somebody_already_in_the_house_is_asked_to_claim_nothing() {
    let said = invitation(&joined()).text();

    assert!(said.contains("already in the house"), "{said}");
    assert!(
        !said.contains("set a password"),
        "somebody already in was told to claim an account: {said}"
    );
    assert!(
        !said.contains('\u{2588}'),
        "a code was drawn for somebody with nothing to claim: {said}"
    );
}

/// What the sweep took back is said whatever was found under the name asked for.
///
/// The withdrawals are other people's, and an operator who invited somebody last
/// week is owed them regardless of whose invitation this run was about.
#[test]
fn withdrawals_are_named_even_where_the_person_was_already_in() {
    let already_in = Invitation {
        withdrawn: vec!["bo".to_owned()],
        ..joined()
    };

    let said = invitation(&already_in).text();

    assert!(said.contains("withdrawn"), "{said}");
    assert!(said.contains("bo"), "{said}");
}

/// The answer first: who, then the one address to send.
///
/// An operator reading two lines has the message they need to pass on. Anything
/// that pushed the address further down would make them read to find it.
#[test]
fn the_name_is_the_first_line_and_the_address_the_second() {
    let said = invitation(&offered(Vec::new())).text();
    let mut lines = said.lines();

    assert!(
        lines.next().is_some_and(|first| first.contains("ana")),
        "{said}"
    );
    assert_eq!(lines.next(), Some("  http://192.168.1.20:8096"));
}

/// What they are owed before they accept, said where they are being asked.
///
/// Telling somebody afterwards is telling them once they have already put their
/// watching on a machine somebody else administers.
#[test]
fn the_operator_is_told_to_pass_on_that_watching_is_visible() {
    let said = invitation(&offered(Vec::new())).text();

    assert!(
        said.contains("can see what they watch"),
        "the household is not told the operator can see what they watch: {said}"
    );
}

/// Where the address works is said beside it, not left to be discovered.
///
/// Somebody who tries it from mobile data and is told nothing reads a working
/// invitation as a dead link.
#[test]
fn the_invitation_says_the_address_works_at_home_only() {
    let said = invitation(&offered(Vec::new())).text();
    let address = said.lines().position(|line| line.contains("192.168.1.20"));
    let where_it_works = said.lines().position(|line| line.contains("home network"));

    assert!(
        where_it_works.is_some_and(|note| address.is_some_and(|at| note == at + 1)),
        "where the address works must sit beside it: address {address:?}, note {where_it_works:?}"
    );
}

/// The address is drawn as well as written, and after it.
#[test]
fn the_address_is_drawn_for_a_camera_as_well_as_written() {
    let said = invitation(&offered(Vec::new())).text();
    let written = said.lines().position(|line| line.contains("192.168.1.20"));
    let drawn = said.lines().position(|line| line.contains('\u{2588}'));

    assert!(
        written.is_some_and(|written| drawn.is_some_and(|drawn| drawn > written)),
        "written at {written:?}, drawn at {drawn:?}: it needs to be both, in that order"
    );
}

/// An address too long to draw still gets the words.
///
/// The code is the convenience; the address and the name are the message. A
/// drawing that could not be made must not take the sentence with it.
#[test]
fn an_address_too_long_to_draw_still_carries_the_words() {
    let mut far_too_long = offered(Vec::new());
    far_too_long.address = format!("http://{}", "h".repeat(8000));

    let said = invitation(&far_too_long).text();

    assert!(said.contains("ana"), "{}", &said[..said.len().min(200)]);
    assert!(
        !said.contains('\u{2588}'),
        "an address that fits in no code was drawn as one anyway"
    );
}

/// Reached the way every surface reaches it, not only by calling the renderer.
#[test]
fn the_dispatch_draws_an_invitation() {
    let said =
        crate::render::shaped(&lemonfiber_core::app::Outcome::Invited(offered(Vec::new()))).text();

    assert!(said.contains("ana"), "{said}");
}

/// An invitation taken back is reported rather than done quietly.
///
/// Somebody who invited a person last week and heard nothing would otherwise have
/// no way to learn the account is gone.
#[test]
fn invitations_taken_back_are_named() {
    let said = invitation(&offered(vec!["bo".to_owned()])).text();

    assert!(said.contains("withdrawn"), "{said}");
    assert!(
        said.contains("bo"),
        "the one taken back was not named: {said}"
    );
}

/// What is worth knowing about the address travels with the address.
///
/// This is the copy somebody keeps. A bookmark made from a number stops working
/// when the router hands it elsewhere, and the person it stops working for is
/// not the one who could find out why.
#[test]
fn an_address_that_is_a_number_carries_its_warning() {
    let said = invitation(&numbered()).text();

    assert!(said.contains("routers hand out different"), "{said}");
    assert!(
        !invitation(&offered(Vec::new()))
            .text()
            .contains("routers hand out different"),
        "an address that is a name was warned about anyway"
    );
}

/// A rehearsal says so before anything that reads as an account that exists.
#[test]
fn a_rehearsal_says_nothing_was_made_before_it_says_anything_else() {
    let said = invitation(&rehearsed(Vec::new())).text();

    assert!(
        said.lines()
            .next()
            .is_some_and(|first| first.contains("Nothing was made")),
        "a run that wrote nothing opened as though it had: {said}"
    );
}

/// What a rehearsal would take back is said as *would*, not as done.
///
/// The same list under the same heading is the difference between telling an
/// operator an account is gone and telling them it is about to be.
#[test]
fn a_rehearsal_says_what_would_be_withdrawn_rather_than_what_was() {
    let said = invitation(&rehearsed(vec!["bo".to_owned()])).text();

    assert!(said.contains("would be withdrawn"), "{said}");
    assert!(
        !said.contains("have been withdrawn"),
        "a rehearsal reported taking an account back: {said}"
    );
}

/// A link that could not be made is said, and says there is nothing to undo.
///
/// The fear on reading that a second service was unreachable is that half a thing
/// was made and somebody has to go and tidy it up. Nothing was, so it says so.
#[test]
fn a_link_that_could_not_be_made_is_said_and_says_nothing_needs_undoing() {
    let unlinked = Invitation {
        linked: Linked::NotYet,
        ..offered(Vec::new())
    };

    let text = invitation(&unlinked).text();
    assert!(
        text.contains("can watch but cannot ask for anything yet"),
        "{text}"
    );
    assert!(text.contains("Nothing is half-made"), "{text}");
}

/// The ordinary case says nothing about it, and neither does a rehearsal.
///
/// A member the request service already knows about needs no line: the invitation
/// is what the operator is reading, and a sentence that is true of every ordinary
/// run is one they learn to skip — taking the unusual one with it.
#[test]
fn a_link_that_was_made_or_never_tried_says_nothing_about_it() {
    for report in [
        offered(Vec::new()),
        Invitation {
            linked: Linked::NotTried,
            ..offered(Vec::new())
        },
    ] {
        let text = invitation(&report).text();
        assert!(
            !text.contains("cannot ask for anything yet"),
            "an ordinary invitation explained a link nobody asked about: {text}"
        );
    }
}

/// Somebody already in the house is told it too, on the path that returns early.
///
/// Both endings of this view go through one footer, so a caveat owed on one is
/// owed on the other — this is what holds that, since the early return is the
/// place it would be forgotten.
#[test]
fn somebody_already_here_is_told_about_the_link_as_well() {
    let unlinked = Invitation {
        linked: Linked::NotYet,
        ..joined()
    };

    // Bound once rather than called again in the message: an argument only
    // evaluated on failure is a line the coverage gate never sees run.
    let text = invitation(&unlinked).text();
    assert!(text.contains("cannot ask for anything yet"), "{text}");
}

#[test]
fn nothing_is_said_about_withdrawals_where_there_were_none() {
    let said = invitation(&offered(Vec::new())).text();

    assert!(
        !said.contains("withdrawn"),
        "a run that took nothing back said it had: {said}"
    );
}

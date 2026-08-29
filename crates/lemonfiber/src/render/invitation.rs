//! What to send somebody, and the one thing they should know before they accept.
//!
//! The shape is the answer first: who it is for, then the single address to pass
//! on. An operator reading only the first two lines has everything they need to
//! send the message.
//!
//! The code beneath it is the same address again, for a camera. Somebody being
//! invited is usually holding the phone they will watch on, and typing an address
//! and then a password on a phone keyboard is exactly the friction that makes
//! people give up before they start.

use lemonfiber_core::model::{Invitation, InvitationStanding};

use super::{qr, Lines};
use crate::say;

/// What an operator is told after offering somebody an account.
pub(super) fn invitation(report: &Invitation) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        // First, because everything under it reads as an account that exists.
        lines.put("Nothing was made. This is what the invitation would say:".to_owned());
    }
    lines.put(match report.standing {
        InvitationStanding::Made => format!(
            "{} can sign in — unclaimed until they set a password, and it lapses in {} hours",
            report.name, report.hours
        ),
        InvitationStanding::Waiting => format!(
            "{} was already invited and it still stands, so here is that message again",
            report.name
        ),
        InvitationStanding::Joined => format!("{} is already in the house", report.name),
    });
    lines.put(format!("  {}", report.address));
    lines.put(HOME_ONLY.to_owned());
    if let Some(caution) = &report.caution {
        lines.put(format!("  {caution}"));
    }

    // Nothing to claim, so nothing to point a camera at. The address stands because
    // it is still where they sign in, but a code and an instruction about setting a
    // first password would be telling somebody to do again what they have done.
    if report.standing == InvitationStanding::Joined {
        return withdrawals(lines, report);
    }

    if let Some(drawn) = qr::rows(&report.address, say::folding()) {
        lines.spaced("Or point their phone's camera at this:");
        for row in drawn {
            lines.put(format!("  {row}"));
        }
    }

    lines.spaced(format!(
        "Tell them to sign in as `{}`. They will be asked to set a password.",
        report.name
    ));

    // Said here because here is where somebody is being asked to join. Telling them
    // afterwards is telling them once they have already put their watching on a
    // machine somebody else administers.
    lines.put(WATCHED.to_owned());

    withdrawals(lines, report)
}

/// What the sweep took back on the way past, where it took anything.
fn withdrawals(mut lines: Lines, report: &Invitation) -> Lines {
    if !report.withdrawn.is_empty() {
        lines.spaced(if report.rehearsed {
            "Nobody claimed these in time, so they would be withdrawn:"
        } else {
            "Nobody claimed these in time, so they have been withdrawn:"
        });
        for name in &report.withdrawn {
            lines.put(format!("  {name}"));
        }
    }
    lines
}

/// Where the address works, said beside the address rather than under it.
///
/// The stack is published to the home network and nowhere else. An address that
/// opens nothing from a phone on mobile data is not broken — it is being asked
/// from the wrong place, and somebody who was not told that reads a working
/// invitation as a dead link and gives up without saying so.
const HOME_ONLY: &str = "  It opens on the home network only, not from outside the house.";

/// What the household is owed before they accept.
///
/// The operator of this stack can see what everybody watches — the media server
/// keeps that and shows it to whoever administers it. Said plainly and without
/// softening: somebody deciding whether to accept an account is entitled to know
/// what accepting it means, and a sentence they have to go looking for is one they
/// will not find.
const WATCHED: &str = "  Tell them too: whoever runs this server can see what they watch and when.";

#[cfg(test)]
mod tests {
    use super::invitation;
    use lemonfiber_core::model::{Invitation, InvitationStanding};

    fn offered(withdrawn: Vec<String>) -> Invitation {
        Invitation {
            name: "ana".to_owned(),
            address: "http://192.168.1.20:8096".to_owned(),
            caution: None,
            hours: 48,
            withdrawn,
            rehearsed: false,
            standing: InvitationStanding::Made,
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
            crate::render::shaped(&lemonfiber_core::app::Outcome::Invited(offered(Vec::new())))
                .text();

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

    #[test]
    fn nothing_is_said_about_withdrawals_where_there_were_none() {
        let said = invitation(&offered(Vec::new())).text();

        assert!(
            !said.contains("withdrawn"),
            "a run that took nothing back said it had: {said}"
        );
    }
}

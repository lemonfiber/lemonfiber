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

use lemonfiber_core::model::Invitation;

use super::{qr, Lines};
use crate::say;

/// What an operator is told after offering somebody an account.
pub(super) fn invitation(report: &Invitation) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        // First, because everything under it reads as an account that exists.
        lines.put("Nothing was made. This is what the invitation would say:".to_owned());
    }
    lines.put(format!(
        "{} can sign in — unclaimed until they set a password, and it lapses in {} hours",
        report.name, report.hours
    ));
    lines.put(format!("  {}", report.address));
    lines.put(HOME_ONLY.to_owned());

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
    use lemonfiber_core::model::Invitation;

    fn offered(withdrawn: Vec<String>) -> Invitation {
        Invitation {
            name: "ana".to_owned(),
            address: "http://192.168.1.20:8096".to_owned(),
            hours: 48,
            withdrawn,
            rehearsed: false,
        }
    }

    /// The same invitation, said by a run that wrote nothing.
    fn rehearsed(withdrawn: Vec<String>) -> Invitation {
        Invitation {
            rehearsed: true,
            ..offered(withdrawn)
        }
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

//! What a browser may ask when somebody new moves in.
//!
//! One action, and one argument — which is why it has a file rather than a corner
//! of the one beside it: everything interesting about an invitation is what it is
//! *for*, and there is nothing else to get wrong.
//!
//! The name is required and the refusal names the argument rather than the action,
//! which is what lets a form put the message beside the field somebody typed in
//! rather than at the top of the page.

use lemonfiber_api::actions::{named, Arguments, Refused};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::model::{Invitation, InvitationStanding, Linked};

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

#[test]
fn an_invitation_carries_the_name_it_is_for() {
    let with_name = Arguments {
        name: Some("ana".to_owned()),
        ..Arguments::default()
    };

    assert!(matches!(
        command("invite", with_name),
        Some(Command::Invite { name }) if name == "ana"
    ));
}

/// An invitation for nobody is refused, and the refusal names the argument.
#[test]
fn an_invitation_for_nobody_is_refused_by_the_argument_it_lacks() {
    assert!(
        matches!(
            refusal("invite", Arguments::default()),
            Some(Refused::Missing { argument, .. }) if argument == "name"
        ),
        "an invitation for nobody was accepted"
    );
}

/// What a browser is handed once the job finishes, written from this side of it.
///
/// The job runner hands back whatever the outcome serialises to, and that happens
/// here rather than in the core: this crate compiles its own copy of the envelope,
/// so a shape only ever proved on the other side of the boundary is one nothing has
/// checked on the side a browser actually talks to.
#[test]
fn an_invitation_reaches_a_browser_under_its_own_name() {
    let made = Outcome::Invited(Invitation {
        name: "ana".to_owned(),
        address: "http://192.168.1.20:8096".to_owned(),
        caution: None,
        hours: 48,
        withdrawn: vec!["bo".to_owned()],
        rehearsed: false,
        standing: InvitationStanding::Made,
        linked: Linked::Made,
    });

    let json = made.envelope().to_json().unwrap_or_default();

    assert!(json.contains(r#""kind":"invitation""#), "{json}");
    assert!(json.contains("ana"), "{json}");
    assert!(
        json.contains("bo"),
        "an invitation taken back was dropped on the way to a browser: {json}"
    );
}

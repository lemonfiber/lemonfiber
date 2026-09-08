//! Every credential this stack holds, on a terminal, with none of their values.
//!
//! The inventory leads because it is the answer to the question that was asked, and
//! each line says the four things an operator needs: what it is, what authenticates
//! with it, where the value lives, and where it stands. The advisories come after,
//! not beside — one credential that has gone stale should not push the other twelve
//! off the top of a screen.
//!
//! What the storage protects against is printed every time, both halves of it. An
//! operator reading this list is exactly the operator about to decide whether a copy
//! of that file is safe to put somewhere, and the answer is longer than "yes".

use lemonfiber_core::credential::{Inventory, Reach, Revealed, Rotation, Settled};

use super::Lines;

/// Everything held, and what became of anything asked about one of them.
pub(crate) fn listing(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    lines.put("The credentials this stack holds. None of their values is shown.");
    for held in &inventory.held {
        lines.spaced(format!("  {} — {}", held.name, held.state.as_str()));
        lines.put(format!("    recorded as  {}", held.setting));
        lines.put(format!("    kept in      {}", held.location));
        for consumer in &held.consumers {
            lines.put(format!("    used by      {consumer}"));
        }
    }
    lines.extend(advisories(inventory));
    lines.extend(protection(inventory));
    if let Some(rotated) = &inventory.rotated {
        lines.extend(rotation(rotated));
    }
    if let Some(revealed) = &inventory.revealed {
        lines.extend(reveal(revealed));
    }
    lines
}

/// What is worth saying about the ones that are not simply working.
fn advisories(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    let said = inventory.advisories();
    if said.is_empty() {
        return lines;
    }
    lines.spaced("Worth knowing. None of this expires on its own:");
    for one in said {
        lines.put(format!("  {one}"));
    }
    lines
}

/// What keeping them in files does, and does not, protect against.
fn protection(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(inventory.protection.summary.clone());
    lines.spaced("That protects against:");
    for one in &inventory.protection.against {
        lines.put(format!("  {one}"));
    }
    lines.spaced("It does not protect against:");
    for one in &inventory.protection.not_against {
        lines.put(format!("  {one}"));
    }
    lines
}

/// What became of a rotation, and of every consumer it had to reach.
fn rotation(rotated: &Rotation) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(match &rotated.settled {
        Settled::Replaced { observed } => format!(
            "{} was replaced. {observed}. The old value is gone.",
            rotated.credential
        ),
        Settled::Refused { detail } => format!("{} was not replaced: {detail}", rotated.credential),
        Settled::Unproven { detail } => {
            format!("{} was not replaced: {detail}", rotated.credential)
        }
        Settled::Elsewhere { detail } => format!("{}: {detail}", rotated.credential),
        Settled::Unknown { known } => format!(
            "Nothing here is called `{}`. What is: {}.",
            rotated.credential,
            known.join(", ")
        ),
    });
    if rotated.consumers.is_empty() {
        return lines;
    }
    lines.spaced("Everything that authenticates with it:");
    for one in &rotated.consumers {
        lines.put(format!("  {} — {}", one.consumer, reached(&one.reach)));
    }
    let stranded = rotated.stranded();
    if !stranded.is_empty() {
        lines.spaced(format!(
            "Still holding the old value, and it no longer works: {}.",
            stranded.join(", ")
        ));
    }
    lines
}

/// How far the replacement reached one consumer, in one clause.
fn reached(reach: &Reach) -> String {
    match reach {
        Reach::Updated => "has it".to_owned(),
        Reach::Pending { detail } => format!("still to be given it: run `{detail}`"),
        Reach::Failed { detail } => format!("could not be given it: {detail}"),
    }
}

/// One credential, printed or explained.
///
/// The warning goes first either way. An operator who asked for this and changed
/// their mind reads it before the value scrolls past, rather than after.
fn reveal(revealed: &Revealed) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(revealed.warning.clone());
    if let Some(value) = &revealed.value {
        lines.spaced(format!("  {}: {value}", revealed.name));
    }
    lines
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::credential::{
        Held, Inventory, Origin, Propagation, Revealed, Rotation, Settled, State, REVEALED,
        SHOULDER,
    };

    /// One credential, with the state and advisory a test is about.
    fn held(state: State, advisory: Option<&str>) -> Held {
        Held {
            name: "qBittorrent web UI password".to_owned(),
            setting: "QBITTORRENT_PASSWORD".to_owned(),
            consumers: vec!["the tunnel's forwarded-port push".to_owned()],
            location: "/somewhere/.env".to_owned(),
            origin: Origin::Lemonfiber,
            state,
            fingerprint: None,
            advisory: advisory.map(ToOwned::to_owned),
        }
    }

    /// The inventory as it reaches a terminal.
    fn drawn(inventory: Inventory) -> String {
        crate::render::shaped(&Outcome::Credentials(inventory)).text()
    }

    /// A value built rather than written, so nothing reads it as a real credential.
    fn a_value() -> String {
        format!("{}{}", "the-", "value-itself")
    }

    #[test]
    fn each_credential_says_what_it_is_where_it_is_and_what_uses_it() {
        let text = drawn(Inventory::of(vec![held(State::Active, None)]));

        assert!(
            text.contains("qBittorrent web UI password — active"),
            "{text}"
        );
        assert!(text.contains("QBITTORRENT_PASSWORD"), "{text}");
        assert!(text.contains("/somewhere/.env"), "{text}");
        assert!(text.contains("forwarded-port push"), "{text}");
    }

    /// The honest half nobody wants to write is printed every time.
    #[test]
    fn what_the_storage_does_not_protect_against_is_printed_beside_what_it_does() {
        let text = drawn(Inventory::of(vec![held(State::Active, None)]));

        assert!(text.contains("It does not protect against:"), "{text}");
        assert!(text.contains("malware"), "{text}");
        assert!(text.contains("not encrypted"), "{text}");
    }

    #[test]
    fn an_advisory_is_printed_and_says_it_does_not_expire() {
        let text = drawn(Inventory::of(vec![held(
            State::Stale,
            Some("never proven against the service that takes it"),
        )]));

        assert!(text.contains("None of this expires on its own"), "{text}");
        assert!(text.contains("never proven"), "{text}");
    }

    #[test]
    fn nothing_worth_advising_prints_no_advisory_heading() {
        let text = drawn(Inventory::of(vec![held(State::Active, None)]));

        assert!(!text.contains("Worth knowing"), "{text}");
    }

    #[test]
    fn a_landed_rotation_names_every_consumer_and_how_far_it_reached_each() {
        let text = drawn(
            Inventory::of(vec![held(State::Active, None)]).after(Rotation::landed(
                "qBittorrent web UI password",
                "qBittorrent signed in with it",
                vec![
                    Propagation::updated("qBittorrent's own web UI"),
                    Propagation::pending("the forwarded-port push", "lemonfiber restart torrent"),
                    Propagation::failed("Sonarr's download client", "Sonarr did not answer"),
                ],
            )),
        );

        assert!(text.contains("was replaced"), "{text}");
        assert!(text.contains("qBittorrent's own web UI — has it"), "{text}");
        assert!(
            text.contains("still to be given it: run `lemonfiber restart torrent`"),
            "{text}"
        );
        assert!(
            text.contains("could not be given it: Sonarr did not answer"),
            "{text}"
        );
        assert!(
            text.contains(
                "Still holding the old value, and it no longer works: Sonarr's \
                           download client."
            ),
            "{text}"
        );
    }

    #[test]
    fn a_refused_rotation_says_so_and_lists_no_consumers() {
        let text = drawn(
            Inventory::of(vec![held(State::Active, None)]).after(Rotation::stopped(
                "qBittorrent web UI password",
                Settled::Refused {
                    detail: "qBittorrent refused the password lemonfiber holds".to_owned(),
                },
            )),
        );

        assert!(text.contains("was not replaced"), "{text}");
        assert!(
            !text.contains("Everything that authenticates with it"),
            "{text}"
        );
    }

    #[test]
    fn an_unproven_rotation_says_what_stopped_it() {
        let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
            "qBittorrent web UI password",
            Settled::Unproven {
                detail: "no randomness was available".to_owned(),
            },
        )));

        assert!(text.contains("no randomness was available"), "{text}");
    }

    #[test]
    fn a_credential_that_must_be_changed_elsewhere_says_where() {
        let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
            "Usenet provider password",
            Settled::Elsewhere {
                detail: "change it with your provider first".to_owned(),
            },
        )));

        assert!(
            text.contains("Usenet provider password: change it with your provider first"),
            "{text}"
        );
    }

    #[test]
    fn a_name_nothing_answers_to_says_what_would_have() {
        let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
            "the wifi password",
            Settled::Unknown {
                known: vec!["Indexer API key".to_owned()],
            },
        )));

        assert!(
            text.contains("Nothing here is called `the wifi password`"),
            "{text}"
        );
        assert!(text.contains("Indexer API key"), "{text}");
    }

    #[test]
    fn an_unconfirmed_reveal_prints_the_warning_and_no_value() {
        let secret = a_value();
        let text = drawn(Inventory::of(Vec::new()).showing(Revealed {
            name: "Indexer API key".to_owned(),
            value: None,
            warning: SHOULDER.to_owned(),
        }));

        assert!(text.contains("scrollback"), "{text}");
        assert!(!text.contains(&secret), "{text}");
    }

    #[test]
    fn a_confirmed_reveal_prints_the_value_under_the_warning() {
        let secret = a_value();
        let text = drawn(Inventory::of(Vec::new()).showing(Revealed {
            name: "Indexer API key".to_owned(),
            value: Some(secret.clone()),
            warning: REVEALED.to_owned(),
        }));

        assert!(
            text.contains(&format!("Indexer API key: {secret}")),
            "{text}"
        );
        assert!(text.contains("Clear your scrollback"), "{text}");
    }

    /// The property the whole shape exists for: listing what is held prints no value,
    /// whatever is recorded, because there is nowhere in the listing to put one.
    #[test]
    fn listing_what_is_held_prints_no_value_even_when_one_is_recorded() {
        let secret = a_value();
        let mut line = held(State::Active, None);
        line.fingerprint = Some(lemonfiber_core::credential::fingerprint(&secret));

        let text = drawn(Inventory::of(vec![line]));

        assert!(!text.is_empty());
        assert!(!text.contains(&secret), "{text}");
    }
}

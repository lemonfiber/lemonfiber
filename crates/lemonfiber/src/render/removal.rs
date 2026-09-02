//! What removing somebody costs, said before it is done.
//!
//! The shape is the cost first and the confirmation last, because the operator reading
//! this is deciding, not being informed. A summary that leads with "removed" and buries
//! what went is one somebody skims past.
//!
//! **Every figure here is knowable without removing anybody**, which is the whole reason
//! the unconfirmed run is worth having: what it says is not an estimate of what would
//! happen, it is the same reading the confirmed run acts on.

use lemonfiber_core::model::{HouseholdRemoval, Revoked};

use super::Lines;

/// What an operator is told when removing somebody, or asking what it would cost.
pub(super) fn removal(report: &HouseholdRemoval) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.confirmed {
        format!("{} is no longer in this household.", report.name)
    } else {
        format!("Removing {} would take all of this:", report.name)
    });

    // Watch history is not a choice this program makes. The media server's own removal
    // takes only an account identifier — there is no option to keep anything — so this
    // is stated as the fact it is rather than as something that could be arranged.
    lines.spaced(if report.confirmed {
        "  Their watch history went with the account. It cannot be got back."
    } else {
        "  Their watch history, which goes with the account. It cannot be got back."
    });

    lines.put(asked_for(report));

    if !report.asks_through_the_request_service {
        // Said rather than left silent: an operator who expected two revocations and
        // saw one has to be told which, and that nothing failed.
        lines.put(
            "  They never signed in to the request service, so there was no account \
             there to take."
                .to_owned(),
        );
    }

    if report.confirmed {
        lines.spaced(match report.revoked {
            Revoked::Everywhere => "Both accounts are gone.".to_owned(),
            Revoked::MediaServerOnly => {
                "The media server's account is gone. The request service's is not — see \
                 below."
                    .to_owned()
            }
            // Not reachable from a confirmed run, and said plainly rather than left to
            // print an empty line if it ever becomes so.
            Revoked::Nothing => "Nothing was removed.".to_owned(),
        });
    } else {
        lines.spaced(format!(
            "Nothing has been removed. Run `lemonfiber remove {} --confirm` to go ahead.",
            report.name
        ));
    }

    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// The line about what they asked for, in the number the operator is deciding about.
///
/// **Destroyed, not transferred.** The request service removes them by hand so a title
/// still waiting goes back to being unrequested; saying "removed" alone would let
/// somebody read it as the requests surviving under another name.
fn asked_for(report: &HouseholdRemoval) -> String {
    match (report.requests, report.confirmed) {
        (0, _) => "  Nothing they asked for — they had no requests outstanding.".to_owned(),
        (1, true) => "  The one thing they asked for, which no longer exists.".to_owned(),
        (1, false) => "  The one thing they asked for, which stops existing.".to_owned(),
        (many, true) => format!("  The {many} things they asked for, which no longer exist."),
        (many, false) => format!("  The {many} things they asked for, which stop existing."),
    }
}

#[cfg(test)]
mod tests {
    use super::removal;
    use lemonfiber_core::model::{HouseholdRemoval, Revoked};

    /// A removal as it stands before anything is done to it.
    fn asked(requests: usize) -> HouseholdRemoval {
        HouseholdRemoval {
            name: "ana".to_owned(),
            confirmed: false,
            requests,
            asks_through_the_request_service: true,
            revoked: Revoked::Nothing,
            findings: Vec::new(),
        }
    }

    /// The cost leads, and the confirmation is the last thing said.
    ///
    /// An operator reading this is deciding, so what they lose has to be above the line
    /// that tells them how to go ahead — not underneath it where they scroll past.
    #[test]
    fn what_goes_is_said_before_how_to_go_ahead() {
        let text = removal(&asked(2)).text();
        let cost = text.find("watch history");
        let confirm = text.find("--confirm");

        assert!(cost.is_some() && confirm.is_some(), "{text}");
        assert!(
            cost < confirm,
            "the confirmation came before the cost: {text}"
        );
        assert!(
            text.contains("Removing ana would take all of this:"),
            "{text}"
        );
    }

    /// Watch history is stated as a fact, because there is no option to keep it.
    #[test]
    fn the_watch_history_is_not_offered_as_a_choice() {
        let text = removal(&asked(0)).text();
        assert!(text.contains("It cannot be got back."), "{text}");
        assert!(
            !text.contains("kept") && !text.contains("keep"),
            "the history was described as something that could be kept: {text}"
        );
    }

    /// Requests stop existing rather than moving somewhere, and the number is exact.
    #[test]
    fn the_requests_are_counted_and_said_to_stop_existing() {
        assert!(removal(&asked(2))
            .text()
            .contains("The 2 things they asked for, which stop existing"));
        assert!(removal(&asked(1))
            .text()
            .contains("The one thing they asked for, which stops existing"));
        assert!(removal(&asked(0)).text().contains("Nothing they asked for"));
    }

    /// Somebody the request service never knew is said so, rather than left silent.
    #[test]
    fn never_having_asked_for_anything_is_said_rather_than_left_blank() {
        let never = HouseholdRemoval {
            asks_through_the_request_service: false,
            ..asked(0)
        };
        // Bound once: an argument only evaluated on failure is a line nothing runs.
        let text = removal(&never).text();
        assert!(text.contains("no account there to take"), "{text}");
        // The same report twice is the same report — held here so the equality the
        // machine-readable contract rests on is exercised rather than only derived.
        assert_eq!(never, never.clone(), "two readings of one removal differ");
    }

    /// Done, it says so in the past tense and names how far it got.
    #[test]
    fn a_confirmed_removal_says_what_it_did_and_how_far_it_reached() {
        let done = HouseholdRemoval {
            confirmed: true,
            revoked: Revoked::Everywhere,
            ..asked(1)
        };
        let text = removal(&done).text();
        assert!(
            text.contains("ana is no longer in this household."),
            "{text}"
        );
        assert!(text.contains("Both accounts are gone."), "{text}");
        assert!(text.contains("which no longer exist"), "{text}");
        assert!(
            !text.contains("--confirm"),
            "a done removal still asked: {text}"
        );
    }

    /// One that reached only the media server says which half is outstanding.
    #[test]
    fn a_partial_removal_names_the_half_that_is_left() {
        let half = HouseholdRemoval {
            confirmed: true,
            revoked: Revoked::MediaServerOnly,
            findings: vec!["the request service still holds an account".to_owned()],
            ..asked(1)
        };
        let text = removal(&half).text();
        assert!(text.contains("The request service's is not"), "{text}");
        assert!(text.contains("! the request service still holds"), "{text}");
    }
}

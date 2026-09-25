//! Requests the household makes and the allowances it is given, as commands.

use super::*;

/// What a walk was asked for is words joined back into a title, and asking for
/// nothing in particular is a request rather than an omission.
#[test]
fn a_walk_is_asked_for_by_words_or_by_nothing() {
    let said = ["the".to_owned(), "big".to_owned(), "lebowski".to_owned()];
    assert_eq!(named(&said), Some("the big lebowski".to_owned()));
    assert_eq!(named(&[]), None);
    assert_eq!(named(&["   ".to_owned()]), None);
}

/// The two things a single word can move forward go to two commands, because what
/// each answers with does not resemble the other.
#[test]
fn the_two_things_an_update_can_mean_go_to_two_commands() {
    let stack = moving(UpdateCommand::Stack {
        service: Some("sonarr".to_owned()),
        confirm: true,
        wait: true,
    });
    assert!(
        matches!(stack, Command::Update(asked)
                 if asked.service.as_deref() == Some("sonarr")
                 && asked.confirm
                 && asked.wait == Waiting::ForTheDownloads),
        "the stack's own three fields are carried"
    );
    assert_eq!(
        moving(UpdateCommand::Itself { to: None }),
        Command::SelfUpdate { to: None }
    );
}

/// Naming no count takes the one a browser asking the same question is answered
/// with — the served read's own number, read from there rather than restated.
#[test]
fn a_shelf_with_no_count_takes_the_one_both_surfaces_share() {
    assert_eq!(
        super::super::held("Ada".to_owned(), None),
        Ok(Command::Held {
            member: "Ada".to_owned(),
            most: lemonfiber_api::read::table::A_SHELF,
        })
    );
}

/// Refused rather than rounded, at both ends. Somebody who asked for a thousand and
/// was shown five hundred has been told that is the shelf.
#[test]
fn a_count_outside_what_one_shelf_shows_is_refused_at_either_end() {
    let ceiling = lemonfiber_api::read::table::MOST_AT_ONCE;
    assert!(super::super::held("Ada".to_owned(), Some(0)).is_err());
    assert!(super::super::held("Ada".to_owned(), Some(ceiling + 1)).is_err());
    assert!(super::super::held("Ada".to_owned(), Some(ceiling)).is_ok());
}

/// One choice about what the household may ask for, as the command line took it.
fn allowing(
    member: Option<&str>,
    policy: Option<&str>,
    requests: Option<u32>,
    days: Option<u32>,
) -> Result<Command, u8> {
    household(
        None,
        Some(HouseholdCommand::Allow {
            member: member.map(str::to_owned),
            policy: policy.map(str::to_owned),
            requests,
            days,
        }),
    )
}

/// Naming nothing under the word is the reading, narrowed or whole.
#[test]
fn naming_nothing_under_the_word_is_the_reading() {
    assert_eq!(
        household(None, None),
        Ok(Command::Household { member: None })
    );
    assert_eq!(
        household(Some("ana".to_owned()), None),
        Ok(Command::Household {
            member: Some("ana".to_owned())
        })
    );
}

/// The narrowing and a decision are two requests in one line, so the pair is
/// refused rather than one half being dropped.
#[test]
fn the_narrowing_and_a_decision_are_refused_together() {
    assert_eq!(
        household(
            Some("ana".to_owned()),
            Some(HouseholdCommand::Approve { request: 7 })
        ),
        Err(USAGE)
    );
}

/// A policy and a limit reach the choice as one value, for the house or one person.
#[test]
fn a_policy_and_a_limit_reach_the_choice_as_one_value() {
    assert_eq!(
        allowing(Some("ana"), Some("within-a-limit"), Some(5), Some(7)),
        Ok(Command::Allowing(Chosen {
            member: Some("ana".to_owned()),
            policy: Some(Policy::WithinALimit),
            quota: Some(Quota {
                requests: 5,
                days: 7
            }),
        }))
    );
}

/// Saying nothing about something carries nothing.
///
/// A run that named only a limit is not a run that chose to trust everybody, and a
/// value written here for a word nobody typed would be this surface deciding on the
/// household's behalf.
#[test]
fn saying_nothing_about_something_carries_nothing() {
    assert_eq!(
        allowing(None, None, Some(3), Some(30)),
        Ok(Command::Allowing(Chosen {
            member: None,
            policy: None,
            quota: Some(Quota {
                requests: 3,
                days: 30
            }),
        }))
    );
    assert_eq!(
        allowing(None, Some("trusted"), None, None),
        Ok(Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        }))
    );
}

/// A policy this build does not know is a usage error naming the ones there are.
#[test]
fn a_policy_this_build_does_not_know_is_a_usage_error() {
    assert_eq!(allowing(None, Some("generous"), None, None), Err(USAGE));
}

/// Approving and declining are the same command with different answers, and only
/// one of them carries a reason.
#[test]
fn approving_and_declining_are_one_command_with_two_answers() {
    assert_eq!(
        household(None, Some(HouseholdCommand::Approve { request: 7 })),
        Ok(Command::Deciding(Decision {
            request: 7,
            answer: Answer::LetThrough,
        }))
    );
    assert_eq!(
        household(
            None,
            Some(HouseholdCommand::Decline {
                request: 8,
                reason: "no room".to_owned(),
            })
        ),
        Ok(Command::Deciding(Decision {
            request: 8,
            answer: Answer::TurnedDown {
                reason: "no room".to_owned()
            },
        }))
    );
}

/// Arranging an expiry, withdrawing it, and running on it are three requests.
///
/// **Naming nothing is the one that acts**, and it is a request rather than an
/// omission: an operator who has already said how long is too long is not saying it
/// again in order to act on it, and there is no period this surface would supply for
/// a run that named none.
#[test]
fn arranging_an_expiry_and_running_on_it_are_different_requests() {
    assert_eq!(
        household(
            None,
            Some(HouseholdCommand::Expiring {
                after: Some(30),
                never: false,
            })
        ),
        Ok(Command::Expiring(Arranged::After(30)))
    );
    assert_eq!(
        household(
            None,
            Some(HouseholdCommand::Expiring {
                after: None,
                never: true,
            })
        ),
        Ok(Command::Expiring(Arranged::Never))
    );
    assert_eq!(
        household(
            None,
            Some(HouseholdCommand::Expiring {
                after: None,
                never: false,
            })
        ),
        Ok(Command::Expiring(Arranged::AsAgreed))
    );
}

/// Three flags at the command line are one choice in the core.
#[test]
fn an_invitation_carries_what_was_chosen_as_one_allowance() {
    assert_eq!(
        invitation(
            "ada".to_owned(),
            RawAllowance {
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                unrated: Some(RawUnrated::Block),
            }
        ),
        Command::Invite {
            name: "ada".to_owned(),
            allowance: Allowance {
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                unrated: Some(Unrated::HeldBack),
            },
        }
    );
}

/// Every flag reaches the core as it was written, and none of it is read here.
///
/// A shell that read `50%` would be a second answer to what a share means, and
/// the two would part company on the first change to either — which a
/// household would meet as an evening that went wrong on one surface and not
/// on another.
#[test]
fn what_was_asked_about_the_line_reaches_the_core_unread() {
    assert_eq!(
        sharing(RawBandwidth {
            down: Some("50%".to_owned()),
            up: Some("2MiB".to_owned()),
            active: Some("07:00-23:00".to_owned()),
            line: Some("60MiB/6MiB".to_owned()),
            cap: Some("1TiB".to_owned()),
            when_exceeded: Some("pause".to_owned()),
            unrestricted_for: Some(60),
        }),
        Command::Bandwidth(BandwidthAsked {
            down: Some("50%".to_owned()),
            up: Some("2MiB".to_owned()),
            active: Some("07:00-23:00".to_owned()),
            line: Some("60MiB/6MiB".to_owned()),
            cap: Some("1TiB".to_owned()),
            exceeded: Some("pause".to_owned()),
            unrestricted_for: Some(60),
        })
    );
}

/// Asked nothing, it asks the core for nothing — which is the reading.
#[test]
fn a_line_asked_about_and_not_changed_carries_no_answers() {
    assert_eq!(
        sharing(RawBandwidth {
            down: None,
            up: None,
            active: None,
            line: None,
            cap: None,
            when_exceeded: None,
            unrestricted_for: None,
        }),
        Command::Bandwidth(BandwidthAsked::default())
    );
}

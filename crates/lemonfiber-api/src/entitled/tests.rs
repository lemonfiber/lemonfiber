use lemonfiber_core::app::{Command, Diagnosing, Whom};

use super::{callable_by_a_key, may, Door, Permitted};
use crate::admission::Caller;

/// The member asking, by the id the media server files them under.
const ASKING: &str = "a7f3";

/// Somebody else in the same household, who the one asking is not.
const SOMEBODY_ELSE: &str = "b2e9";

fn member() -> Caller {
    Caller::Member(ASKING.to_owned())
}

fn named(id: &str) -> Whom {
    Whom::Named(id.to_owned())
}

/// Asking for the household's defaults is discarded like a name is: a member is
/// somebody, and is answered with their own row and their own shelf.
#[test]
fn a_member_asking_for_the_defaults_is_given_their_own() {
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Household {
                member: Some(Whom::Defaults)
            }
        ),
        Permitted::This(Command::Household {
            member: Some(named(ASKING))
        })
    );
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Held {
                member: Whom::Defaults,
                most: 25,
            }
        ),
        Permitted::This(Command::Held {
            member: named(ASKING),
            most: 25,
        })
    );
}

/// The operator is answered as the household's defaults where they asked to be,
/// which is how the member's side is seen without reading any member's.
#[test]
fn the_operator_is_answered_as_the_defaults() {
    let defaults = Command::Held {
        member: Whom::Defaults,
        most: 25,
    };
    assert_eq!(
        may(&Caller::Operator, Door::Reading, defaults.clone()),
        Permitted::This(defaults)
    );
}

#[test]
fn a_members_household_read_is_narrowed_to_them() {
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Household { member: None }
        ),
        Permitted::This(Command::Household {
            member: Some(named(ASKING))
        })
    );
}

/// The one that matters. A request naming another member is not compared and
/// refused; it is answered with the row belonging to whoever is asking — so
/// there is no command in existence that asks the core for somebody else's row
/// on a member's behalf, which is a stronger thing than refusing to send one.
#[test]
fn a_member_naming_somebody_else_is_still_narrowed_to_themselves() {
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Household {
                member: Some(named(SOMEBODY_ELSE))
            }
        ),
        Permitted::This(Command::Household {
            member: Some(named(ASKING))
        })
    );
}

/// The shelf is narrowed the same way, and the point is the same one: a member's
/// shelf is what their account may watch, so a request naming somebody else is
/// answered with their own rather than compared and turned down.
#[test]
fn a_member_naming_somebody_elses_shelf_is_given_their_own() {
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Held {
                member: named(SOMEBODY_ELSE),
                most: 25,
            }
        ),
        Permitted::This(Command::Held {
            member: named(ASKING),
            most: 25,
        })
    );
}

/// How much of the shelf to answer with is the caller's and survives the
/// narrowing. Whose shelf it is never was theirs to choose, and does not.
#[test]
fn how_much_of_the_shelf_a_member_asked_for_is_carried_through() {
    assert_eq!(
        may(
            &member(),
            Door::Reading,
            Command::Held {
                member: named(ASKING),
                most: 7,
            }
        ),
        Permitted::This(Command::Held {
            member: named(ASKING),
            most: 7,
        }),
        "the count a member asked for was not carried through"
    );
}

#[test]
fn a_member_is_refused_a_read_that_is_not_theirs() {
    assert_eq!(
        may(&member(), Door::Reading, Command::Version),
        Permitted::Nothing
    );
}

#[test]
fn a_member_is_refused_something_that_changes_the_machine() {
    assert_eq!(
        may(&member(), Door::Reading, Command::AtBoot),
        Permitted::Nothing
    );
}

/// Both of the people this product already answered everything for keep the
/// whole surface, and keep the member they named rather than having one chosen
/// for them — an operator reading one person's requests is the point of the
/// parameter.
#[test]
fn the_machine_and_the_operator_are_asked_nothing_new() {
    for caller in [Caller::Machine, Caller::Operator] {
        assert_eq!(
            may(&caller, Door::Reading, Command::Version),
            Permitted::This(Command::Version)
        );
        assert_eq!(
            may(
                &caller,
                Door::Reading,
                Command::Household {
                    member: Some(named(SOMEBODY_ELSE))
                }
            ),
            Permitted::This(Command::Household {
                member: Some(named(SOMEBODY_ELSE))
            })
        );
    }
}

/// A key with this scope.
fn key(scope: lemonfiber_core::keys::Scope) -> Caller {
    Caller::Key(crate::admission::Keyed {
        name: "home-assistant".to_owned(),
        scope,
    })
}

/// A restart, which a key may call.
fn a_restart() -> Command {
    Command::Restart {
        forms: vec!["tv".to_owned()],
        services: Vec::new(),
    }
}

/// An uninstall, which no key may ever call.
fn an_uninstall() -> Command {
    Command::Uninstall(lemonfiber_core::app::Removing::surveying(
        lemonfiber_core::uninstall::Tier::Media,
    ))
}

#[test]
fn a_read_key_reaches_every_read_and_no_action() {
    use lemonfiber_core::keys::Scope;
    let reading = key(Scope::Read);
    assert_eq!(
        may(&reading, Door::Reading, Command::Version),
        Permitted::This(Command::Version)
    );
    for command in [a_restart(), an_uninstall(), Command::Seed] {
        assert_eq!(
            may(&reading, Door::Acting, command),
            Permitted::NotForAKey("read".to_owned())
        );
    }
}

#[test]
fn an_act_key_calls_exactly_what_a_key_may_call() {
    use lemonfiber_core::keys::Scope;
    let acting = key(Scope::Act);
    assert_eq!(
        may(&acting, Door::Acting, a_restart()),
        Permitted::This(a_restart())
    );
    let downloads = Command::Downloads(lemonfiber_core::bandwidth::Pausing::Pause);
    assert_eq!(
        may(&acting, Door::Acting, downloads.clone()),
        Permitted::This(downloads)
    );
    for refused in [
        an_uninstall(),
        Command::Seed,
        Command::Reset { confirm: true },
        Command::Forget { confirm: true },
        Command::Doctor(Diagnosing {
            narrowing: lemonfiber_core::doctor::Narrowing::Suite,
            disruptive: true,
            accept: Some("vpn.leak".to_owned()),
        }),
    ] {
        assert_eq!(
            may(&acting, Door::Acting, refused),
            Permitted::NotForAKey("act".to_owned())
        );
    }
}

#[test]
fn a_member_key_is_exactly_that_members_session_at_either_door() {
    use lemonfiber_core::keys::Scope;
    let theirs = key(Scope::Member {
        id: ASKING.to_owned(),
        name: "ana".to_owned(),
    });
    for door in [Door::Reading, Door::Acting] {
        assert_eq!(
            may(
                &theirs,
                door,
                Command::Household {
                    member: Some(named(SOMEBODY_ELSE))
                }
            ),
            may(
                &member(),
                door,
                Command::Household {
                    member: Some(named(SOMEBODY_ELSE))
                }
            )
        );
        assert_eq!(may(&theirs, door, Command::Version), Permitted::Nothing);
        assert_eq!(may(&theirs, door, a_restart()), Permitted::Nothing);
    }
}

#[test]
fn nothing_that_cannot_be_undone_or_widens_trust_is_callable_by_a_key() {
    let never = [
        an_uninstall(),
        Command::Reset { confirm: true },
        Command::Seed,
        Command::Credentials(lemonfiber_core::app::Asking::Read),
        Command::Plugins(lemonfiber_core::app::plugins::Asked::Installed),
    ];
    for command in never {
        assert!(!callable_by_a_key(&command), "{command:?}");
    }
}

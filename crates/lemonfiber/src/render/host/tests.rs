use super::said;
use lemonfiber_core::ports::docker::{Origin, Target};

/// A run against this machine has nothing to say, and nothing is what it says.
///
/// Driven through the door rather than around it, because the claim is about the
/// door: a local run reaches it and leaves without printing. What it settles is
/// deliberately the ordinary value — this is a latch for the whole process, and
/// the tests run one to a process, but a test that put a host in it here would
/// still be the wrong test to write.
#[test]
fn a_run_against_this_machine_has_nothing_to_say() {
    assert_eq!(Target::local().host(), None, "there is nothing to mistake");
    super::settle(&Target::local());
}

/// A run aimed elsewhere says so, once.
///
/// Twice on purpose. The sentence belongs to the run rather than to whoever built
/// a context, so a second context in one process is the same run — and hearing it
/// again reads as two commands about two machines. Run rather than read back:
/// what a door puts where is the architecture test's to guard, and reading this
/// process's own stream would be a harness rather than a test.
#[test]
fn settling_the_same_target_twice_in_one_process_is_one_run() {
    let there = Target::at("ssh://media@nas.local", Origin::Variable);

    super::settle(&there);
    super::settle(&there);
}

/// Both halves of what is said, and both ways a run came to be aimed.
///
/// The host is the half that matters, and it is first for that reason. How it was
/// chosen is the half that tells an operator where to go and undo it, and the two
/// answers point at different places.
#[test]
fn what_is_said_names_the_machine_and_how_the_run_came_to_be_aimed_there() {
    let by_variable = said("ssh://media@nas.local", None);
    assert_eq!(
        by_variable.first().map(String::as_str),
        Some("This run is aimed at ssh://media@nas.local, not at this machine.")
    );
    assert_eq!(
        by_variable.get(1).map(String::as_str),
        Some("DOCKER_HOST names it.")
    );
    assert_eq!(
        by_variable.last().map(String::as_str),
        Some(""),
        "a gap under it, so the command's own output is not run into"
    );

    let by_context = said("tcp://nas.local:2375", Some("nas"));
    assert_eq!(
        by_context.get(1).map(String::as_str),
        Some("A Docker context named nas is in force.")
    );
}

/// What is said is what may be said: the endpoint with anything a credential
/// rides in taken out of it.
///
/// Asserted on the lines themselves rather than on the endpoint alone, because
/// the lines are what reaches a terminal — and a withholding that happened one
/// step earlier is only useful if nothing after it puts the value back.
#[test]
fn nothing_said_at_the_top_carries_a_credential() {
    let carried = Target::at("ssh://media:hunter2@nas.local", Origin::Variable);
    let shown = carried.host().unwrap_or_default();
    let lines = said(&shown, carried.context());

    assert!(
        !lines.iter().any(|line| line.contains("hunter2")),
        "a password reached the line printed under every command"
    );
    assert!(
        lines.iter().any(|line| line.contains("nas.local")),
        "the host went with the password, leaving nothing to read"
    );
}

//! Saying which machine a run is about, when it is not this one.
//!
//! A remote Docker context is set in a shell and then forgotten, and the command
//! that follows reads exactly like a local one: same words, same services, same
//! stack. Stopping somebody else's media server because the shell still had a
//! context in it is an easy mistake and an unpleasant one, so a run that is aimed
//! somewhere else says so before it does anything.
//!
//! Said once, at the top, rather than on every line. A banner is what an operator
//! reads and a prefix on forty lines is what they stop reading — and the run that
//! has to be caught is the one where the operator did not know there was anything
//! to look for.
//!
//! Nothing is said for a run against this machine. There is nothing to mistake, and
//! a line under every command naming the machine it is already on is the kind of
//! noise that teaches people to skip the first line.

use std::sync::OnceLock;

use lemonfiber_core::ports::docker::Target;

use crate::say::say;

/// Whether this run has already said where it is aimed.
///
/// A latch because the sentence belongs to the run rather than to whoever built a
/// context: two contexts in one process is one run, and hearing it twice reads as
/// two commands about two machines.
static ANNOUNCED: OnceLock<()> = OnceLock::new();

/// Settle which machine this run's answers are about, and say so where there is
/// somebody to tell.
///
/// The payload half is settled whatever the audience, because a machine-readable
/// answer carries the host in its envelope and a consumer reads it there. The
/// sentence is for a person only: a run whose whole output is one document must not
/// have prose folded into it.
pub(crate) fn settle(target: &Target) {
    let Some(host) = lemonfiber_core::model::settle_host(target.host()) else {
        return;
    };
    if crate::say::for_a_parser() || ANNOUNCED.set(()).is_err() {
        return;
    }
    for line in said(&host, target.context()) {
        say!("{line}");
    }
}

/// What a run aimed at another machine says about it, before it does anything.
///
/// Built rather than printed, which is this module's whole convention: a renderer
/// hands its lines back and one printer at the edge puts them on a terminal, so what
/// an operator reads is a value a test can assert on rather than a side effect it
/// can only watch happen.
///
/// Two sentences and a gap. The first is the one that matters and says the machine;
/// the second says how the run came to be aimed there, because a variable is
/// something an operator can see in their shell and a context recorded months ago is
/// not, and the two are fixed in different places.
fn said(host: &str, context: Option<&str>) -> Vec<String> {
    let how = match context {
        Some(name) => format!("A Docker context named {name} is in force."),
        None => "DOCKER_HOST names it.".to_owned(),
    };
    vec![
        format!("This run is aimed at {host}, not at this machine."),
        how,
        String::new(),
    ]
}

#[cfg(test)]
mod tests {
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
}

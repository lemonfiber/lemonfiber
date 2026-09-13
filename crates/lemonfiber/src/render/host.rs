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
    say!("This run is aimed at {host}, not at this machine.");
    match target.context() {
        Some(name) => say!("A Docker context named {name} is in force."),
        None => say!("DOCKER_HOST names it."),
    }
    say!();
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::ports::docker::{Origin, Target};

    /// A local run says nothing and settles nothing, which is what lets every other
    /// test in this binary run without one of them deciding for the rest.
    ///
    /// The value it settles is deliberately the default one. This is a latch for the
    /// whole process, and a test that put a host in it would put that host on every
    /// envelope every other test here renders.
    #[test]
    fn a_run_against_this_machine_has_nothing_to_say() {
        super::settle(&Target::local());
        assert_eq!(lemonfiber_core::model::settle_host(None), None);
    }

    /// What is shown is what may be shown: the endpoint with anything a credential
    /// rides in taken out of it.
    #[test]
    fn what_would_be_said_carries_no_credential() {
        let carried = Target::at("ssh://media:hunter2@nas.local", Origin::Variable);
        let shown = carried.host().unwrap_or_default();
        assert!(
            !shown.contains("hunter2"),
            "the password survived into what is said at the top"
        );
        assert!(
            shown.contains("nas.local"),
            "the host went with the password, leaving nothing to read"
        );
    }
}

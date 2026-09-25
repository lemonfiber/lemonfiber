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
mod tests;

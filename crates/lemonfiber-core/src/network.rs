//! Asking the operating system about this machine as a place on a network.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::network::Site;
use crate::ports::Runner;

/// The program that answers what this machine calls itself.
const HOSTNAME: &str = "hostname";

/// Open network files, narrowed to listening connections, with nothing looked up.
///
/// Names are not resolved and ports are not translated into service names, so what
/// comes back carries the numbers rather than `http` and a hostname somebody's
/// resolver invented.
const LSOF: &[&str] = &["lsof", "-nP", "-iTCP", "-sTCP:LISTEN"];

/// Sockets, narrowed the same way, without the header row.
///
/// Asked second because the first is on both platforms lemonfiber supports and this
/// is on one, and a machine that has stripped one of them has usually kept the other.
const SS: &[&str] = &["ss", "-H", "-l", "-t", "-n"];

/// This machine, asked about itself.
///
/// Through the process port every other program here goes through rather than
/// through a seam of its own: a run that has already scripted what programs say has
/// scripted this one, and there stays one place in this workspace that spawns
/// anything.
pub struct Here {
    /// How these programs are run.
    runner: Arc<dyn Runner>,
}

impl Here {
    /// This machine, asked through the given runner.
    #[must_use]
    pub const fn over(runner: Arc<dyn Runner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Site for Here {
    async fn name(&self) -> Option<String> {
        hostname(self).await
    }

    async fn answering_on(&self, ports: &[u16]) -> Vec<u16> {
        listening(self, ports).await
    }
}

async fn hostname(here: &Here) -> Option<String> {
    let output = here.runner.run(&[HOSTNAME.to_owned()]).await.ok()?;
    if !output.succeeded() {
        return None;
    }
    let said = output.stdout.trim().trim_end_matches('.');
    if said.is_empty() {
        return None;
    }
    Some(said.to_owned())
}

/// Which of the wanted ports something on this machine is already holding.
///
/// Two programs, tried in turn, and the first that is actually installed is the one
/// that answers — including when its answer is that nothing holds these ports.
///
/// A machine with neither comes out empty, which is what a machine holding none of
/// them comes out as too. The caller does the same thing either way, and the port
/// this implements says so rather than raising a failure it would have to decide
/// about twice.
async fn listening(here: &Here, wanted: &[u16]) -> Vec<u16> {
    // Nothing wanted means nothing to find out, and no reason to spawn anything to
    // find it out with. A start that publishes no port takes this way out.
    if wanted.is_empty() {
        return Vec::new();
    }
    for program in [LSOF, SS] {
        if let Some(said) = asked(here, program).await {
            return bound(&said, wanted);
        }
    }
    Vec::new()
}

/// What one of these programs wrote, or nothing where it could not be run at all.
///
/// The distinction drawn is between a program that answered and a program that is
/// not on this machine, and exit status is not it: one of these reports "nothing
/// matched" by exiting badly, so judging on the status would send every truthful
/// empty answer on to the next program — which, on a machine that has only the
/// first, is a second program spawned to fail on every start.
async fn asked(here: &Here, program: &[&str]) -> Option<String> {
    let argv: Vec<String> = program.iter().map(|word| (*word).to_owned()).collect();
    here.runner
        .run(&argv)
        .await
        .ok()
        .map(|output| output.stdout)
}

/// Every wanted port named as a listening address anywhere in what a program said.
///
/// A listening address is the one thing in either program's output shaped like a
/// colon and a number at the end of a word — `*:8989`, `127.0.0.1:8989`, `[::]:8989`
/// — so the port is read off the end of whatever carries it rather than off a column
/// whose position differs between the two programs and between their versions.
///
/// Reading loosely is safe because the reading is then narrowed to what was asked
/// about: a word misread as a port can only ever produce a number nobody wanted, and
/// a number nobody wanted is dropped here. The address a listener is bound to is not
/// looked at for the same reason the engine's own published addresses are not — a
/// port held on one address of this machine is a port a second binding of it may
/// still fail on, and saying so is the whole point of asking.
fn bound(said: &str, wanted: &[u16]) -> Vec<u16> {
    let mut found: Vec<u16> = said
        .split_whitespace()
        .filter_map(|word| word.rsplit_once(':'))
        .filter_map(|(_, port)| port.parse::<u16>().ok())
        .filter(|port| wanted.contains(port))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

#[cfg(test)]
mod tests;

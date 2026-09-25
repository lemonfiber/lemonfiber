//! Asking the operating system what state this machine is in.
//!
//! Two questions with one answer each, asked the way this workspace asks a machine
//! its name: through the process port every other program here goes through, so a
//! run that has already scripted what programs say has scripted these too, and
//! there stays one place in this workspace that spawns anything.
//!
//! **Every platform is asked, and the first that answers wins.** Neither question
//! has one command that works everywhere, and deciding which to run from the build
//! target would leave five of the six answers unreachable from any one laptop —
//! which is the arrangement [`crate::platform`] exists to refuse. Asking a program
//! that is not there costs a failed spawn, which is exactly what "this platform does
//! not answer that way" looks like from here.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::machine::{Power, Started, Supply};
use crate::ports::Runner;

/// The program the BSD way of asking runs.
///
/// Named apart from the argument vector it heads so a test can assert which program
/// was asked without reaching into the vector by position, which this workspace
/// denies everywhere rather than only where a panic would matter.
const SYSCTL: &str = "sysctl";

/// The BSD way of asking when a machine started, which macOS answers.
const BOOT_SYSCTL: [&str; 3] = [SYSCTL, "-n", "kern.boottime"];

/// The Linux way: the kernel's own counters name the moment under `btime`.
const BOOT_PROC: [&str; 2] = ["cat", "/proc/stat"];

/// The program that reports where the power is coming from on macOS.
const PMSET: &str = "pmset";

/// The macOS way of asking where the power is coming from.
const POWER_PMSET: [&str; 3] = [PMSET, "-g", "batt"];

/// The Linux way, under the two names a mains adapter is conventionally given.
///
/// Two rather than a wildcard, because a wildcard needs a shell and this workspace
/// spawns programs rather than shells — a difference worth keeping for the one
/// reason it always is, which is what a form name with a space in it does to each.
const POWER_ONLINE: [[&str; 2]; 2] = [
    ["cat", "/sys/class/power_supply/AC/online"],
    ["cat", "/sys/class/power_supply/ACAD/online"],
];

/// This machine, asked about the state it is in.
pub struct Asking {
    /// How the programs that answer are run.
    runner: Arc<dyn Runner>,
}

impl Asking {
    /// This machine, asked through the given runner.
    #[must_use]
    pub const fn over(runner: Arc<dyn Runner>) -> Self {
        Self { runner }
    }

    /// What a program said, or nothing where it would not run or exited badly.
    async fn said(&self, argv: &[&str]) -> Option<String> {
        let argv: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        let output = self.runner.run(&argv).await.ok()?;
        output.succeeded().then(|| output.stdout.clone())
    }
}

#[async_trait]
impl Started for Asking {
    async fn at(&self) -> Option<u64> {
        started(self).await
    }
}

#[async_trait]
impl Supply for Asking {
    async fn source(&self) -> Option<Power> {
        supply(self).await
    }
}

/// When this machine started, from whichever platform answers.
async fn started(asking: &Asking) -> Option<u64> {
    if let Some(said) = asking.said(&BOOT_SYSCTL).await {
        if let Some(moment) = sysctl_moment(&said) {
            return Some(moment);
        }
    }
    proc_moment(&asking.said(&BOOT_PROC).await?)
}

/// Where the power comes from, from whichever platform answers.
async fn supply(asking: &Asking) -> Option<Power> {
    if let Some(said) = asking.said(&POWER_PMSET).await {
        if let Some(power) = drawing(&said) {
            return Some(power);
        }
    }
    for argv in POWER_ONLINE {
        if let Some(power) = asking.said(&argv).await.as_deref().and_then(online) {
            return Some(power);
        }
    }
    None
}

/// The epoch second inside `{ sec = 1694612345, usec = 0 } Wed Sep 13 ...`.
///
/// Split on the field name rather than parsed as a structure, because the text
/// around it differs between releases and the number does not.
///
/// The name is matched as a name, which is the whole difficulty: `usec =` ends in
/// `sec =`, and a reader that took any occurrence would read the microseconds of a
/// line whose seconds it could not find and report a machine as having started in
/// 1970 — which is every run deciding this machine has just restarted. So a match
/// is taken only where nothing lettered runs into it, and a line that names the
/// microseconds alone is no answer rather than the wrong one.
fn sysctl_moment(said: &str) -> Option<u64> {
    let after = said.match_indices("sec =").find_map(|(at, found)| {
        let leading = said.get(..at)?.chars().next_back();
        let its_own_word = leading.is_none_or(|char| !char.is_ascii_alphanumeric());
        its_own_word.then(|| said.get(at + found.len()..))?
    })?;
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The epoch second `btime` names among the kernel's counters.
fn proc_moment(said: &str) -> Option<u64> {
    said.lines()
        .find_map(|line| line.strip_prefix("btime "))
        .and_then(|value| value.trim().parse().ok())
}

/// What a power report says it is drawing from.
///
/// Matched on the phrase rather than on the line, because what surrounds it moves
/// between releases: a charge percentage, a time remaining, a warning about a
/// battery that needs replacing. The two phrases themselves have not moved.
fn drawing(said: &str) -> Option<Power> {
    let lowered = said.to_lowercase();
    if lowered.contains("ac power") {
        return Some(Power::Mains);
    }
    lowered.contains("battery power").then_some(Power::Battery)
}

/// Whether a mains adapter reports itself connected.
fn online(said: &str) -> Option<Power> {
    match said.trim() {
        "1" => Some(Power::Mains),
        "0" => Some(Power::Battery),
        // A file that is there and says something else is a file this does not
        // understand, which is not the same as a machine with no mains adapter —
        // and both are answered by asking the next thing rather than by guessing.
        _ => None,
    }
}

#[cfg(test)]
mod tests;

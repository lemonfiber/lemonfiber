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

/// The BSD way of asking when a machine started, which macOS answers.
const BOOT_SYSCTL: [&str; 3] = ["sysctl", "-n", "kern.boottime"];

/// The Linux way: the kernel's own counters name the moment under `btime`.
const BOOT_PROC: [&str; 2] = ["cat", "/proc/stat"];

/// The macOS way of asking where the power is coming from.
const POWER_PMSET: [&str; 3] = ["pmset", "-g", "batt"];

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
/// around it differs between releases and the number does not. The split lands on
/// the first `sec =` rather than on `usec =`'s, which is the one that means seconds.
fn sysctl_moment(said: &str) -> Option<u64> {
    let after = said.split("sec =").nth(1)?;
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
mod tests {
    use std::sync::Arc;

    use super::{drawing, online, proc_moment, sysctl_moment, Asking, BOOT_SYSCTL, POWER_PMSET};
    use crate::ports::machine::{Power, Started, Supply};
    use crate::ports::process::{Failure, Output};
    use crate::ports::Runner;
    use lemonfiber_fixtures::support::{Recording, Scripted};

    /// What macOS answers when asked when it started.
    const BOOTED: &str = "{ sec = 1694612345, usec = 123456 } Wed Sep 13 18:19:05 2023\n";

    /// What Linux answers, among a good deal else.
    const COUNTERS: &str = "cpu  1 2 3\nintr 9\nbtime 1694612345\nprocesses 42\n";

    /// A runner answering every program with the given output.
    fn saying(stdout: &str) -> Arc<dyn Runner> {
        Arc::new(Scripted(Ok(Output {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })))
    }

    /// A runner no program on it will run at all.
    fn silent() -> Arc<dyn Runner> {
        Arc::new(Scripted(Err(Failure::NotFound {
            program: "sysctl".to_owned(),
        })))
    }

    #[tokio::test]
    async fn the_moment_a_machine_started_is_read_from_whichever_platform_answers() {
        assert_eq!(Asking::over(saying(BOOTED)).at().await, Some(1_694_612_345));
        assert_eq!(
            Asking::over(saying(COUNTERS)).at().await,
            Some(1_694_612_345),
            "the other platform's spelling of the same moment"
        );
    }

    #[tokio::test]
    async fn a_machine_that_will_not_say_when_it_started_says_nothing() {
        assert_eq!(Asking::over(silent()).at().await, None);
        assert_eq!(Asking::over(saying("")).at().await, None);
        assert_eq!(Asking::over(saying("no idea")).at().await, None);
    }

    #[tokio::test]
    async fn a_program_that_exits_badly_has_not_answered() {
        let failing: Arc<dyn Runner> = Arc::new(Scripted(Ok(Output {
            status: Some(1),
            stdout: BOOTED.to_owned(),
            stderr: String::new(),
        })));
        assert_eq!(Asking::over(failing).at().await, None);
    }

    #[tokio::test]
    async fn the_first_thing_asked_is_the_one_that_names_the_moment() {
        let asked = Arc::new(Recording::answering(Ok(Output {
            status: Some(0),
            stdout: BOOTED.to_owned(),
            stderr: String::new(),
        })));
        let at = Asking::over(Arc::clone(&asked) as Arc<dyn Runner>)
            .at()
            .await;
        assert_eq!(at, Some(1_694_612_345));
        assert!(asked.ran(BOOT_SYSCTL[0]));
    }

    #[tokio::test]
    async fn a_laptop_says_which_of_the_two_it_is_drawing_from() {
        let mains = "Now drawing from 'AC Power'\n -InternalBattery-0 100%; charged\n";
        let battery = "Now drawing from 'Battery Power'\n -InternalBattery-0 84%; discharging\n";
        assert_eq!(
            Asking::over(saying(mains)).source().await,
            Some(Power::Mains)
        );
        assert_eq!(
            Asking::over(saying(battery)).source().await,
            Some(Power::Battery)
        );
    }

    #[tokio::test]
    async fn the_other_platform_answers_with_a_number_instead() {
        assert_eq!(
            Asking::over(saying("1\n")).source().await,
            Some(Power::Mains)
        );
        assert_eq!(
            Asking::over(saying("0\n")).source().await,
            Some(Power::Battery)
        );
    }

    #[tokio::test]
    async fn a_machine_with_no_battery_at_all_says_nothing_rather_than_mains() {
        // A desktop has no power report and no mains adapter file. Answering "mains"
        // would be a guess that happens to be right; answering nothing is the fact,
        // and what to do about it is the caller's to decide once rather than this
        // seam's to decide for every caller.
        assert_eq!(Asking::over(silent()).source().await, None);
        assert_eq!(Asking::over(saying("something else")).source().await, None);
    }

    #[tokio::test]
    async fn the_power_report_is_asked_of_the_program_that_gives_one() {
        let asked = Arc::new(Recording::answering(Ok(Output {
            status: Some(0),
            stdout: "Now drawing from 'AC Power'".to_owned(),
            stderr: String::new(),
        })));
        let source = Asking::over(Arc::clone(&asked) as Arc<dyn Runner>)
            .source()
            .await;
        assert_eq!(source, Some(Power::Mains));
        assert!(asked.ran(POWER_PMSET[0]));
    }

    #[test]
    fn the_seconds_field_is_read_rather_than_the_microseconds_one() {
        // `usec =` contains `sec =`, so a reader that took the wrong match would
        // report a machine as having started in 1970.
        assert_eq!(sysctl_moment(BOOTED), Some(1_694_612_345));
        assert_eq!(sysctl_moment("{ usec = 7 }"), None);
        assert_eq!(sysctl_moment("nothing like it"), None);
    }

    #[test]
    fn the_moment_is_picked_out_of_a_page_of_counters() {
        assert_eq!(proc_moment(COUNTERS), Some(1_694_612_345));
        assert_eq!(proc_moment("cpu 1 2 3\n"), None);
        assert_eq!(proc_moment("btime notanumber\n"), None);
    }

    #[test]
    fn only_the_two_phrases_that_have_not_moved_are_matched() {
        assert_eq!(drawing("Now drawing from 'AC Power'"), Some(Power::Mains));
        assert_eq!(
            drawing("now drawing from 'battery power'"),
            Some(Power::Battery)
        );
        assert_eq!(drawing("charged"), None);
    }

    #[test]
    fn a_mains_adapter_answers_one_or_nothing_this_understands() {
        assert_eq!(online(" 1 \n"), Some(Power::Mains));
        assert_eq!(online("0"), Some(Power::Battery));
        assert_eq!(online("Unknown"), None);
    }
}

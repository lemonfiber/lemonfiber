use std::sync::Arc;

use async_trait::async_trait;

use super::{drawing, online, proc_moment, sysctl_moment, Asking, PMSET, PS, SYSCTL};
use crate::ports::machine::{Power, Running, Started, Supply};
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
    assert!(asked.ran(SYSCTL));
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
    assert!(asked.ran(PMSET));
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

/// A runner on which `ps` answers as given, and every other program as given too.
struct Listing {
    /// What `ps` comes to.
    ps: Scripted,
    /// What anything else comes to.
    otherwise: Scripted,
}

#[async_trait]
impl Runner for Listing {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if argv.first().map(String::as_str) == Some(PS) {
            return self.ps.run(argv).await;
        }
        self.otherwise.run(argv).await
    }
}

/// A program's answer, as it exited and what it printed.
fn exited(status: i32, stdout: &str) -> Output {
    Output {
        status: Some(status),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// A program that is not on this machine.
fn missing() -> Result<Output, Failure> {
    Err(Failure::NotFound {
        program: "absent".to_owned(),
    })
}

/// Whether process 38480 is running, on a machine answering as given.
async fn is_running(
    ps: Result<Output, Failure>,
    otherwise: Result<Output, Failure>,
) -> Option<bool> {
    Asking::over(Arc::new(Listing {
        ps: Scripted(ps),
        otherwise: Scripted(otherwise),
    }))
    .alive(38_480)
    .await
}

#[tokio::test]
async fn a_process_is_running_where_the_listing_names_it_and_gone_where_it_names_nothing() {
    assert_eq!(
        is_running(Ok(exited(0, "38480\n")), missing()).await,
        Some(true)
    );
    assert_eq!(is_running(Ok(exited(1, "")), missing()).await, Some(false));
    // A listing naming something else, or a failure that printed something, is not
    // an answer about this process.
    assert_eq!(is_running(Ok(exited(0, "12\n")), missing()).await, None);
    assert_eq!(
        is_running(Ok(exited(1, "ps: illegal option")), missing()).await,
        None
    );
}

#[tokio::test]
async fn where_there_is_no_ps_the_windows_listing_is_asked() {
    let found =
        "\r\nlemonfiber.exe               38480 Console                    1     12,345 K\r\n";
    assert_eq!(
        is_running(missing(), Ok(exited(0, found))).await,
        Some(true)
    );
    let none = "INFO: No tasks are running which match the specified criteria.\r\n";
    assert_eq!(
        is_running(missing(), Ok(exited(0, none))).await,
        Some(false)
    );
    assert_eq!(is_running(missing(), Ok(exited(1, ""))).await, None);
    assert_eq!(is_running(missing(), missing()).await, None);
}

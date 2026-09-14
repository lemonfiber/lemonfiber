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
mod tests {
    use super::{Here, HOSTNAME};
    use crate::ports::network::Site;
    use crate::ports::process::Output;
    use lemonfiber_fixtures::support::{spoke, Recording, Scripted, Sequenced};
    use std::sync::Arc;

    /// A runner answering every program with the given output.
    fn saying(stdout: &str) -> Arc<Scripted> {
        Arc::new(Scripted(Ok(Output {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })))
    }

    /// What the open-files program writes for one listener, header and all.
    const OPEN_FILES: &str = "COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
         plex     4213 root   38u  IPv4 0x1a2b3c      0t0  TCP *:8989 (LISTEN)\n";

    /// What the socket program writes for one listener.
    const SOCKETS: &str = "LISTEN 0      4096         0.0.0.0:8989      0.0.0.0:*\n";

    #[tokio::test]
    async fn the_name_is_what_the_machine_says_it_is() {
        assert_eq!(
            Here::over(saying("kitchen-nas\n")).name().await,
            Some("kitchen-nas".to_owned())
        );
    }

    #[tokio::test]
    async fn a_qualified_name_keeps_every_part_but_its_trailing_dot() {
        assert_eq!(
            Here::over(saying("kitchen-nas.lan.\n")).name().await,
            Some("kitchen-nas.lan".to_owned())
        );
    }

    #[tokio::test]
    async fn a_machine_that_answers_with_nothing_has_no_name() {
        assert_eq!(Here::over(saying("   \n")).name().await, None);
        assert_eq!(Here::over(saying(".")).name().await, None);
    }

    #[tokio::test]
    async fn a_program_that_exits_badly_leaves_the_name_unknown() {
        let here = Here::over(Arc::new(Scripted(Ok(Output {
            status: Some(1),
            stdout: "kitchen-nas".to_owned(),
            stderr: String::new(),
        }))));
        assert_eq!(here.name().await, None);
    }

    #[tokio::test]
    async fn a_program_that_will_not_run_leaves_the_name_unknown() {
        let here = Here::over(Arc::new(Scripted(Err(
            crate::ports::process::Failure::NotFound {
                program: HOSTNAME.to_owned(),
            },
        ))));
        assert_eq!(here.name().await, None);
    }

    #[tokio::test]
    async fn the_name_is_asked_of_the_program_that_answers_it() {
        let asked = Arc::new(Recording::answering(Ok(Output {
            status: Some(0),
            stdout: "kitchen-nas".to_owned(),
            stderr: String::new(),
        })));
        let here = Here::over(Arc::clone(&asked) as Arc<dyn crate::ports::Runner>);
        assert_eq!(here.name().await, Some("kitchen-nas".to_owned()));
        assert!(asked.ran(HOSTNAME));
    }

    #[tokio::test]
    async fn a_wanted_port_a_listener_holds_is_named() {
        let here = Here::over(saying(OPEN_FILES));
        assert_eq!(here.answering_on(&[8989]).await, vec![8989]);
    }

    #[tokio::test]
    async fn a_port_nobody_asked_about_is_not_reported() {
        let here = Here::over(saying(OPEN_FILES));
        assert!(here.answering_on(&[7878]).await.is_empty());
    }

    /// The line carries three numbers and one of them is the port: the queue depth
    /// stands on its own, what the listener is connected to reads `0.0.0.0:*`, and
    /// only the address ends in a colon and a number. Asking about all three gets
    /// back the one that was written as somewhere to reach.
    #[tokio::test]
    async fn a_number_that_is_not_an_address_is_not_a_port() {
        let here = Here::over(saying(SOCKETS));
        assert_eq!(here.answering_on(&[8989, 4096, 1]).await, vec![8989]);
    }

    #[tokio::test]
    async fn an_address_written_the_newer_way_is_read_the_same_way() {
        let here = Here::over(saying("LISTEN 0 4096 [::]:8989 [::]:*\n"));
        assert_eq!(here.answering_on(&[8989]).await, vec![8989]);
    }

    #[tokio::test]
    async fn the_second_program_answers_where_the_first_is_not_installed() {
        let runner = Sequenced::answering(vec![
            Err(crate::ports::process::Failure::NotFound {
                program: "lsof".to_owned(),
            }),
            Ok(spoke(SOCKETS)),
        ]);
        let here = Here::over(Arc::clone(&runner) as Arc<dyn crate::ports::Runner>);

        assert_eq!(here.answering_on(&[8989]).await, vec![8989]);
        assert!(runner.ran("lsof"));
        assert!(runner.ran("ss"));
    }

    #[tokio::test]
    async fn a_machine_that_will_say_nothing_holds_nothing() {
        let here = Here::over(saying(""));
        assert!(here.answering_on(&[8989]).await.is_empty());
    }

    /// A program that is installed and truthfully found nothing has answered, so the
    /// second is never reached. Otherwise a machine that has only the first would
    /// spawn the second to fail on every start that holds no clash — which is most
    /// of them.
    #[tokio::test]
    async fn a_program_that_ran_and_found_nothing_ends_the_asking() {
        let runner = Sequenced::answering(vec![Ok(spoke(""))]);
        let here = Here::over(Arc::clone(&runner) as Arc<dyn crate::ports::Runner>);

        assert!(here.answering_on(&[8989]).await.is_empty());
        assert_eq!(runner.seen().len(), 1, "{:?}", runner.seen());
    }

    #[tokio::test]
    async fn a_machine_with_neither_program_holds_nothing() {
        let missing = || {
            Err(crate::ports::process::Failure::NotFound {
                program: "lsof".to_owned(),
            })
        };
        let runner = Sequenced::answering(vec![missing(), missing()]);
        let here = Here::over(Arc::clone(&runner) as Arc<dyn crate::ports::Runner>);

        assert!(here.answering_on(&[8989]).await.is_empty());
        assert_eq!(runner.seen().len(), 2, "{:?}", runner.seen());
    }

    /// A start that publishes no port has nothing to ask about, and asking anyway
    /// would spawn two programs to be told what was already known.
    #[tokio::test]
    async fn asking_about_no_ports_runs_nothing() {
        let runner = Sequenced::answering(vec![Ok(spoke(OPEN_FILES))]);
        let here = Here::over(Arc::clone(&runner) as Arc<dyn crate::ports::Runner>);

        assert!(here.answering_on(&[]).await.is_empty());
        assert!(runner.seen().is_empty());
    }
}

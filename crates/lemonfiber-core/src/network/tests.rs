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

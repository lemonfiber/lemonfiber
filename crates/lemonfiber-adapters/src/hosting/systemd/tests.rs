use super::{quoted, quoting, written, Host, Hosted, Standing, Systemd};
use lemonfiber_fixtures::support::Sequenced;
use lemonfiber_ports::hosting::{Failure, Held, Manager, Placed, Program};
use lemonfiber_ports::process::Output;
use lemonfiber_ports::Runner;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn spoke(status: i32, stdout: &str) -> Output {
    Output {
        status: Some(status),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

fn a_command(output: PathBuf) -> Hosted {
    Hosted {
        name: "expiring".to_owned(),
        program: PathBuf::from("/nowhere/lemonfiber"),
        arguments: vec!["household".to_owned(), "expiring".to_owned()],
        output,
        about: "closes requests nobody ruled on".to_owned(),
    }
}

fn units(name: &str) -> PathBuf {
    super::super::scratch(name)
}

fn over(dir: &Path, answers: Vec<Output>) -> (Systemd, Arc<Sequenced>) {
    let runner = Sequenced::answering(answers.into_iter().map(Ok).collect());
    (
        Systemd::over(dir.to_path_buf(), Arc::clone(&runner) as Arc<dyn Runner>),
        runner,
    )
}

#[test]
fn an_argument_goes_out_quoted_and_comes_back_as_it_was() {
    for word in [
        "/usr/local/bin/lemonfiber",
        "/media/My Films",
        "a \"quoted\" form",
        "back\\slash",
    ] {
        assert_eq!(quoted(&quoting(word)), vec![word.to_owned()]);
    }
    assert_eq!(
        quoted(&format!("{} {}", quoting("one"), quoting("two"))),
        vec!["one".to_owned(), "two".to_owned()]
    );
    assert!(quoted("").is_empty());
}

#[test]
fn the_unit_says_what_it_runs_where_it_writes_and_that_it_stays_ended() {
    let text = written(&a_command(PathBuf::from("/r/expiring.log")));
    assert!(text.contains("ExecStart=\"/nowhere/lemonfiber\" \"household\" \"expiring\""));
    assert!(text.contains("StandardOutput=append:/r/expiring.log"));
    assert!(text.contains("StandardError=append:/r/expiring.log"));
    assert!(text.contains("Restart=no"));
    assert!(text.contains("WantedBy=default.target"));
    assert!(text.contains("Description=lemonfiber: closes requests nobody ruled on"));
}

#[tokio::test]
async fn installing_re_reads_the_directory_then_enables_and_starts_it() {
    let dir = units("systemd-place");
    let (systemd, runner) = over(&dir, vec![spoke(0, ""), spoke(0, "")]);
    assert_eq!(
        systemd.place(&a_command(dir.join("expiring.log"))).await,
        Ok(Placed {
            definition: dir.join("lemonfiber-expiring.service"),
            started: true,
        })
    );
    assert!(runner.ran("daemon-reload"));
    assert!(runner.ran("enable"));
    assert!(runner.ran("--now"));
    assert!(dir.join("lemonfiber-expiring.service").is_file());
}

#[tokio::test]
async fn an_install_systemd_refused_leaves_no_unit_behind() {
    let dir = units("systemd-refused");
    let (systemd, _) = over(
        &dir,
        vec![
            spoke(0, ""),
            Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "Failed to enable unit: Unit file is masked".to_owned(),
            },
            spoke(0, ""),
        ],
    );
    assert_eq!(
        systemd.place(&a_command(dir.join("expiring.log"))).await,
        Err(Failure::Refused {
            manager: "systemd",
            reason: "Failed to enable unit: Unit file is masked".to_owned(),
        })
    );
    assert!(!dir.join("lemonfiber-expiring.service").exists());
}

#[tokio::test]
async fn an_install_that_could_not_run_systemctl_leaves_nothing_behind() {
    let dir = units("systemd-absent-systemctl");
    let (systemd, _) = over(&dir, vec![spoke(0, "")]);
    assert_eq!(
        systemd.place(&a_command(dir.join("expiring.log"))).await,
        Err(Failure::Refused {
            manager: "systemd",
            reason: "systemctl could not be run".to_owned(),
        })
    );
    assert!(!dir.join("lemonfiber-expiring.service").exists());
}

#[tokio::test]
async fn this_says_which_manager_it_speaks_to() {
    let (systemd, _) = over(&units("systemd-manager"), Vec::new());
    assert_eq!(systemd.manager(), Manager::Systemd);
}

#[tokio::test]
async fn a_name_nothing_is_installed_under_reads_as_nothing() {
    let dir = units("systemd-none");
    let (systemd, _) = over(&dir, Vec::new());
    assert_eq!(systemd.standing("expiring").await, Ok(Held::absent()));
    assert_eq!(systemd.withdraw("expiring").await, Ok(Vec::new()));
}

#[tokio::test]
async fn what_is_installed_is_read_back_out_of_the_unit_that_was_written() {
    let dir = units("systemd-standing");
    let (systemd, _) = over(&dir, vec![spoke(0, ""), spoke(0, ""), spoke(0, "active\n")]);
    let log = dir.join("expiring.log");
    assert!(systemd.place(&a_command(log.clone())).await.is_ok());
    assert_eq!(
        systemd.standing("expiring").await,
        Ok(Held {
            standing: Standing::Running,
            definition: Some(dir.join("lemonfiber-expiring.service")),
            program: Some(Program {
                at: PathBuf::from("/nowhere/lemonfiber"),
                present: false,
            }),
            runs: Some("/nowhere/lemonfiber household expiring".to_owned()),
            output: Some(log),
        })
    );
}

#[tokio::test]
async fn a_unit_naming_a_program_that_is_there_says_it_is_there() {
    let dir = units("systemd-present");
    let program = dir.join("lemonfiber");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&program, "");
    let _ = std::fs::write(
        dir.join("lemonfiber-expiring.service"),
        written(&Hosted {
            program: program.clone(),
            ..a_command(dir.join("expiring.log"))
        }),
    );
    let (systemd, _) = over(&dir, vec![spoke(0, "active")]);
    assert_eq!(
        systemd.standing("expiring").await.map(|held| held.program),
        Ok(Some(Program {
            at: program,
            present: true,
        }))
    );
}

#[tokio::test]
async fn a_unit_that_says_nothing_about_what_it_runs_names_nothing() {
    let dir = units("systemd-bare");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("lemonfiber-expiring.service"), "[Unit]\n");
    let (systemd, _) = over(&dir, vec![spoke(0, "inactive")]);
    assert_eq!(
        systemd.standing("expiring").await,
        Ok(Held {
            standing: Standing::Stopped,
            definition: Some(dir.join("lemonfiber-expiring.service")),
            program: None,
            runs: None,
            output: None,
        })
    );
}

#[tokio::test]
async fn every_answer_systemctl_can_give_about_a_unit_is_read() {
    let dir = units("systemd-says");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("lemonfiber-expiring.service"), "[Unit]\n");
    let cases = [
        (vec![spoke(0, "active\n")], Standing::Running),
        (vec![spoke(3, "inactive\n")], Standing::Stopped),
        (vec![spoke(3, "failed\n")], Standing::Stopped),
        (vec![spoke(0, "activating\n")], Standing::Stopped),
        (vec![spoke(0, "deactivating\n")], Standing::Stopped),
        (vec![spoke(4, "unknown\n")], Standing::Unsaid),
        (Vec::new(), Standing::Unsaid),
    ];
    for (answers, expected) in cases {
        let (systemd, _) = over(&dir, answers);
        assert_eq!(
            systemd.standing("expiring").await.map(|held| held.standing),
            Ok(expected)
        );
    }
}

#[tokio::test]
async fn taking_it_back_disables_it_first_and_then_the_unit_goes() {
    let dir = units("systemd-withdraw");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("lemonfiber-expiring.service");
    let _ = std::fs::write(&at, "[Unit]\n");
    let (systemd, runner) = over(
        &dir,
        vec![spoke(0, ""), spoke(3, "inactive\n"), spoke(0, "")],
    );
    assert_eq!(systemd.withdraw("expiring").await, Ok(vec![at.clone()]));
    assert!(runner.ran("disable"));
    assert!(runner.ran("daemon-reload"));
    assert!(!at.exists());
}

/// Nothing here is ever asked of the system manager, in either direction.
///
/// The requirement is that hosting needs no administrative rights, and the whole
/// of what keeps that true is one flag: `systemctl` without `--user` addresses
/// the system instance, which would need them and would run as somebody else.
/// Asserted over every call rather than at each site, because a flag that has to
/// be remembered at four call sites is one that will be forgotten at a fifth.
#[tokio::test]
async fn every_call_addresses_the_operators_own_session_and_never_the_system() {
    let dir = units("systemd-own-session");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("lemonfiber-expiring.service");
    let _ = std::fs::write(&at, "[Unit]\n");
    let (systemd, runner) = over(
        &dir,
        vec![
            spoke(0, ""),
            spoke(0, ""),
            spoke(0, "active\n"),
            spoke(0, ""),
            spoke(3, "inactive\n"),
            spoke(0, ""),
        ],
    );
    assert!(systemd
        .place(&a_command(dir.join("expiring.log")))
        .await
        .is_ok());
    assert!(systemd.standing("expiring").await.is_ok());
    assert!(systemd.withdraw("expiring").await.is_ok());

    let calls = runner.seen();
    assert!(calls.len() >= 5, "the three operations ran: {calls:?}");
    for argv in calls {
        assert!(
            argv.first().is_some_and(|program| program == "systemctl")
                && argv.iter().any(|word| word == "--user"),
            "{argv:?} would have been addressed to the system manager"
        );
    }
}

#[tokio::test]
async fn one_that_is_still_running_afterwards_is_refused_and_kept() {
    let dir = units("systemd-stubborn");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("lemonfiber-expiring.service");
    let _ = std::fs::write(&at, "[Unit]\n");
    let (systemd, _) = over(&dir, vec![spoke(0, ""), spoke(0, "active\n")]);
    assert_eq!(
        systemd.withdraw("expiring").await,
        Err(Failure::Refused {
            manager: "systemd",
            reason: "it is still running".to_owned(),
        })
    );
    assert!(at.exists());
}

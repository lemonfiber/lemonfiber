use super::{defined, quoted, quoting, read_back, written, Host, Hosted, Standing, TaskScheduler};
use lemonfiber_fixtures::support::Sequenced;
use lemonfiber_ports::hosting::{Cadence, Failure, Manager, Placed};
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
        name: "watch".to_owned(),
        program: PathBuf::from("C:/Program Files/lemonfiber/lemonfiber.exe"),
        arguments: vec!["watch".to_owned(), "100%".to_owned()],
        output,
        about: "guards the data location".to_owned(),
        cadence: Cadence::Kept,
    }
}

fn every_minute(output: PathBuf) -> Hosted {
    Hosted {
        cadence: Cadence::Minutely,
        ..a_command(output)
    }
}

fn scripts(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    super::super::scratch(name)
}

fn over(dir: &Path, answers: Vec<Output>) -> (TaskScheduler, Arc<Sequenced>) {
    let runner = Sequenced::answering(answers.into_iter().map(Ok).collect());
    (
        TaskScheduler::over(dir.to_path_buf(), Arc::clone(&runner) as Arc<dyn Runner>),
        runner,
    )
}

fn script(dir: &Path) -> PathBuf {
    dir.join("lemonfiber-watch.cmd")
}

fn operator() -> Output {
    spoke(0, "host\\op\r\n")
}

#[test]
fn a_kept_command_is_defined_to_start_at_the_operators_own_logon_with_their_least_privilege() {
    let xml = defined(
        &a_command(PathBuf::from("C:/r/watch.log")),
        "host\\op & co",
        Path::new("C:/tasks/lemonfiber-watch.cmd"),
    );
    assert!(xml.contains(
        "<LogonTrigger><Enabled>true</Enabled><UserId>host\\op &amp; co</UserId></LogonTrigger>"
    ));
    assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
    assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
    assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
    assert!(xml.contains("<Command>C:/tasks/lemonfiber-watch.cmd</Command>"));
    assert!(xml.contains("<Description>lemonfiber: guards the data location</Description>"));
    assert!(!xml.contains("TimeTrigger"));
}

#[test]
fn a_command_started_every_minute_is_defined_to_repeat_every_minute_for_good() {
    let xml = defined(
        &every_minute(PathBuf::from("C:/r/watch.log")),
        "op",
        Path::new("C:/tasks/lemonfiber-watch.cmd"),
    );
    assert!(xml.contains("<Repetition><Interval>PT1M</Interval>"));
    assert!(xml.contains("<StopAtDurationEnd>false</StopAtDurationEnd>"));
    assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
    assert!(!xml.contains("LogonTrigger"));
}

#[test]
fn a_word_goes_out_quoted_and_comes_back_as_it_was() {
    for word in ["C:/Program Files/x.exe", "100%", "50%% done", "a & b | c"] {
        assert_eq!(quoted(&quoting(word)), vec![word.to_owned()]);
    }
}

#[test]
fn the_script_says_what_it_runs_and_where_it_writes_and_reads_back_the_same() {
    let hosted = a_command(PathBuf::from("C:/r/watch.log"));
    let text = written(&hosted);
    assert!(text.contains("rem lemonfiber: guards the data location"));
    assert!(!text.contains("every minute"));
    assert_eq!(
        read_back(&text),
        (
            vec![
                "C:/Program Files/lemonfiber/lemonfiber.exe".to_owned(),
                "watch".to_owned(),
                "100%".to_owned(),
            ],
            Some(PathBuf::from("C:/r/watch.log"))
        )
    );
    assert!(written(&every_minute(PathBuf::from("C:/r/watch.log"))).contains("rem every minute"));
}

#[test]
fn a_script_naming_nothing_it_runs_reads_back_as_nothing() {
    assert_eq!(read_back("@echo off\r\n"), (Vec::new(), None));
}

#[tokio::test]
async fn this_says_which_manager_it_speaks_to() {
    let (scheduler, _) = over(Path::new("/nowhere"), Vec::new());
    assert_eq!(scheduler.manager(), Manager::TaskScheduler);
}

#[tokio::test]
async fn a_command_is_registered_from_its_definition_and_started() {
    let dir = scripts("tasks-kept");
    let (scheduler, runner) = over(&dir, vec![operator(), spoke(0, ""), spoke(0, "")]);
    assert_eq!(
        scheduler.place(&a_command(dir.join("watch.log"))).await,
        Ok(Placed {
            definition: script(&dir),
            started: true,
        })
    );
    assert!(runner.ran("whoami") && runner.ran("/XML") && runner.ran("/Run"));
    assert!(script(&dir).is_file());
    assert!(!dir.join("lemonfiber-watch.xml").exists());
}

#[tokio::test]
async fn a_registration_that_would_not_start_says_it_did_not() {
    let dir = scripts("tasks-minutely");
    let (scheduler, _) = over(&dir, vec![operator(), spoke(0, ""), spoke(1, "")]);
    assert_eq!(
        scheduler.place(&every_minute(dir.join("watch.log"))).await,
        Ok(Placed {
            definition: script(&dir),
            started: false,
        })
    );
}

#[tokio::test]
async fn a_registration_refused_leaves_no_script_behind() {
    let dir = scripts("tasks-refused");
    let (scheduler, _) = over(
        &dir,
        vec![
            operator(),
            Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "ERROR: Access is denied.".to_owned(),
            },
        ],
    );
    assert_eq!(
        scheduler.place(&a_command(dir.join("watch.log"))).await,
        Err(Failure::Refused {
            manager: "Task Scheduler",
            reason: "ERROR: Access is denied.".to_owned(),
        })
    );
    assert!(!script(&dir).exists());
    assert!(!dir.join("lemonfiber-watch.xml").exists());
}

#[tokio::test]
async fn a_registration_that_could_not_run_schtasks_leaves_nothing_behind() {
    let dir = scripts("tasks-absent");
    let (scheduler, _) = over(&dir, vec![operator()]);
    assert!(scheduler
        .place(&a_command(dir.join("watch.log")))
        .await
        .is_err());
    assert!(!script(&dir).exists());
}

#[tokio::test]
async fn an_account_that_cannot_be_named_is_refused_before_anything_is_written() {
    let dir = scripts("tasks-nobody");
    let (scheduler, _) = over(&dir, vec![spoke(0, "  \r\n")]);
    assert!(scheduler
        .place(&a_command(dir.join("watch.log")))
        .await
        .is_err());
    assert!(!script(&dir).exists());
}

#[tokio::test]
async fn a_value_with_a_quote_in_it_is_never_written_or_registered() {
    let dir = scripts("tasks-quote");
    let (scheduler, runner) = over(&dir, Vec::new());
    let hosted = Hosted {
        arguments: vec!["a\" & del x".to_owned()],
        ..a_command(dir.join("watch.log"))
    };
    assert!(matches!(
        scheduler.place(&hosted).await,
        Err(Failure::Unwritable { .. })
    ));
    assert!(runner.seen().is_empty());
    assert!(!script(&dir).exists());
}

#[tokio::test]
async fn every_status_the_scheduler_gives_is_read() {
    let dir = scripts("tasks-status");
    let _ = std::fs::create_dir_all(&dir);
    for (cadence, answer, standing) in [
        (
            Cadence::Kept,
            spoke(0, "Status:        Running\r\n"),
            Standing::Running,
        ),
        (
            Cadence::Kept,
            spoke(0, "Status:        Ready\r\n"),
            Standing::Stopped,
        ),
        (
            Cadence::Kept,
            spoke(0, "Status:        Disabled\r\n"),
            Standing::Stopped,
        ),
        (
            Cadence::Kept,
            spoke(0, "Status:        Bereit\r\n"),
            Standing::Unsaid,
        ),
        (Cadence::Kept, spoke(1, ""), Standing::Stopped),
        (
            Cadence::Minutely,
            spoke(0, "Status:        Ready\r\n"),
            Standing::Running,
        ),
    ] {
        let hosted = Hosted {
            cadence,
            ..a_command(dir.join("watch.log"))
        };
        let _ = std::fs::write(script(&dir), written(&hosted));
        let (scheduler, _) = over(&dir, vec![answer]);
        assert_eq!(
            scheduler.standing("watch").await.map(|held| held.standing),
            Ok(standing)
        );
    }
    let (scheduler, _) = over(&dir, Vec::new());
    assert_eq!(
        scheduler.standing("watch").await.map(|held| held.standing),
        Ok(Standing::Unsaid)
    );
}

#[tokio::test]
async fn what_is_registered_is_read_back_out_of_the_script() {
    let dir = scripts("tasks-read");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(script(&dir), written(&a_command(dir.join("watch.log"))));
    let (scheduler, _) = over(&dir, vec![spoke(0, "Status: Ready\r\n")]);
    let held = scheduler.standing("watch").await;
    assert_eq!(
        held.as_ref()
            .map(|held| (held.runs.clone(), held.output.clone())),
        Ok((
            Some("C:/Program Files/lemonfiber/lemonfiber.exe watch 100%".to_owned()),
            Some(dir.join("watch.log"))
        ))
    );
    assert!(held.is_ok_and(|held| held.program.is_some_and(|program| !program.present)));
}

#[tokio::test]
async fn a_name_nothing_is_registered_under_reads_as_nothing_and_withdraws_nothing() {
    let dir = scripts("tasks-none");
    let (scheduler, runner) = over(&dir, Vec::new());
    assert_eq!(
        scheduler.standing("watch").await.map(|held| held.standing),
        Ok(Standing::Absent)
    );
    assert_eq!(scheduler.withdraw("watch").await, Ok(Vec::new()));
    assert!(runner.seen().is_empty());
}

#[tokio::test]
async fn taking_it_back_ends_and_deletes_the_task_and_then_the_script_goes() {
    let dir = scripts("tasks-withdraw");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(script(&dir), "@echo off\r\n");
    let (scheduler, runner) = over(&dir, vec![spoke(0, ""), spoke(0, "")]);
    assert_eq!(scheduler.withdraw("watch").await, Ok(vec![script(&dir)]));
    assert!(runner.ran("/End") && runner.ran("/Delete"));
    assert!(!script(&dir).exists());
}

#[tokio::test]
async fn a_task_the_scheduler_will_not_delete_keeps_its_script() {
    let dir = scripts("tasks-kept-back");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(script(&dir), "@echo off\r\n");
    let (scheduler, _) = over(
        &dir,
        vec![
            spoke(0, ""),
            Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "ERROR: Access is denied.".to_owned(),
            },
        ],
    );
    assert!(scheduler.withdraw("watch").await.is_err());
    assert!(script(&dir).exists());
    let (unreached, _) = over(&dir, vec![spoke(0, "")]);
    assert!(unreached.withdraw("watch").await.is_err());
    assert!(script(&dir).exists());
}

#[tokio::test]
async fn an_account_that_cannot_be_asked_for_is_refused_before_anything_is_written() {
    let dir = scripts("tasks-unasked");
    let (scheduler, runner) = over(&dir, Vec::new());
    assert!(scheduler
        .place(&a_command(dir.join("watch.log")))
        .await
        .is_err());
    assert!(!script(&dir).exists());
    assert!(!runner.ran("/Create"));
}

#[tokio::test]
async fn a_script_that_cannot_be_written_is_refused_and_nothing_registered() {
    let dir = scripts("tasks-unwritable");
    let _ = std::fs::create_dir_all(dir.parent().unwrap_or(&dir));
    let _ = std::fs::write(&*dir, "");
    let (scheduler, runner) = over(&dir, vec![operator()]);
    let placed = scheduler.place(&a_command(dir.join("watch.log"))).await;
    let _ = std::fs::remove_file(&*dir);
    assert!(matches!(placed, Err(Failure::Unwritable { .. })));
    assert!(!runner.ran("/Create"));
}

#[tokio::test]
async fn a_definition_that_cannot_be_written_takes_its_script_back() {
    let dir = scripts("tasks-undefined");
    let _ = std::fs::create_dir_all(script(&dir).with_extension("xml"));
    let (scheduler, runner) = over(&dir, vec![operator()]);
    assert!(matches!(
        scheduler.place(&every_minute(dir.join("watch.log"))).await,
        Err(Failure::Unwritable { .. })
    ));
    assert!(!script(&dir).exists());
    assert!(!runner.ran("/Create"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_script_that_will_not_go_once_its_task_is_deleted_is_refused_as_unwritable() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scripts("tasks-script-kept");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(script(&dir), "@echo off\r\n");
    let _ = std::fs::set_permissions(&*dir, std::fs::Permissions::from_mode(0o555));
    let (scheduler, _) = over(&dir, vec![spoke(0, ""), spoke(0, "")]);
    let withdrawn = scheduler.withdraw("watch").await;
    let _ = std::fs::set_permissions(&*dir, std::fs::Permissions::from_mode(0o755));
    assert!(
        matches!(withdrawn, Err(Failure::Unwritable { ref at, .. }) if *at == script(&dir)),
        "{withdrawn:?}"
    );
}

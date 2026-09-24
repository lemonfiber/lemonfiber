use super::{arguments, escaped, inside, plain, under, Host, Hosted, Launchd, Standing, OUT};
use lemonfiber_fixtures::support::Sequenced;
use lemonfiber_ports::hosting::{Failure, Held, Manager, Program};
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
        program: PathBuf::from("/nowhere/lemonfiber"),
        arguments: vec!["watch".to_owned(), "full".to_owned()],
        output,
        about: "guards the data location".to_owned(),
    }
}

fn agents(name: &str) -> PathBuf {
    super::super::scratch(name)
}

fn over(dir: &Path, answers: Vec<Output>) -> (Launchd, Arc<Sequenced>) {
    let runner = Sequenced::answering(answers.into_iter().map(Ok).collect());
    (
        Launchd::over(dir.to_path_buf(), Arc::clone(&runner) as Arc<dyn Runner>),
        runner,
    )
}

#[test]
fn the_three_characters_a_plist_cannot_carry_go_out_and_come_back() {
    let awkward = "/media/rock & <roll>";
    assert_eq!(escaped(awkward), "/media/rock &amp; &lt;roll&gt;");
    assert_eq!(plain(&escaped(awkward)), awkward);
    // An escape written into a path is a path, not a bracket, on the way back.
    assert_eq!(plain(&escaped("&lt;")), "&lt;");
}

#[test]
fn only_a_string_element_on_its_own_line_has_anything_inside_it() {
    assert_eq!(inside("  <string>one</string>").as_deref(), Some("one"));
    assert_eq!(inside("<true/>"), None);
}

#[test]
fn the_arguments_are_the_strings_between_the_array_markers() {
    let text = super::written(
        "com.lemonfiber.watch",
        &a_command(PathBuf::from("/r/w.log")),
    );
    assert_eq!(
        arguments(&text),
        vec![
            "/nowhere/lemonfiber".to_owned(),
            "watch".to_owned(),
            "full".to_owned()
        ]
    );
    assert_eq!(under(&text, OUT).as_deref(), Some("/r/w.log"));
    assert!(text.contains("<key>Label</key>\n<string>com.lemonfiber.watch</string>"));
    assert!(text.contains("<key>RunAtLoad</key>"));
}

#[test]
fn nothing_written_here_asks_to_be_brought_back_after_it_ends() {
    let text = super::written(
        "com.lemonfiber.watch",
        &a_command(PathBuf::from("/r/w.log")),
    );
    assert!(!text.contains("KeepAlive"));
}

#[test]
fn a_plist_with_no_array_and_no_key_yields_neither() {
    assert!(arguments("<dict>\n</dict>\n").is_empty());
    assert_eq!(under("<dict>\n</dict>\n", OUT), None);
    assert_eq!(under("<key>StandardOutPath</key>\n", OUT), None);
}

#[tokio::test]
async fn installing_asks_the_session_unloads_the_old_and_bootstraps_the_new() {
    let dir = agents("launchd-place");
    let (launchd, runner) = over(
        &dir,
        vec![
            spoke(0, "501\n"),
            spoke(0, "501\n"),
            spoke(0, ""),
            spoke(0, ""),
        ],
    );
    let placed = launchd.place(&a_command(dir.join("watch.log"))).await;
    assert_eq!(
        placed,
        Ok(lemonfiber_ports::hosting::Placed {
            definition: dir.join("com.lemonfiber.watch.plist"),
            started: true,
        })
    );
    assert!(runner.ran("bootout"));
    assert!(runner.ran("bootstrap"));
    assert!(dir.join("com.lemonfiber.watch.plist").is_file());
}

#[tokio::test]
async fn an_install_the_manager_refused_leaves_no_definition_behind() {
    let dir = agents("launchd-refused");
    let (launchd, _) = over(
        &dir,
        vec![
            spoke(0, "501\n"),
            spoke(0, "501\n"),
            spoke(0, ""),
            Output {
                status: Some(5),
                stdout: String::new(),
                stderr: "Load failed: 5: Input/output error".to_owned(),
            },
        ],
    );
    assert_eq!(
        launchd.place(&a_command(dir.join("watch.log"))).await,
        Err(Failure::Refused {
            manager: "launchd",
            reason: "Load failed: 5: Input/output error".to_owned(),
        })
    );
    assert!(!dir.join("com.lemonfiber.watch.plist").exists());
}

#[tokio::test]
async fn an_install_that_cannot_name_the_session_or_run_launchctl_leaves_nothing() {
    let dir = agents("launchd-no-session");
    let (launchd, _) = over(&dir, vec![spoke(1, "")]);
    assert_eq!(
        launchd.place(&a_command(dir.join("watch.log"))).await,
        Err(Failure::Refused {
            manager: "launchd",
            reason: "this login session could not be identified".to_owned(),
        })
    );
    assert!(!dir.join("com.lemonfiber.watch.plist").exists());

    let gone = agents("launchd-no-launchctl");
    let (absent, _) = over(&gone, vec![spoke(0, "501"), spoke(0, "501"), spoke(0, "")]);
    assert_eq!(
        absent.place(&a_command(gone.join("watch.log"))).await,
        Err(Failure::Refused {
            manager: "launchd",
            reason: "launchctl could not be run".to_owned(),
        })
    );
    assert!(!gone.join("com.lemonfiber.watch.plist").exists());
}

#[tokio::test]
async fn this_says_which_manager_it_speaks_to() {
    let (launchd, _) = over(&agents("launchd-manager"), Vec::new());
    assert_eq!(launchd.manager(), Manager::Launchd);
}

/// A session nothing can name unloads nothing, and the definition still goes.
///
/// Removing it is what stops it coming back at the next login, which is the
/// whole of what an installation was; and nothing here is restarted, so a run
/// still going is one that ends on its own terms. Refusing would be refusing on
/// a reading this machine declined to give.
#[tokio::test]
async fn a_session_that_cannot_be_named_unloads_nothing_and_still_takes_it_back() {
    let dir = agents("launchd-nameless-session");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("com.lemonfiber.watch.plist");
    let _ = std::fs::write(&at, "<dict>\n</dict>\n");
    let (launchd, runner) = over(&dir, vec![spoke(1, ""), spoke(1, "")]);

    assert_eq!(launchd.withdraw("watch").await, Ok(vec![at.clone()]));
    assert!(
        !runner.ran("bootout"),
        "it addressed a session nothing had named"
    );
    assert!(!at.exists());
}

#[tokio::test]
async fn a_name_nothing_is_installed_under_reads_as_nothing() {
    let dir = agents("launchd-absent");
    let (launchd, _) = over(&dir, Vec::new());
    assert_eq!(launchd.standing("watch").await, Ok(Held::absent()));
    assert_eq!(launchd.withdraw("watch").await, Ok(Vec::new()));
}

#[tokio::test]
async fn what_is_installed_is_read_back_out_of_the_definition_that_was_written() {
    let dir = agents("launchd-standing");
    let (launchd, _) = over(
        &dir,
        vec![
            spoke(0, "501"),
            spoke(0, "501"),
            spoke(0, ""),
            spoke(0, ""),
            spoke(0, "501"),
            spoke(0, "\tstate = running\n"),
        ],
    );
    let log = dir.join("watch.log");
    assert!(launchd.place(&a_command(log.clone())).await.is_ok());
    assert_eq!(
        launchd.standing("watch").await,
        Ok(Held {
            standing: Standing::Running,
            definition: Some(dir.join("com.lemonfiber.watch.plist")),
            program: Some(Program {
                at: PathBuf::from("/nowhere/lemonfiber"),
                present: false,
            }),
            runs: Some("/nowhere/lemonfiber watch full".to_owned()),
            output: Some(log),
        })
    );
}

#[tokio::test]
async fn a_definition_naming_a_program_that_is_there_says_it_is_there() {
    let dir = agents("launchd-present");
    let program = dir.join("lemonfiber");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&program, "");
    let (launchd, _) = over(&dir, vec![spoke(0, "501"), spoke(0, "\tstate = running\n")]);
    let text = super::written(
        "com.lemonfiber.watch",
        &Hosted {
            program: program.clone(),
            ..a_command(dir.join("watch.log"))
        },
    );
    let _ = std::fs::write(dir.join("com.lemonfiber.watch.plist"), text);
    assert_eq!(
        launchd.standing("watch").await.map(|held| held.program),
        Ok(Some(Program {
            at: program,
            present: true,
        }))
    );
}

#[tokio::test]
async fn every_answer_launchctl_can_give_about_a_loaded_agent_is_read() {
    let dir = agents("launchd-says");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("com.lemonfiber.watch.plist"), "<dict>\n</dict>\n");
    let cases = [
        (
            vec![spoke(0, "501"), spoke(0, "state = running")],
            Standing::Running,
        ),
        (
            vec![spoke(0, "501"), spoke(0, "state = not running")],
            Standing::Stopped,
        ),
        (
            vec![spoke(0, "501"), spoke(0, "nothing useful")],
            Standing::Unsaid,
        ),
        (vec![spoke(0, "501"), spoke(1, "")], Standing::Stopped),
        (vec![spoke(0, "501")], Standing::Unsaid),
        (vec![spoke(1, "")], Standing::Unsaid),
    ];
    for (answers, expected) in cases {
        let (launchd, _) = over(&dir, answers);
        assert_eq!(
            launchd.standing("watch").await.map(|held| held.standing),
            Ok(expected)
        );
    }
}

#[tokio::test]
async fn taking_it_back_unloads_it_first_and_then_the_definition_goes() {
    let dir = agents("launchd-withdraw");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("com.lemonfiber.watch.plist");
    let _ = std::fs::write(&at, "<dict>\n</dict>\n");
    let (launchd, runner) = over(
        &dir,
        vec![spoke(0, "501"), spoke(0, ""), spoke(0, "501"), spoke(1, "")],
    );
    assert_eq!(launchd.withdraw("watch").await, Ok(vec![at.clone()]));
    assert!(runner.ran("bootout"));
    assert!(!at.exists());
}

/// Nothing here is ever asked of a domain the operator does not own.
///
/// The requirement is that hosting needs no administrative rights, and on this
/// platform the whole of what keeps that true is which domain a target names:
/// `gui/<uid>` is the operator's own login session, and `system/` is the one
/// that would need them. Asserted over every target rather than at each site.
#[tokio::test]
async fn every_target_names_the_operators_own_login_session() {
    let dir = agents("launchd-own-session");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("com.lemonfiber.watch.plist");
    let _ = std::fs::write(&at, "<dict>\n</dict>\n");
    let (launchd, runner) = over(
        &dir,
        vec![spoke(0, "501"), spoke(0, ""), spoke(0, "501"), spoke(1, "")],
    );
    assert!(launchd.withdraw("watch").await.is_ok());

    let targets: Vec<String> = runner
        .seen()
        .into_iter()
        .filter(|argv| argv.first().is_some_and(|program| program == "launchctl"))
        .filter_map(|argv| argv.last().cloned())
        .collect();
    assert!(!targets.is_empty(), "launchctl was reached at all");
    for target in targets {
        assert!(
            target.starts_with("gui/"),
            "{target} is not the operator's own login session"
        );
    }
}

/// Installing a command that is already hosted leaves one agent, not two.
///
/// The manager is told to take the old one out before the new one is put in, so
/// an operator who installs twice has one thing running their command. Two
/// agents under one name would each run it, and the second would be invisible in
/// a reading that names the command rather than the agent.
#[tokio::test]
async fn installing_what_is_already_hosted_leaves_one_of_it() {
    let dir = agents("launchd-twice");
    let (launchd, runner) = over(
        &dir,
        vec![
            spoke(0, "501\n"),
            spoke(0, "501\n"),
            spoke(0, ""),
            spoke(0, ""),
            spoke(0, "501\n"),
            spoke(0, "501\n"),
            spoke(0, ""),
            spoke(0, ""),
        ],
    );
    let command = a_command(dir.join("watch.log"));

    assert!(launchd.place(&command).await.is_ok(), "the first install");
    assert!(launchd.place(&command).await.is_ok(), "and the second");

    let definitions = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|kind| kind == "plist"))
        .count();
    assert_eq!(definitions, 1, "installing twice left more than one agent");
    assert!(runner.ran("bootout"), "the one already there was taken out");
}

#[tokio::test]
async fn one_that_is_still_running_afterwards_is_refused_and_kept() {
    let dir = agents("launchd-stubborn");
    let _ = std::fs::create_dir_all(&dir);
    let at = dir.join("com.lemonfiber.watch.plist");
    let _ = std::fs::write(&at, "<dict>\n</dict>\n");
    let (launchd, _) = over(
        &dir,
        vec![
            spoke(0, "501"),
            spoke(0, ""),
            spoke(0, "501"),
            spoke(0, "state = running"),
        ],
    );
    assert_eq!(
        launchd.withdraw("watch").await,
        Err(Failure::Refused {
            manager: "launchd",
            reason: "it is still running".to_owned(),
        })
    );
    assert!(at.exists());
}

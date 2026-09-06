//! A user service in the operator's own session.
//!
//! `--user` throughout, which is the whole of what keeps this out of root: the
//! unit lives under the operator's own configuration directory and is managed by
//! the instance of systemd their login already runs, so nothing here needs a
//! privilege they do not have and nothing another account on the machine picks
//! up. A system unit would have needed one, and would have run as somebody else
//! against a library owned by them.
//!
//! What that costs is stated where the report is built rather than worked around
//! here: a user session ends at logout unless the account is set to linger, and
//! turning that on for somebody is not this program's to do quietly.
//!
//! The unit carries `Restart=no`. See the note on the module above.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed, Program, Standing};
use crate::ports::process::Output;
use crate::ports::Runner;

use super::{after, complaint, definition, put, take};

/// What this manager is called where its refusals are reported.
const MANAGER: &str = "systemd";

/// What `systemctl is-active` says about a unit it is running.
const ACTIVE: &str = "active";

/// The key naming what a unit runs.
const RUNS: &str = "ExecStart=";

/// The key naming where it writes, and the way a file is named after it.
const WRITES: &str = "StandardOutput=";

/// How systemd is told to append to a file rather than take over one.
const APPEND: &str = "append:";

/// A user service, written into a directory and managed through `systemctl`.
pub struct Systemd {
    units: PathBuf,
    runner: Arc<dyn Runner>,
}

impl Systemd {
    /// Units kept in this directory, managed by running programs through this runner.
    #[must_use]
    pub const fn over(units: PathBuf, runner: Arc<dyn Runner>) -> Self {
        Self { units, runner }
    }

    /// The unit systemd knows one of lemonfiber's commands by.
    fn unit(name: &str) -> String {
        format!("lemonfiber-{name}.service")
    }

    /// Where that unit's definition is written.
    fn at(&self, name: &str) -> PathBuf {
        self.units.join(Self::unit(name))
    }

    /// Run a program, or nothing where it could not be run at all.
    async fn spoke(&self, argv: &[&str]) -> Option<Output> {
        let argv: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        self.runner.run(&argv).await.ok()
    }

    /// Tell systemd to re-read what is on disk.
    async fn reread(&self) {
        let _ = self.spoke(&["systemctl", "--user", "daemon-reload"]).await;
    }

    /// What systemd says about a unit it may or may not be running.
    ///
    /// Read from what it wrote rather than from how it exited: `is-active`
    /// answers a question by its exit status, so a unit that is merely not
    /// running looks exactly like a systemd that could not be asked.
    async fn says(&self, name: &str) -> Standing {
        let unit = Self::unit(name);
        let Some(output) = self
            .spoke(&["systemctl", "--user", "is-active", &unit])
            .await
        else {
            return Standing::Unsaid;
        };
        match output.stdout.trim() {
            ACTIVE => Standing::Running,
            "inactive" | "failed" | "activating" | "deactivating" => Standing::Stopped,
            _ => Standing::Unsaid,
        }
    }
}

/// The unit systemd reads, one value to a line so it can be read back.
fn written(hosted: &Hosted) -> String {
    let out = hosted.output.to_string_lossy();
    let runs: Vec<String> = std::iter::once(hosted.program.to_string_lossy().into_owned())
        .chain(hosted.arguments.iter().cloned())
        .map(|word| quoting(&word))
        .collect();
    let runs = runs.join(" ");
    let about = &hosted.about;
    format!(
        "[Unit]\n\
         Description=lemonfiber: {about}\n\
         \n\
         [Service]\n\
         Type=simple\n\
         {RUNS}{runs}\n\
         Restart=no\n\
         {WRITES}{APPEND}{out}\n\
         StandardError={APPEND}{out}\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

/// One argument as systemd reads them, with the two characters it escapes escaped.
fn quoting(word: &str) -> String {
    format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The arguments of a quoted command line, with those escapes taken back out.
fn quoted(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut word = String::new();
    let mut inside = false;
    let mut escaping = false;
    for letter in line.chars() {
        if escaping {
            word.push(letter);
            escaping = false;
        } else if letter == '\\' {
            escaping = true;
        } else if letter == '"' {
            if inside {
                found.push(std::mem::take(&mut word));
            }
            inside = !inside;
        } else if inside {
            word.push(letter);
        }
    }
    found
}

#[async_trait]
impl Host for Systemd {
    fn manager(&self) -> Manager {
        Manager::Systemd
    }

    async fn place(&self, hosted: &Hosted) -> Result<Placed, Failure> {
        let at = self.at(&hosted.name);
        put(&at, &written(hosted))?;
        self.reread().await;
        let unit = Self::unit(&hosted.name);
        // Enabling and starting in one act, so a name that was already installed
        // is replaced rather than joined by a second under a different unit.
        let told = self
            .spoke(&["systemctl", "--user", "enable", "--now", &unit])
            .await;
        match told {
            Some(output) if output.succeeded() => Ok(Placed {
                definition: at,
                started: true,
            }),
            Some(output) => {
                let _ = take(&at);
                self.reread().await;
                Err(refused(&complaint(&output)))
            }
            None => {
                let _ = take(&at);
                Err(refused("systemctl could not be run"))
            }
        }
    }

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        let at = self.at(name);
        let Some(text) = definition(&at) else {
            return Ok(Held::absent());
        };
        let arguments = after(&text, RUNS)
            .map(|line| quoted(&line))
            .unwrap_or_default();
        Ok(Held {
            standing: self.says(name).await,
            definition: Some(at),
            program: arguments.first().map(|at| Program {
                at: PathBuf::from(at),
                present: Path::new(at).exists(),
            }),
            runs: (!arguments.is_empty()).then(|| arguments.join(" ")),
            output: after(&text, WRITES)
                .and_then(|value| value.strip_prefix(APPEND).map(PathBuf::from)),
        })
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        let at = self.at(name);
        if definition(&at).is_none() {
            return Ok(Vec::new());
        }
        let unit = Self::unit(name);
        let _ = self
            .spoke(&["systemctl", "--user", "disable", "--now", &unit])
            .await;
        if matches!(self.says(name).await, Standing::Running) {
            return Err(refused("it is still running"));
        }
        take(&at)?;
        self.reread().await;
        Ok(vec![at])
    }
}

/// A refusal in systemd's name.
fn refused(reason: &str) -> Failure {
    Failure::Refused {
        manager: MANAGER,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{quoted, quoting, written, Host, Hosted, Standing, Systemd};
    use crate::ports::hosting::{Failure, Held, Placed, Program};
    use crate::ports::process::Output;
    use crate::ports::Runner;
    use lemonfiber_fixtures::support::Sequenced;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    fn spoke(status: i32, stdout: &str) -> Result<Output, crate::ports::process::Failure> {
        Ok(Output {
            status: Some(status),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
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
        let dir = crate::app::fixtures::scratch(name);
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn over(
        dir: &Path,
        answers: Vec<Result<Output, crate::ports::process::Failure>>,
    ) -> (Systemd, Arc<Sequenced>) {
        let runner = Sequenced::answering(answers);
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
                Ok(Output {
                    status: Some(1),
                    stdout: String::new(),
                    stderr: "Failed to enable unit: Unit file is masked".to_owned(),
                }),
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
}

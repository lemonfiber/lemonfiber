//! A launch agent in the operator's own login session.
//!
//! An agent rather than a daemon, which is the whole of what keeps this out of
//! root: agents live under the operator's home directory and are loaded into the
//! session they log into, so nothing here needs a privilege the operator does not
//! already have and nothing another user of the machine logs into inherits it.
//!
//! `launchctl` is asked which session that is rather than told, because the
//! domain a bootstrap goes into is named after the user id and this process is
//! the only thing that knows which user it is running as.
//!
//! The plist carries no `KeepAlive`. See the note on the module above.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed, Program, Standing};
use crate::ports::process::Output;
use crate::ports::Runner;

use super::{complaint, definition, put, take};

/// What this manager is called where its refusals are reported.
const MANAGER: &str = "launchd";

/// What `launchctl print` says about an agent it is running.
const RUNNING: &str = "state = running";

/// What it says about one it holds and is not running.
const HELD: &str = "state = ";

/// The line a plist writes its output paths under, and the one before the program.
const OUT: &str = "<key>StandardOutPath</key>";

/// A launch agent, written into a directory and loaded through `launchctl`.
pub struct Launchd {
    agents: PathBuf,
    runner: Arc<dyn Runner>,
}

impl Launchd {
    /// Agents kept in this directory, loaded by running programs through this runner.
    #[must_use]
    pub const fn over(agents: PathBuf, runner: Arc<dyn Runner>) -> Self {
        Self { agents, runner }
    }

    /// The label launchd knows one of lemonfiber's commands by.
    fn label(name: &str) -> String {
        format!("com.lemonfiber.{name}")
    }

    /// Where that label's definition is written.
    fn plist(&self, name: &str) -> PathBuf {
        self.agents.join(format!("{}.plist", Self::label(name)))
    }

    /// Run a program, or nothing where it could not be run at all.
    async fn spoke(&self, argv: &[&str]) -> Option<Output> {
        let argv: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        self.runner.run(&argv).await.ok()
    }

    /// Which login session this process belongs to, as launchd names domains.
    async fn session(&self) -> Option<String> {
        let output = self.spoke(&["id", "-u"]).await?;
        let uid = output.stdout.trim().to_owned();
        (output.succeeded() && !uid.is_empty()).then_some(uid)
    }

    /// What launchd says about a label it may or may not be running.
    async fn says(&self, name: &str) -> Standing {
        let Some(uid) = self.session().await else {
            return Standing::Unsaid;
        };
        let target = format!("gui/{uid}/{}", Self::label(name));
        let Some(output) = self.spoke(&["launchctl", "print", &target]).await else {
            return Standing::Unsaid;
        };
        if !output.succeeded() {
            // launchctl answers about the agents it holds. A definition written
            // here that it will not answer about is one it has not loaded, which
            // is installed and not running rather than not installed.
            return Standing::Stopped;
        }
        if output.stdout.contains(RUNNING) {
            Standing::Running
        } else if output.stdout.contains(HELD) {
            Standing::Stopped
        } else {
            Standing::Unsaid
        }
    }

    /// Unload a label, whatever it was doing, ignoring what came back.
    async fn unload(&self, name: &str) {
        if let Some(uid) = self.session().await {
            let target = format!("gui/{uid}/{}", Self::label(name));
            let _ = self.spoke(&["launchctl", "bootout", &target]).await;
        }
    }
}

/// The plist launchd reads, one value to a line so it can be read back.
///
/// No document-type header. The property-list reader identifies the format by its
/// root element and has never fetched the schema that header names, so carrying it
/// would put a host this program never reaches into the list of hosts it names, and
/// three words no operator should have to meet into a string one could read. Two
/// separate sweeps said so, which is two more than the header is worth.
fn written(label: &str, hosted: &Hosted) -> String {
    let out = escaped(&hosted.output.to_string_lossy());
    let arguments: String = std::iter::once(hosted.program.to_string_lossy().into_owned())
        .chain(hosted.arguments.iter().cloned())
        .map(|word| format!("<string>{}</string>\n", escaped(&word)))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         <key>Label</key>\n\
         <string>{label}</string>\n\
         <key>ProgramArguments</key>\n\
         <array>\n\
         {arguments}\
         </array>\n\
         <key>RunAtLoad</key>\n\
         <true/>\n\
         {OUT}\n\
         <string>{out}</string>\n\
         <key>StandardErrorPath</key>\n\
         <string>{out}</string>\n\
         </dict>\n\
         </plist>\n"
    )
}

/// The three characters a plist cannot carry raw.
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The same three, back again. The ampersand goes last, so an escaped `&lt;`
/// written into a path does not come back out as a bracket.
fn plain(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The text inside a `<string>` written on one line.
fn inside(line: &str) -> Option<String> {
    line.trim()
        .strip_prefix("<string>")
        .and_then(|rest| rest.strip_suffix("</string>"))
        .map(plain)
}

/// Every argument the plist runs, program first.
fn arguments(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|line| line.trim() != "<array>")
        .skip(1)
        .take_while(|line| line.trim() != "</array>")
        .filter_map(inside)
        .collect()
}

/// The string written on the line after a key.
fn under(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines().skip_while(|line| line.trim() != key);
    lines.next()?;
    inside(lines.next()?)
}

#[async_trait]
impl Host for Launchd {
    fn manager(&self) -> Manager {
        Manager::Launchd
    }

    async fn place(&self, hosted: &Hosted) -> Result<Placed, Failure> {
        let at = self.plist(&hosted.name);
        put(&at, &written(&Self::label(&hosted.name), hosted))?;
        let Some(uid) = self.session().await else {
            let _ = take(&at);
            return Err(refused("this login session could not be identified"));
        };
        // Anything already loaded under the name goes first, so installing twice
        // replaces rather than leaves two agents running the same command.
        self.unload(&hosted.name).await;
        let domain = format!("gui/{uid}");
        let path = at.to_string_lossy().into_owned();
        match self
            .spoke(&["launchctl", "bootstrap", &domain, &path])
            .await
        {
            Some(output) if output.succeeded() => Ok(Placed {
                definition: at,
                started: true,
            }),
            Some(output) => {
                let _ = take(&at);
                Err(refused(&complaint(&output)))
            }
            None => {
                let _ = take(&at);
                Err(refused("launchctl could not be run"))
            }
        }
    }

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        let at = self.plist(name);
        let Some(text) = definition(&at) else {
            return Ok(Held::absent());
        };
        let arguments = arguments(&text);
        Ok(Held {
            standing: self.says(name).await,
            definition: Some(at),
            program: arguments.first().map(|at| Program {
                at: PathBuf::from(at),
                present: Path::new(at).exists(),
            }),
            runs: (!arguments.is_empty()).then(|| arguments.join(" ")),
            output: under(&text, OUT).map(PathBuf::from),
        })
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        let at = self.plist(name);
        if definition(&at).is_none() {
            return Ok(Vec::new());
        }
        self.unload(name).await;
        if matches!(self.says(name).await, Standing::Running) {
            return Err(refused("it is still running"));
        }
        take(&at)?;
        Ok(vec![at])
    }
}

/// A refusal in launchd's name.
fn refused(reason: &str) -> Failure {
    Failure::Refused {
        manager: MANAGER,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{arguments, escaped, inside, plain, under, Host, Hosted, Launchd, Standing, OUT};
    use crate::ports::hosting::{Failure, Held, Manager, Program};
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

    fn over(
        dir: &Path,
        answers: Vec<Result<Output, crate::ports::process::Failure>>,
    ) -> (Launchd, Arc<Sequenced>) {
        let runner = Sequenced::answering(answers);
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
            Ok(crate::ports::hosting::Placed {
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
                Ok(Output {
                    status: Some(5),
                    stdout: String::new(),
                    stderr: "Load failed: 5: Input/output error".to_owned(),
                }),
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
}

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

use lemonfiber_ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed, Program, Standing};
use lemonfiber_ports::process::Output;
use lemonfiber_ports::Runner;

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
        placed(self, hosted).await
    }

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        standing_of(self, name).await
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        withdrawn(self, name).await
    }
}

/// Write the unit, enable and start it, and take it away again if that refuses.
async fn placed(systemd: &Systemd, hosted: &Hosted) -> Result<Placed, Failure> {
    let at = systemd.at(&hosted.name);
    put(&at, &written(hosted))?;
    systemd.reread().await;
    let unit = Systemd::unit(&hosted.name);
    // Enabling and starting in one act, so a name that was already installed
    // is replaced rather than joined by a second under a different unit.
    let told = systemd
        .spoke(&["systemctl", "--user", "enable", "--now", &unit])
        .await;
    match told {
        Some(output) if output.succeeded() => Ok(Placed {
            definition: at,
            started: true,
        }),
        Some(output) => {
            let _ = take(&at);
            systemd.reread().await;
            Err(refused(&complaint(&output)))
        }
        None => {
            let _ = take(&at);
            Err(refused("systemctl could not be run"))
        }
    }
}

/// What the unit on disk says, and what systemd says about it.
async fn standing_of(systemd: &Systemd, name: &str) -> Result<Held, Failure> {
    let at = systemd.at(name);
    let Some(text) = definition(&at) else {
        return Ok(Held::absent());
    };
    let arguments = after(&text, RUNS)
        .map(|line| quoted(&line))
        .unwrap_or_default();
    Ok(Held {
        standing: systemd.says(name).await,
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

/// Disable and stop it and remove the unit, refusing while it is still running.
async fn withdrawn(systemd: &Systemd, name: &str) -> Result<Vec<PathBuf>, Failure> {
    let at = systemd.at(name);
    if definition(&at).is_none() {
        return Ok(Vec::new());
    }
    let unit = Systemd::unit(name);
    let _ = systemd
        .spoke(&["systemctl", "--user", "disable", "--now", &unit])
        .await;
    if matches!(systemd.says(name).await, Standing::Running) {
        return Err(refused("it is still running"));
    }
    take(&at)?;
    systemd.reread().await;
    Ok(vec![at])
}

/// A refusal in systemd's name.
fn refused(reason: &str) -> Failure {
    Failure::Refused {
        manager: MANAGER,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests;

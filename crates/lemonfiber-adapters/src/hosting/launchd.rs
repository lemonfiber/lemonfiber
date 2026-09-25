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

use lemonfiber_ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed, Program, Standing};
use lemonfiber_ports::process::Output;
use lemonfiber_ports::Runner;

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
        .map(|word| {
            let mut line = String::from("<string>");
            line.push_str(&escaped(&word));
            line.push_str("</string>\n");
            line
        })
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
        placed(self, hosted).await
    }

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        standing_of(self, name).await
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        withdrawn(self, name).await
    }
}

/// Write the definition, load it, and take it away again if the load refuses.
async fn placed(launchd: &Launchd, hosted: &Hosted) -> Result<Placed, Failure> {
    let at = launchd.plist(&hosted.name);
    put(&at, &written(&Launchd::label(&hosted.name), hosted))?;
    let Some(uid) = launchd.session().await else {
        let _ = take(&at);
        return Err(refused("this login session could not be identified"));
    };
    // Anything already loaded under the name goes first, so installing twice
    // replaces rather than leaves two agents running the same command.
    launchd.unload(&hosted.name).await;
    let domain = format!("gui/{uid}");
    let path = at.to_string_lossy().into_owned();
    match launchd
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

/// What the definition on disk says, and what launchd says about it.
async fn standing_of(launchd: &Launchd, name: &str) -> Result<Held, Failure> {
    let at = launchd.plist(name);
    let Some(text) = definition(&at) else {
        return Ok(Held::absent());
    };
    let arguments = arguments(&text);
    Ok(Held {
        standing: launchd.says(name).await,
        definition: Some(at),
        program: arguments.first().map(|at| Program {
            at: PathBuf::from(at),
            present: Path::new(at).exists(),
        }),
        runs: (!arguments.is_empty()).then(|| arguments.join(" ")),
        output: under(&text, OUT).map(PathBuf::from),
    })
}

/// Unload it and remove the definition, refusing while it is still running.
async fn withdrawn(launchd: &Launchd, name: &str) -> Result<Vec<PathBuf>, Failure> {
    let at = launchd.plist(name);
    if definition(&at).is_none() {
        return Ok(Vec::new());
    }
    launchd.unload(name).await;
    if matches!(launchd.says(name).await, Standing::Running) {
        return Err(refused("it is still running"));
    }
    take(&at)?;
    Ok(vec![at])
}

/// A refusal in launchd's name.
fn refused(reason: &str) -> Failure {
    Failure::Refused {
        manager: MANAGER,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests;

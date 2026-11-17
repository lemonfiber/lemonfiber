//! A task in the operator's own Task Scheduler.
//!
//! Registered from a definition naming the operator's own account, at the least
//! privilege it holds, so registering needs no administrative rights and nothing
//! another user of the machine logs into inherits it.
//!
//! The task runs a script written beside the others lemonfiber keeps, one command to
//! a line, so what it runs and where it writes are read back from the script rather
//! than from a listing whose words follow the machine's language.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use lemonfiber_ports::hosting::{
    Cadence, Failure, Held, Host, Hosted, Manager, Placed, Program, Standing,
};
use lemonfiber_ports::process::Output;
use lemonfiber_ports::Runner;

use super::{complaint, definition, put, take};

/// What this manager is called where its refusals are reported.
const MANAGER: &str = "Task Scheduler";

/// The program every request to the scheduler is made through.
const SCHTASKS: &str = "schtasks";

/// The program that names the account this process runs as.
const WHOAMI: &str = "whoami";

/// The field of a task's listing naming its status.
const STATUS: &str = "Status:";

/// The status of a task the scheduler is running.
const RUNNING: &str = "Running";

/// The status of a task it holds and is not running.
const READY: &str = "Ready";

/// The status of a task it holds and will not start.
const DISABLED: &str = "Disabled";

/// The line a script for a command started every minute carries.
const EVERY_MINUTE: &str = "rem every minute";

/// The words between what a script runs and the file it appends to.
const APPENDS: &str = " >> ";

/// The words after that file, sending the error channel there too.
const BOTH: &str = " 2>&1";

/// The namespace a task definition is written in.
const SCHEMA: &str = "http://schemas.microsoft.com/windows/2004/02/mit/task";

/// The moment a task started every minute counts its minutes from, long past.
const SINCE: &str = "2000-01-01T00:00:00";

/// The mark a UTF-16 file opens with, saying which order its bytes are in.
const BYTE_ORDER: u16 = 0xFEFF;

/// Why a definition a script cannot carry is not written.
const UNREPRESENTABLE: &str = "a value in it holds a double quote, a line break or another \
                               control character, which a task's script cannot carry";

/// A task registered through `schtasks`, running a script kept in a directory.
pub struct TaskScheduler {
    scripts: PathBuf,
    runner: Arc<dyn Runner>,
}

impl TaskScheduler {
    /// Scripts kept in this directory, registered by running programs through this runner.
    #[must_use]
    pub const fn over(scripts: PathBuf, runner: Arc<dyn Runner>) -> Self {
        Self { scripts, runner }
    }

    /// The name the scheduler knows one of lemonfiber's commands by.
    fn task(name: &str) -> String {
        format!("lemonfiber-{name}")
    }

    /// Where that task's script is written.
    fn at(&self, name: &str) -> PathBuf {
        self.scripts.join(format!("{}.cmd", Self::task(name)))
    }

    /// Run `schtasks` with these arguments, or nothing where it could not be run at all.
    async fn spoke(&self, arguments: &[&str]) -> Option<Output> {
        let argv: Vec<String> = std::iter::once(SCHTASKS)
            .chain(arguments.iter().copied())
            .map(str::to_owned)
            .collect();
        self.runner.run(&argv).await.ok()
    }

    /// The account this process runs as, as the scheduler names accounts.
    async fn operator(&self) -> Option<String> {
        let argv = vec![WHOAMI.to_owned()];
        let output = self.runner.run(&argv).await.ok()?;
        let user = output.stdout.trim().to_owned();
        (output.succeeded() && !user.is_empty()).then_some(user)
    }

    /// What the scheduler says about a task, where `timed` is whether it is started on a
    /// clock, which the scheduler holding it ready is all of.
    async fn says(&self, name: &str, timed: bool) -> Standing {
        let task = Self::task(name);
        let Some(output) = self.spoke(&["/Query", "/TN", &task, "/FO", "LIST"]).await else {
            return Standing::Unsaid;
        };
        if !output.succeeded() {
            return Standing::Stopped;
        }
        let status = output
            .stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix(STATUS))
            .map(str::trim);
        match status {
            Some(RUNNING) => Standing::Running,
            Some(READY) if timed => Standing::Running,
            Some(READY | DISABLED) => Standing::Stopped,
            _ => Standing::Unsaid,
        }
    }
}

/// The script the task runs, one value to a line so it can be read back.
fn written(hosted: &Hosted) -> String {
    let runs: Vec<String> = std::iter::once(hosted.program.to_string_lossy().into_owned())
        .chain(hosted.arguments.iter().cloned())
        .map(|word| quoting(&word))
        .collect();
    let timed = match hosted.cadence {
        Cadence::Kept => String::new(),
        Cadence::Minutely => format!("{EVERY_MINUTE}\r\n"),
    };
    format!(
        "@echo off\r\n\
         rem lemonfiber: {about}\r\n\
         {timed}\
         {runs}{APPENDS}{out}{BOTH}\r\n",
        about = escaped(&hosted.about),
        runs = runs.join(" "),
        out = quoting(&hosted.output.to_string_lossy()),
    )
}

/// Whether every value the script would carry can be carried by one quoted word of it.
///
/// A double quote ends the word it is inside, so whatever follows it is read by the
/// command interpreter as words of its own; a line break ends the command.
fn representable(hosted: &Hosted) -> bool {
    std::iter::once(hosted.program.to_string_lossy())
        .chain(std::iter::once(hosted.output.to_string_lossy()))
        .chain(hosted.arguments.iter().map(|word| word.as_str().into()))
        .chain(std::iter::once(hosted.about.as_str().into()))
        .all(|value| {
            !value
                .chars()
                .any(|letter| letter == '"' || letter.is_control())
        })
}

/// A value with the sign the interpreter expands variables between doubled.
fn escaped(value: &str) -> String {
    value.replace('%', "%%")
}

/// One word as the interpreter reads it: quoted, with that sign doubled.
fn quoting(word: &str) -> String {
    format!("\"{}\"", escaped(word))
}

/// The quoted words of a line, with that sign single again.
fn quoted(line: &str) -> Vec<String> {
    line.split('"')
        .skip(1)
        .step_by(2)
        .map(|word| word.replace("%%", "%"))
        .collect()
}

/// The command a script runs and the file it appends to, read back out of it.
fn read_back(text: &str) -> (Vec<String>, Option<PathBuf>) {
    let Some((runs, out)) = text
        .lines()
        .find(|line| line.starts_with('"'))
        .and_then(|line| line.split_once(APPENDS))
    else {
        return (Vec::new(), None);
    };
    (
        quoted(runs),
        quoted(out).into_iter().next().map(PathBuf::from),
    )
}

#[async_trait]
impl Host for TaskScheduler {
    fn manager(&self) -> Manager {
        Manager::TaskScheduler
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

/// Write the script, register the task and start it, and take the script away again if
/// the scheduler refuses it.
async fn placed(scheduler: &TaskScheduler, hosted: &Hosted) -> Result<Placed, Failure> {
    let at = scheduler.at(&hosted.name);
    if !representable(hosted) {
        return Err(Failure::Unwritable {
            at,
            reason: UNREPRESENTABLE.to_owned(),
        });
    }
    let Some(user) = scheduler.operator().await else {
        return Err(refused("the account this runs as could not be identified"));
    };
    put(&at, &written(hosted))?;
    let task = TaskScheduler::task(&hosted.name);
    let xml = at.with_extension("xml");
    if let Err(failure) = put_wide(&xml, &defined(hosted, &user, &at)) {
        let _ = take(&at);
        return Err(failure);
    }
    let told = scheduler
        .spoke(&[
            "/Create",
            "/F",
            "/TN",
            &task,
            "/XML",
            &xml.to_string_lossy(),
        ])
        .await;
    let _ = take(&xml);
    match told {
        Some(output) if output.succeeded() => {
            let started = scheduler
                .spoke(&["/Run", "/TN", &task])
                .await
                .is_some_and(|output| output.succeeded());
            Ok(Placed {
                definition: at,
                started,
            })
        }
        Some(output) => {
            let _ = take(&at);
            Err(refused(&complaint(&output)))
        }
        None => {
            let _ = take(&at);
            Err(refused("schtasks could not be run"))
        }
    }
}

/// The task the scheduler registers: the script run as `user`, at each of their logons or
/// every minute, with no privilege beyond theirs and never two runs at once.
fn defined(hosted: &Hosted, user: &str, script: &Path) -> String {
    let user = xml_escaped(user);
    let trigger = match hosted.cadence {
        Cadence::Kept => {
            format!("<LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger>")
        }
        Cadence::Minutely => format!(
            "<TimeTrigger><StartBoundary>{SINCE}</StartBoundary><Enabled>true</Enabled>\
             <Repetition><Interval>PT1M</Interval><StopAtDurationEnd>false</StopAtDurationEnd>\
             </Repetition></TimeTrigger>"
        ),
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n\
         <Task version=\"1.2\" xmlns=\"{SCHEMA}\">\r\n\
         <RegistrationInfo><Description>lemonfiber: {about}</Description></RegistrationInfo>\r\n\
         <Triggers>{trigger}</Triggers>\r\n\
         <Principals><Principal id=\"Author\"><UserId>{user}</UserId>\
         <LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel>\
         </Principal></Principals>\r\n\
         <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\
         <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\
         <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\
         <ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Enabled>true</Enabled></Settings>\r\n\
         <Actions Context=\"Author\"><Exec><Command>{command}</Command></Exec></Actions>\r\n\
         </Task>\r\n",
        about = xml_escaped(&hosted.about),
        command = xml_escaped(&script.to_string_lossy()),
    )
}

/// The five characters a task definition cannot carry raw.
fn xml_escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Write a definition the way the scheduler reads one: UTF-16, with its byte order mark.
fn put_wide(at: &Path, text: &str) -> Result<(), Failure> {
    let bytes: Vec<u8> = std::iter::once(BYTE_ORDER)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(at, bytes).map_err(|error| Failure::Unwritable {
        at: at.to_path_buf(),
        reason: error.to_string(),
    })
}

/// What the script on disk says, and what the scheduler says about its task.
async fn standing_of(scheduler: &TaskScheduler, name: &str) -> Result<Held, Failure> {
    let at = scheduler.at(name);
    let Some(text) = definition(&at) else {
        return Ok(Held::absent());
    };
    let timed = text.lines().any(|line| line.trim() == EVERY_MINUTE);
    let (arguments, output) = read_back(&text);
    Ok(Held {
        standing: scheduler.says(name, timed).await,
        definition: Some(at),
        program: arguments.first().map(|at| Program {
            at: PathBuf::from(at),
            present: Path::new(at).exists(),
        }),
        runs: (!arguments.is_empty()).then(|| arguments.join(" ")),
        output,
    })
}

/// End and delete the task and remove its script, keeping the script where the
/// scheduler would not delete the task.
async fn withdrawn(scheduler: &TaskScheduler, name: &str) -> Result<Vec<PathBuf>, Failure> {
    let at = scheduler.at(name);
    if definition(&at).is_none() {
        return Ok(Vec::new());
    }
    let task = TaskScheduler::task(name);
    let _ = scheduler.spoke(&["/End", "/TN", &task]).await;
    match scheduler.spoke(&["/Delete", "/TN", &task, "/F"]).await {
        Some(output) if output.succeeded() => {}
        Some(output) => return Err(refused(&complaint(&output))),
        None => return Err(refused("schtasks could not be run")),
    }
    take(&at)?;
    Ok(vec![at])
}

/// A refusal in the scheduler's name.
fn refused(reason: &str) -> Failure {
    Failure::Refused {
        manager: MANAGER,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests;

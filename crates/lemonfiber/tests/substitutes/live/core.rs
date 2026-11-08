//! lemonfiber, the build under test, run as an operator runs it: its own configuration and
//! data directories, every answer read as JSON.

use std::ffi::OsStr;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The keys an answer names the offer it is read with under.
const OFFERS: [&str; 2] = ["offer", "agreement"];

/// One lemonfiber, kept beneath a directory of its own.
pub struct Core {
    /// Where its configuration is kept.
    config: PathBuf,
    /// Where its data is kept.
    data: PathBuf,
}

/// What one run of it said.
#[derive(Debug)]
pub struct Ran {
    /// Whether it exited zero.
    pub succeeded: bool,
    /// What it printed, read as JSON where it is.
    pub said: serde_json::Value,
    /// What it printed on standard output and on standard error, as written.
    pub printed: String,
}

impl Core {
    /// A lemonfiber keeping everything beneath `root`.
    #[must_use]
    pub fn beneath(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            data: root.join("data"),
        }
    }

    /// Run it with `args` and `--json`, typing `typed` where it asks.
    #[must_use]
    pub fn run(&self, args: &[&str], typed: &str) -> Ran {
        let child = Command::new(env!("CARGO_BIN_EXE_lemonfiber"))
            .arg("--json")
            .args(self.dirs())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let Ok(mut child) = child else {
            return Ran {
                succeeded: false,
                said: serde_json::Value::Null,
                printed: format!("{args:?}: {child:?}"),
            };
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(typed.as_bytes());
        }
        let Ok(output) = child.wait_with_output() else {
            return Ran {
                succeeded: false,
                said: serde_json::Value::Null,
                printed: format!("{args:?}: no output"),
            };
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let said = serde_json::from_str(stdout.trim())
            .or_else(|_| last_document(&stdout))
            .unwrap_or(serde_json::Value::Null);
        let ran = Ran {
            succeeded: output.status.success(),
            said,
            printed: format!("$ lemonfiber {}\n{stdout}\n{stderr}", args.join(" ")),
        };
        println!("{}", ran.printed);
        ran
    }

    /// The arguments that keep a run of it beneath its own directories.
    #[must_use]
    pub fn dirs(&self) -> [&OsStr; 4] {
        [
            OsStr::new("--config-dir"),
            self.config.as_os_str(),
            OsStr::new("--data-dir"),
            self.data.as_os_str(),
        ]
    }

    /// Run `args`, and where it names an offer, run it again answering that offer.
    #[must_use]
    pub fn agreed(&self, args: &[&str]) -> Ran {
        let read = self.run(args, "");
        let Some(offer) = offer_in(&read.said) else {
            return read;
        };
        let mut answered: Vec<&str> = args.to_vec();
        answered.extend(["--offer", offer.as_str()]);
        self.run(&answered, "")
    }
}

/// The last JSON document of `printed`, one per line, where a run streams them.
fn last_document(printed: &str) -> Result<serde_json::Value, serde_json::Error> {
    printed
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.trim()).ok())
        .map_or_else(|| serde_json::from_str("null"), Ok)
}

/// The offer an answer names, wherever in it it is named.
#[must_use]
pub fn offer_in(said: &serde_json::Value) -> Option<String> {
    match said {
        serde_json::Value::Object(fields) => OFFERS
            .iter()
            .find_map(|key| fields.get(*key).and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .or_else(|| fields.values().find_map(offer_in)),
        serde_json::Value::Array(items) => items.iter().find_map(offer_in),
        _ => None,
    }
}

/// Every string in `said` under a key named `key`, wherever it is.
#[must_use]
pub fn strings_under(said: &serde_json::Value, key: &str) -> Vec<String> {
    match said {
        serde_json::Value::Object(fields) => fields
            .iter()
            .flat_map(|(name, value)| {
                let here = (name == key)
                    .then(|| value.as_str().map(str::to_owned))
                    .flatten();
                here.into_iter().chain(strings_under(value, key))
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|one| strings_under(one, key))
            .collect(),
        _ => Vec::new(),
    }
}

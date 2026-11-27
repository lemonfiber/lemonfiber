//! lemonfiber, the build under test, run as an operator runs it: its own configuration and
//! data directories, every answer read as JSON.

use std::ffi::OsStr;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use lemonfiber_core::model::kind;

/// What a withheld value is printed as.
pub const WITHHELD: &str = "<withheld>";

/// Where an answer of each kind holds a secret, as a JSON pointer into the answer.
pub const SECRETS: [(&str, &str); 4] = [
    (kind::MINTED_KEY.as_str(), "/data/secret"),
    (kind::CREDENTIALS.as_str(), "/data/revealed/value"),
    (kind::INVITATION.as_str(), "/data/decline"),
    (kind::INVITATION.as_str(), "/data/join"),
];

/// The keys an answer names the offer it is read with under.
const OFFERS: [&str; 2] = ["offer", "agreement"];

/// One lemonfiber, kept beneath a directory of its own.
pub struct Core {
    /// Where its configuration is kept.
    config: PathBuf,
    /// Where its data is kept.
    data: PathBuf,
}

/// What one run of it said, debugged as it is printed.
pub struct Ran {
    /// Whether it exited zero.
    pub succeeded: bool,
    /// What it printed, read as JSON where it is: one document, or every document where
    /// it printed several.
    pub said: serde_json::Value,
    /// What it printed on standard output and on standard error, as written.
    pub printed: String,
}

impl std::fmt::Debug for Ran {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ran")
            .field("succeeded", &self.succeeded)
            .field("printed", &self.printed)
            .finish_non_exhaustive()
    }
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
        self.withholding(args, typed, &[])
    }

    /// As [`Self::run`], with each of `withheld` written as `<withheld>` wherever what
    /// was run and what it said are printed, as is every secret [`SECRETS`] finds in
    /// what it said.
    #[must_use]
    pub fn withholding(&self, args: &[&str], typed: &str, withheld: &[&str]) -> Ran {
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
                printed: shown(format!("{args:?}: {child:?}"), withheld),
            };
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(typed.as_bytes());
        }
        let Ok(output) = child.wait_with_output() else {
            return Ran {
                succeeded: false,
                said: serde_json::Value::Null,
                printed: shown(format!("{args:?}: no output"), withheld),
            };
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let said = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| documents(&stdout));
        let held = secrets_in(&said);
        let every: Vec<&str> = withheld
            .iter()
            .copied()
            .chain(held.iter().map(String::as_str))
            .collect();
        let ran = Ran {
            succeeded: output.status.success(),
            printed: shown(
                format!("$ lemonfiber {}\n{stdout}\n{stderr}", args.join(" ")),
                &every,
            ),
            said,
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

    /// Where it keeps everything, laid out as it lays it out.
    #[must_use]
    pub fn paths(&self) -> lemonfiber_core::config::paths::Paths {
        lemonfiber_core::config::paths::Paths::rooted(&self.config, &self.data)
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

/// `printed`, with each of `withheld` written as [`WITHHELD`], as it is and as JSON
/// writes it inside a string.
#[must_use]
pub fn shown(printed: String, withheld: &[&str]) -> String {
    withheld
        .iter()
        .filter(|secret| !secret.is_empty())
        .flat_map(|secret| [(*secret).to_owned(), escaped(secret)])
        .fold(printed, |printed, secret| {
            printed.replace(&secret, WITHHELD)
        })
}

/// `secret` as JSON writes it inside a string.
fn escaped(secret: &str) -> String {
    let quoted = serde_json::Value::from(secret).to_string();
    quoted
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(&quoted)
        .to_owned()
}

/// Every secret [`SECRETS`] finds in `said`, in each document where it holds several.
#[must_use]
pub fn secrets_in(said: &serde_json::Value) -> Vec<String> {
    match said {
        serde_json::Value::Array(documents) => documents.iter().flat_map(secrets_in).collect(),
        one => SECRETS
            .iter()
            .filter(|(of, _)| one.get("kind").and_then(serde_json::Value::as_str) == Some(*of))
            .filter_map(|(_, at)| one.pointer(at).and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect(),
    }
}

/// Every JSON document of `printed`, one per line, where a run streams them.
fn documents(printed: &str) -> serde_json::Value {
    serde_json::Value::Array(
        printed
            .lines()
            .filter_map(|line| serde_json::from_str(line.trim()).ok())
            .collect(),
    )
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

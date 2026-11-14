//! A git source, as the runner a plugin install drives git through meets it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::process::{Failure, Output, Runner};

use crate::support::{refused, spoke};

/// A git source that serves one manifest, every other program answered with nothing.
pub struct Remote {
    /// What `git ls-remote` answers, or how it fails.
    listed: Result<String, String>,
    /// How the fetch fails, where it does.
    fetch: Option<String>,
    /// What a checkout leaves as the plugin's manifest.
    manifest: String,
    /// The Compose verb refused once something has been removed, where one is.
    refusing: Option<&'static str>,
    /// How long every call takes, where it takes any time at all.
    delay: Option<Duration>,
    /// The ending of a source address git cannot find, where a test names one.
    missing: Option<&'static str>,
    /// Every git command, without the leading `git -c …`.
    asked: Mutex<Vec<Vec<String>>>,
    /// Every command, whole, git or not.
    ran: Mutex<Vec<Vec<String>>>,
    /// The environment each command was run with.
    environments: Mutex<Vec<Vec<(String, String)>>>,
    /// How many calls are being answered now.
    now: AtomicUsize,
    /// The most calls ever answered at once.
    most: AtomicUsize,
}

impl Remote {
    /// A source whose `ls-remote` answers `listed`, checking out `manifest`.
    #[must_use]
    pub fn listing(listed: &str, manifest: &str) -> Self {
        Self::answering(Ok(listed.to_owned()), manifest)
    }

    /// A source whose `ls-remote` fails, saying `why`.
    #[must_use]
    pub fn unlisted(why: &str, manifest: &str) -> Self {
        Self::answering(Err(why.to_owned()), manifest)
    }

    /// The same source, failing the fetch with `why`.
    #[must_use]
    pub fn unfetched(mut self, why: &str) -> Self {
        self.fetch = Some(why.to_owned());
        self
    }

    /// The same source, with Compose refusing `verb` once something has been removed.
    #[must_use]
    pub fn refusing_after_removal(mut self, verb: Option<&'static str>) -> Self {
        self.refusing = verb;
        self
    }

    /// The same source, taking `delay` over every call and not finding a source whose
    /// address ends in `missing`.
    #[must_use]
    pub fn slow(mut self, delay: Duration, missing: &'static str) -> Self {
        self.delay = Some(delay);
        self.missing = Some(missing);
        self
    }

    /// Every git command it was asked, without `git` and its settings.
    #[must_use]
    pub fn asked(&self) -> Vec<Vec<String>> {
        read(&self.asked)
    }

    /// Every command it was run with, whole, git or not.
    #[must_use]
    pub fn ran(&self) -> Vec<Vec<String>> {
        read(&self.ran)
    }

    /// The environment each command was run with.
    #[must_use]
    pub fn environments(&self) -> Vec<Vec<(String, String)>> {
        read(&self.environments)
    }

    /// The most calls it was ever answering at once.
    #[must_use]
    pub fn most(&self) -> usize {
        self.most.load(Ordering::SeqCst)
    }

    /// Every directory a commit was checked out into, in the order they were made.
    #[must_use]
    pub fn checkouts(&self) -> Vec<PathBuf> {
        self.asked()
            .iter()
            .filter_map(|one| match one.as_slice() {
                [init, quiet, at] if init == "init" && quiet == "--quiet" => Some(at.into()),
                _ => None,
            })
            .collect()
    }

    /// Whether a commit was checked out, and every directory it was checked out into is
    /// gone.
    #[must_use]
    pub fn left_nothing(&self) -> bool {
        let made = self.checkouts();
        !made.is_empty() && made.iter().all(|at| !at.exists())
    }

    fn answering(listed: Result<String, String>, manifest: &str) -> Self {
        Self {
            listed,
            fetch: None,
            manifest: manifest.to_owned(),
            refusing: None,
            delay: None,
            missing: None,
            asked: Mutex::new(Vec::new()),
            ran: Mutex::new(Vec::new()),
            environments: Mutex::new(Vec::new()),
            now: AtomicUsize::new(0),
            most: AtomicUsize::new(0),
        }
    }

    /// Whether Compose refuses this call, something having been removed before it.
    fn refuses(&self, argv: &[String]) -> bool {
        let removed = self
            .ran
            .lock()
            .is_ok_and(|ran| ran.iter().any(|one| one.iter().any(|word| word == "rm")));
        removed
            && self
                .refusing
                .is_some_and(|verb| argv.iter().any(|word| word == verb))
    }

    /// The call, taking as long as this source takes.
    async fn dawdled(&self) {
        let Some(delay) = self.delay else { return };
        let now = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(delay).await;
        self.now.fetch_sub(1, Ordering::SeqCst);
    }

    /// What git says to `rest`, as a source would.
    fn said(&self, rest: &[String]) -> Output {
        if self
            .missing
            .is_some_and(|ending| rest.get(2).is_some_and(|url| url.ends_with(ending)))
        {
            return refused("fatal: repository not found");
        }
        let words: Vec<&str> = rest.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["ls-remote", ..] => match &self.listed {
                Ok(listed) => spoke(listed),
                Err(why) => refused(why),
            },
            ["init", "--quiet", at] => {
                let _ = std::fs::create_dir_all(at);
                spoke("")
            }
            ["-C", _, "fetch", ..] => self.fetch.as_deref().map_or_else(|| spoke(""), refused),
            ["-C", at, "checkout", ..] => {
                let _ = std::fs::write(Path::new(at).join("plugin.toml"), &self.manifest);
                spoke("")
            }
            _ => spoke(""),
        }
    }
}

#[async_trait]
impl Runner for Remote {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        let refuses = self.refuses(argv);
        crate::noted(&self.ran, argv.to_vec());
        self.dawdled().await;
        if refuses {
            return Ok(refused("the daemon said no"));
        }
        if argv.first().map(String::as_str) != Some("git") {
            return Ok(spoke(""));
        }
        let rest = asked_of(argv);
        crate::noted(&self.asked, rest.clone());
        Ok(self.said(&rest))
    }

    async fn run_with(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        crate::noted(&self.environments, env.to_vec());
        self.run(argv).await
    }
}

/// What git was asked, without `git` and the `-c` pairs every command carries.
#[must_use]
pub fn asked_of(argv: &[String]) -> Vec<String> {
    let mut rest = argv.get(1..).unwrap_or_default();
    while let [flag, _, after @ ..] = rest {
        if flag != "-c" {
            break;
        }
        rest = after;
    }
    rest.to_vec()
}

/// What a lock holds, or nothing where a test already failed holding it.
fn read<T: Clone>(held: &Mutex<Vec<T>>) -> Vec<T> {
    held.lock().map(|held| held.clone()).unwrap_or_default()
}

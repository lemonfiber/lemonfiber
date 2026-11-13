//! The runners a test scripts what a program answers through.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::process::{Failure, Output, Runner};

/// A runner that answers with whatever the test scripted.
pub struct Scripted(pub Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
        echoed(&self.0)
    }
}

/// Something a test has done when a call carrying a word arrives.
type Hook = Box<dyn Fn() + Send + Sync>;

/// A runner that answers the same way every time and remembers what it was asked.
///
/// [`Scripted`] throws the argument vector away, which is right until a test's claim
/// is about a command that should **not** have run.
pub struct Recording {
    answer: Result<Output, Failure>,
    seen: Mutex<Vec<Vec<String>>>,
    /// The environment each call was run with, empty for a call run without one.
    environments: Mutex<Vec<Vec<(String, String)>>>,
    /// How long each call takes, where it takes any time at all.
    delay: Option<Duration>,
    /// What runs when a call carrying each word arrives, before it is answered.
    hooks: Vec<(String, Hook)>,
}

impl Recording {
    /// A runner that always answers this way.
    #[must_use]
    pub fn answering(answer: Result<Output, Failure>) -> Self {
        Self {
            answer,
            seen: Mutex::new(Vec::new()),
            environments: Mutex::new(Vec::new()),
            delay: None,
            hooks: Vec::new(),
        }
    }

    /// The same runner, taking `delay` over every call.
    #[must_use]
    pub fn taking(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// The same runner, running `hook` whenever a call carrying `word` arrives.
    #[must_use]
    pub fn on(mut self, word: &str, hook: impl Fn() + Send + Sync + 'static) -> Self {
        self.hooks.push((word.to_owned(), Box::new(hook)));
        self
    }

    /// Whether any command it was handed used this Compose subcommand.
    ///
    /// A poisoned lock reads as nothing having been run, which fails a test asserting
    /// something happened rather than passing one asserting nothing did.
    #[must_use]
    pub fn ran(&self, subcommand: &str) -> bool {
        self.seen.lock().is_ok_and(|seen| {
            seen.iter()
                .any(|argv| argv.iter().any(|word| word == subcommand))
        })
    }

    /// Every argument vector it was handed, in the order it was handed them.
    #[must_use]
    pub fn seen(&self) -> Vec<Vec<String>> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// The environment each call was run with, in the order the calls came.
    #[must_use]
    pub fn environments(&self) -> Vec<Vec<(String, String)>> {
        self.environments
            .lock()
            .map(|environments| environments.clone())
            .unwrap_or_default()
    }

    /// One call, recorded with its environment and answered.
    async fn answered(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        crate::noted(&self.seen, argv.to_vec());
        crate::noted(&self.environments, env.to_vec());
        for (word, hook) in &self.hooks {
            if argv.iter().any(|said| said == word) {
                hook();
            }
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        echoed(&self.answer)
    }
}

#[async_trait]
impl Runner for Recording {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        self.answered(argv, &[]).await
    }

    async fn run_with(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        self.answered(argv, env).await
    }
}

/// The scripted answer again, since a [`Failure`] cannot be cloned.
fn echoed(answer: &Result<Output, Failure>) -> Result<Output, Failure> {
    match answer {
        Ok(output) => Ok(output.clone()),
        Err(Failure::NotFound { program }) => Err(Failure::NotFound {
            program: program.clone(),
        }),
        Err(Failure::Unusable { program, reason }) => Err(Failure::Unusable {
            program: program.clone(),
            reason: reason.clone(),
        }),
    }
}

/// A runner answering each call from a script, in the order the calls arrive.
///
/// A call the script did not cover is answered as a program that is not there.
pub struct Sequenced {
    answers: Mutex<VecDeque<Result<Output, Failure>>>,
    seen: Mutex<Vec<Vec<String>>>,
}

impl Sequenced {
    /// A runner that answers these, in this order.
    #[must_use]
    pub fn answering(answers: Vec<Result<Output, Failure>>) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(answers.into()),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Every argument vector it was handed, in order.
    #[must_use]
    pub fn seen(&self) -> Vec<Vec<String>> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// Whether any call it was handed contained this word.
    #[must_use]
    pub fn ran(&self, word: &str) -> bool {
        self.seen()
            .iter()
            .any(|argv| argv.iter().any(|said| said == word))
    }
}

#[async_trait]
impl Runner for Sequenced {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        crate::noted(&self.seen, argv.to_vec());
        self.answers
            .lock()
            .ok()
            .and_then(|mut answers| answers.pop_front())
            .unwrap_or_else(|| not_found(argv))
    }
}

/// How a [`Keyed`] runner tells which answer a call gets.
enum Matching {
    /// A call carrying every word of the key, anywhere in it.
    Words,
    /// A call whose leading words are the key's words, in order.
    Command,
    /// A call whose words, joined by spaces, contain the key.
    Spoken,
}

/// A runner answering by what a call says, and recording every call.
///
/// The first key that matches wins, so a test names the calls it is about and says
/// once what everything else answers.
pub struct Keyed {
    answers: Vec<(String, Result<Output, Failure>)>,
    /// What every other call answers, or a program that is not there where unsaid.
    otherwise: Option<Result<Output, Failure>>,
    matching: Matching,
    /// The word that, once a call carrying it has arrived, puts `then` ahead of the rest.
    after: Option<String>,
    /// The answers that take precedence once `after` has been seen.
    then: Vec<(String, Result<Output, Failure>)>,
    seen: Mutex<Vec<Vec<String>>>,
}

impl Keyed {
    /// A runner answering each call carrying every word of a key its own way, and
    /// everything else alike.
    #[must_use]
    pub fn answering(
        answers: Vec<(&str, Result<Output, Failure>)>,
        otherwise: Result<Output, Failure>,
    ) -> Arc<Self> {
        Arc::new(Self::built(answers, Some(otherwise), Matching::Words))
    }

    /// A runner answering each command whose leading words are a key its own way, and
    /// everything else as `otherwise` says or, where it says nothing, as a program that
    /// is not there.
    #[must_use]
    pub fn by_command(
        answers: Vec<(&str, Result<Output, Failure>)>,
        otherwise: Option<Result<Output, Failure>>,
    ) -> Arc<Self> {
        Arc::new(Self::built(answers, otherwise, Matching::Command))
    }

    /// A runner answering each call whose spoken form contains a key its own way, and
    /// everything else alike.
    #[must_use]
    pub fn by_spoken(
        answers: Vec<(&str, Result<Output, Failure>)>,
        otherwise: Result<Output, Failure>,
    ) -> Arc<Self> {
        Arc::new(Self::built(answers, Some(otherwise), Matching::Spoken))
    }

    /// A runner answering everything alike until a call carrying `after` arrives, and
    /// from then on answering each call carrying every word of a key in `then` its own
    /// way.
    #[must_use]
    pub fn once(
        after: &str,
        then: Vec<(&str, Result<Output, Failure>)>,
        otherwise: Result<Output, Failure>,
    ) -> Arc<Self> {
        let mut keyed = Self::built(Vec::new(), Some(otherwise), Matching::Words);
        keyed.after = Some(after.to_owned());
        keyed.then = owned(then);
        Arc::new(keyed)
    }

    /// Whether any call it was handed contained this word.
    #[must_use]
    pub fn ran(&self, word: &str) -> bool {
        self.seen
            .lock()
            .is_ok_and(|seen| seen.iter().any(|argv| argv.iter().any(|said| said == word)))
    }

    fn built(
        answers: Vec<(&str, Result<Output, Failure>)>,
        otherwise: Option<Result<Output, Failure>>,
        matching: Matching,
    ) -> Self {
        Self {
            answers: owned(answers),
            otherwise,
            matching,
            after: None,
            then: Vec::new(),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Whether `argv` is a call `key` names.
    fn names(&self, key: &str, argv: &[String]) -> bool {
        match self.matching {
            Matching::Words => key
                .split(' ')
                .all(|word| argv.iter().any(|said| said == word)),
            Matching::Command => {
                let words: Vec<&str> = key.split(' ').collect();
                argv.len() >= words.len() && words.iter().zip(argv).all(|(word, said)| said == word)
            }
            Matching::Spoken => argv.join(" ").contains(key),
        }
    }

    /// Whether a call carrying the word that switches on `then` has arrived.
    fn switched(&self) -> bool {
        self.after.as_ref().is_some_and(|after| {
            self.seen.lock().is_ok_and(|seen| {
                seen.iter()
                    .any(|argv| argv.iter().any(|said| said == after))
            })
        })
    }
}

#[async_trait]
impl Runner for Keyed {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        crate::noted(&self.seen, argv.to_vec());
        let then = if self.switched() {
            self.then.as_slice()
        } else {
            &[]
        };
        let answer = then
            .iter()
            .chain(&self.answers)
            .find(|(key, _)| self.names(key, argv))
            .map(|(_, answer)| answer)
            .or(self.otherwise.as_ref());
        answer.map_or_else(|| not_found(argv), echoed)
    }
}

/// Each key and its answer, owned.
fn owned(answers: Vec<(&str, Result<Output, Failure>)>) -> Vec<(String, Result<Output, Failure>)> {
    answers
        .into_iter()
        .map(|(key, answer)| (key.to_owned(), answer))
        .collect()
}

/// A program that is not there, named as the call named it.
fn not_found(argv: &[String]) -> Result<Output, Failure> {
    Err(Failure::NotFound {
        program: argv.first().cloned().unwrap_or_default(),
    })
}

/// A program that succeeded, saying `stdout`.
#[must_use]
pub fn spoke(stdout: &str) -> Output {
    Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// A program that failed, complaining `stderr`.
#[must_use]
pub fn refused(stderr: &str) -> Output {
    Output {
        status: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

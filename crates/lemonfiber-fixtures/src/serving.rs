//! An engine that serves an image as a test scripts it.

use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Serve, Served, Serving, Standing};

/// Serves every service the same way, and keeps what it was asked.
pub struct Serves {
    answer: Result<Served, Unran>,
    beside: Option<Result<Served, Unran>>,
    standing: Result<Standing, Unran>,
    log: String,
    writes: Vec<(String, String)>,
    asked: Mutex<Vec<Serve>>,
    removed: Mutex<Vec<Served>>,
}

impl Serves {
    /// Every service is served as `served`, and stands running with no health check.
    #[must_use]
    pub fn up(served: Served) -> Self {
        Self {
            answer: Ok(served),
            beside: None,
            standing: Ok(Standing::Running),
            log: String::new(),
            writes: Vec::new(),
            asked: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
        }
    }

    /// A container known as a test's, reached on the loopback interface at `port`.
    #[must_use]
    pub fn on(port: u16) -> Self {
        Self::up(Served {
            container: "4f1c2d3e5a6b7c8d9e0f".to_owned(),
            network: "lemonfiber-record-1-1-0".to_owned(),
            hostname: "4f1c2d3e5a6b".to_owned(),
            addresses: vec!["172.18.0.2".to_owned()],
            machine: vec!["recording-host".to_owned()],
            port: Some(port),
        })
    }

    /// No service can be served, for `unran`.
    #[must_use]
    pub fn refusing(unran: Unran) -> Self {
        Self {
            answer: Err(unran),
            ..Self::on(0)
        }
    }

    /// A container started beside another is served as `answer` instead.
    #[must_use]
    pub fn besides(self, answer: Result<Served, Unran>) -> Self {
        Self {
            beside: Some(answer),
            ..self
        }
    }

    /// Every container stands as `standing`.
    #[must_use]
    pub fn standing(self, standing: Standing) -> Self {
        Self {
            standing: Ok(standing),
            ..self
        }
    }

    /// The engine will not say how any container stands, for `unran`.
    #[must_use]
    pub fn unstanding(self, unran: Unran) -> Self {
        Self {
            standing: Err(unran),
            ..self
        }
    }

    /// Every container's log reads `log`.
    #[must_use]
    pub fn logging(self, log: &str) -> Self {
        Self {
            log: log.to_owned(),
            ..self
        }
    }

    /// Once served, every container writes `text` at `path` inside it, where a directory
    /// is mounted there.
    #[must_use]
    pub fn writing(mut self, path: &str, text: &str) -> Self {
        self.writes.push((path.to_owned(), text.to_owned()));
        self
    }

    /// Every service asked to be served, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Serve> {
        self.asked
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }

    /// Every container taken away, in order.
    #[must_use]
    pub fn removed(&self) -> Vec<Served> {
        self.removed
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }

    /// The one answer, with `serve` kept and what the container writes written.
    fn serving(&self, serve: &Serve) -> Result<Served, Unran> {
        let _ = self.asked.lock().map(|mut held| held.push(serve.clone()));
        if self.answer.is_ok() {
            for (path, text) in &self.writes {
                write_into(serve, path, text);
            }
        }
        self.answer.clone()
    }

    /// `served`, kept among those taken away.
    fn removing(&self, served: &Served) {
        let _ = self
            .removed
            .lock()
            .map(|mut held| held.push(served.clone()));
    }
}

/// Write `text` at `path` inside the container `serve` asks for, through the directory
/// mounted there.
fn write_into(serve: &Serve, path: &str, text: &str) {
    let landed = serve.binds.iter().find_map(|bound| {
        Path::new(path)
            .strip_prefix(&bound.target)
            .ok()
            .map(|inside| bound.source.join(inside))
    });
    let _ = landed.map(|at| {
        let _ = at.parent().map(std::fs::create_dir_all);
        std::fs::write(at, text)
    });
}

#[async_trait]
impl Serving for Serves {
    async fn serve(&self, serve: &Serve) -> Result<Served, Unran> {
        self.serving(serve)
    }

    async fn beside(&self, _served: &Served, serve: &Serve) -> Result<Served, Unran> {
        match &self.beside {
            Some(answer) => {
                let _ = self.asked.lock().map(|mut held| held.push(serve.clone()));
                answer.clone()
            }
            None => self.serving(serve),
        }
    }

    async fn standing(&self, _served: &Served) -> Result<Standing, Unran> {
        self.standing.clone()
    }

    async fn log(&self, _served: &Served, _lines: usize) -> String {
        self.log.clone()
    }

    async fn removed(&self, served: &Served) {
        self.removing(served);
    }
}

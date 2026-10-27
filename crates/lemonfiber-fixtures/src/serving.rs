//! An engine that serves an image as a test scripts it.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Answering, Serve, Served, Serving, StandIn, Standing};

/// Serves every service the same way, and keeps what it was asked.
pub struct Serves {
    answer: Result<Served, Unran>,
    beside: Option<Result<Served, Unran>>,
    standing: Result<Standing, Unran>,
    log: String,
    asked: Mutex<Vec<Serve>>,
    removed: Mutex<Vec<Served>>,
    stood_in: Mutex<Vec<StandIn>>,
    unanswering: Option<Unran>,
}

/// A stand-in that answers nothing, standing for one the engine would answer.
struct Answers;

impl Answering for Answers {}

impl Serves {
    /// Every service is served as `served`, and stands running with no health check.
    #[must_use]
    pub fn up(served: Served) -> Self {
        Self {
            answer: Ok(served),
            beside: None,
            standing: Ok(Standing::Running),
            log: String::new(),
            asked: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
            stood_in: Mutex::new(Vec::new()),
            unanswering: None,
        }
    }

    /// Keep `stand_in` as asked, and answer for it with nothing, or refuse it where this
    /// answers no stand-in.
    fn standing_in(&self, stand_in: &StandIn) -> Result<Box<dyn Answering>, Unran> {
        if let Ok(mut held) = self.stood_in.lock() {
            held.push(stand_in.clone());
        }
        match &self.unanswering {
            Some(unran) => Err(unran.clone()),
            None => Ok(Box::new(Answers)),
        }
    }

    /// No stand-in can answer, for `unran`.
    #[must_use]
    pub fn unanswering(self, unran: Unran) -> Self {
        Self {
            unanswering: Some(unran),
            ..self
        }
    }

    /// Every stand-in it was asked to answer, in the order asked.
    #[must_use]
    pub fn stood_in(&self) -> Vec<StandIn> {
        self.stood_in
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
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

    /// No service can be served, because there is no engine in a test.
    #[must_use]
    pub fn unreachable() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::refusing(Unran::Unreachable {
            reason: "no engine in a test".to_owned(),
        }))
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

    /// The one answer, with `serve` kept.
    fn serving(&self, serve: &Serve) -> Result<Served, Unran> {
        let _ = self.asked.lock().map(|mut held| held.push(serve.clone()));
        self.answer.clone()
    }

    /// The answer to a container started beside another, with `serve` kept.
    fn answered_beside(&self, serve: &Serve) -> Result<Served, Unran> {
        match &self.beside {
            Some(answer) => {
                let _ = self.asked.lock().map(|mut held| held.push(serve.clone()));
                answer.clone()
            }
            None => self.serving(serve),
        }
    }

    /// `served`, kept among those taken away.
    fn removing(&self, served: &Served) {
        let _ = self
            .removed
            .lock()
            .map(|mut held| held.push(served.clone()));
    }
}

#[async_trait]
impl Serving for Serves {
    async fn serve(&self, serve: &Serve) -> Result<Served, Unran> {
        self.serving(serve)
    }

    async fn beside(&self, _served: &Served, serve: &Serve) -> Result<Served, Unran> {
        self.answered_beside(serve)
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

    async fn stand_in(&self, stand_in: &StandIn) -> Result<Box<dyn Answering>, Unran> {
        self.standing_in(stand_in)
    }
}

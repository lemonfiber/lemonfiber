//! A media server minting and revoking app keys, answering which accounts are astray as a
//! test scripts it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use lemonfiber_ports::service::{AppKeys, Astray, Dated, Failure, OutsideAccount, OutsideServer};

/// How the server answers `astray` for one app.
#[derive(Clone)]
pub enum Astrayed {
    /// With these accounts.
    Holds(Vec<Astray>),
    /// It does not offer the question.
    Unsupported,
    /// It does not answer.
    Unavailable,
}

/// A media server answering `astray` per app as given, holding no keys, and recording
/// every call made of it.
pub struct Keys {
    answers: BTreeMap<&'static str, Astrayed>,
    calls: Mutex<Vec<String>>,
}

impl Keys {
    /// A server answering `astray` for each named app as given, and holding nothing astray
    /// for any other.
    #[must_use]
    pub fn answering(answers: &[(&'static str, Astrayed)]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers.iter().cloned().collect(),
            calls: Mutex::default(),
        })
    }

    /// Every call made of it so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn called(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }
}

#[async_trait]
impl AppKeys for Keys {
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        self.called(format!("filed_as {app}"));
        Ok(Vec::new())
    }

    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        self.called(format!("dated {app}"));
        Ok(Vec::new())
    }

    async fn mint(&self, app: &str) -> Result<String, Failure> {
        self.called(format!("mint {app}"));
        Ok(String::new())
    }

    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        self.called(format!("answers_to {key}"));
        Ok(())
    }

    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        self.called(format!("revoke {key}"));
        Ok(())
    }

    async fn account(&self, app: &str) -> Result<OutsideAccount, Failure> {
        Err(Failure::Unsupported {
            service: "media".to_owned(),
            detail: format!("no outside service holds {app}"),
        })
    }

    async fn listed_as(&self) -> Result<OutsideServer, Failure> {
        Err(Failure::Unsupported {
            service: "media".to_owned(),
            detail: "no outside service holds its accounts".to_owned(),
        })
    }

    async fn astray(&self, app: &str) -> Result<Vec<Astray>, Failure> {
        self.called(format!("astray {app}"));
        match self
            .answers
            .get(app)
            .cloned()
            .unwrap_or(Astrayed::Holds(Vec::new()))
        {
            Astrayed::Holds(held) => Ok(held),
            Astrayed::Unsupported => Err(Failure::Unsupported {
                service: "plex".to_owned(),
                detail: "astray is not offered".to_owned(),
            }),
            Astrayed::Unavailable => Err(Failure::Unavailable {
                service: "plex".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests;

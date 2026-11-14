//! A media server and a request service, set up for the household as a test scripts them.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Endpoint, Failure, FulfilmentTarget, HouseholdRequest, IdentitySource, MediaServer,
    MediaServerLink, RegisteredTarget, Requesting, Requests, Telling,
};

use crate::receiving::down;

/// What a request service that will not make an owner says.
pub const NO_OWNER: &str = "HTTP 409: no owner can be made";

/// How the media server's setup answers.
pub enum Startup {
    /// The wizard has already run.
    Completed,
    /// The wizard has not run.
    Fresh,
    /// Not answering.
    Down,
}

/// How the media server answers the account creation. A refusal stands for every
/// non-success; the not-answering path is exercised through the startup read.
pub enum Create {
    /// Taken.
    Ok,
    /// Refused.
    Rejects,
}

/// A media server whose setup answers as scripted.
pub struct MediaSetup {
    startup: Startup,
    create: Create,
}

impl MediaSetup {
    /// A media server whose wizard answers as `startup` and whose account creation as
    /// `create`.
    #[must_use]
    pub const fn new(startup: Startup, create: Create) -> Self {
        Self { startup, create }
    }
}

#[async_trait]
impl MediaServer for MediaSetup {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        match self.startup {
            Startup::Completed => Ok(true),
            Startup::Fresh => Ok(false),
            Startup::Down => Err(down("jellyfin")),
        }
    }

    async fn replace_admin_password(&self, _replacement: &str) -> Result<(), Failure> {
        Ok(())
    }

    async fn create_admin(&self, _name: &str, _password: &str) -> Result<(), Failure> {
        match self.create {
            Create::Ok => Ok(()),
            Create::Rejects => Err(Failure::Refused {
                service: "jellyfin".to_owned(),
                detail: "HTTP 400: user already exists".to_owned(),
            }),
        }
    }
}

/// How the request service answers a read of its initialised state.
pub enum Init {
    /// Not initialised.
    Fresh,
    /// Already initialised.
    Done,
    /// Not answering.
    Down,
}

/// How the request service answers the sign-in. A refusal stands for every non-success; the
/// not-answering path is exercised through the initialised read.
pub enum Configure {
    /// Taken.
    Ok,
    /// Refused.
    Rejects,
}

/// A request service whose setup answers as scripted.
pub struct RequestSetup {
    /// The state reported on the first read (the gate) and on the second (the
    /// read-back), so a fresh-then-confirmed sequence can be scripted.
    gate: Init,
    readback: Init,
    configure: Configure,
    establish: Configure,
    calls: Mutex<u32>,
    /// What the service is holding for what the household gets told, which `tell`
    /// overwrites so a test can read back what would have reached it.
    told: Mutex<Telling>,
}

impl RequestSetup {
    /// A request service reading as `gate` first and `readback` after, its identity
    /// configured as `configure`.
    #[must_use]
    pub fn new(gate: Init, readback: Init, configure: Configure) -> Self {
        Self {
            gate,
            readback,
            configure,
            establish: Configure::Ok,
            calls: Mutex::new(0),
            told: Mutex::new(Telling::default()),
        }
    }

    /// The same service, refusing to make an owner.
    #[must_use]
    pub fn refusing_an_owner(self) -> Self {
        Self {
            establish: Configure::Rejects,
            ..self
        }
    }

    fn reading(state: &Init) -> Result<bool, Failure> {
        match state {
            Init::Fresh => Ok(false),
            Init::Done => Ok(true),
            Init::Down => Err(down("seerr")),
        }
    }
}

#[async_trait]
impl Requests for RequestSetup {
    async fn establish(&self, _source: &IdentitySource) -> Result<(), Failure> {
        match self.establish {
            Configure::Ok => Ok(()),
            Configure::Rejects => Err(Failure::Refused {
                service: "seerr".to_owned(),
                detail: NO_OWNER.to_owned(),
            }),
        }
    }

    async fn initialized(&self) -> Result<bool, Failure> {
        let count = match self.calls.lock() {
            Ok(mut calls) => {
                *calls += 1;
                *calls
            }
            Err(_) => 0,
        };
        if count <= 1 {
            Self::reading(&self.gate)
        } else {
            Self::reading(&self.readback)
        }
    }

    async fn answers(&self) -> Result<(), Failure> {
        Ok(())
    }

    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure> {
        Ok(Vec::new())
    }

    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure> {
        Ok(Vec::new())
    }

    async fn add_fulfilment_target(&self, _target: &FulfilmentTarget) -> Result<(), Failure> {
        Ok(())
    }

    async fn move_fulfilment_target(
        &self,
        _held: &RegisteredTarget,
        _at: &Endpoint,
        _key: &str,
    ) -> Result<(), Failure> {
        Ok(())
    }

    async fn minted_as(&self) -> Result<String, Failure> {
        Ok("Seerr".to_owned())
    }

    async fn test_fulfilment_target(
        &self,
        _kind: Kind,
        _at: &Endpoint,
        _key: &str,
    ) -> Result<(), Failure> {
        Ok(())
    }

    async fn media_server_link(&self) -> Result<MediaServerLink, Failure> {
        Err(Failure::Unavailable {
            service: "seerr".to_owned(),
        })
    }

    async fn link_media_server(&self, _at: &Endpoint, _key: &str) -> Result<(), Failure> {
        Ok(())
    }

    async fn member_for(&self, _media_server_id: &str) -> Result<Option<String>, Failure> {
        Ok(None)
    }

    async fn requesting(&self, _media_server_id: &str) -> Result<Option<Requesting>, Failure> {
        Ok(None)
    }

    async fn approval_first(&self, _id: &str) -> Result<(), Failure> {
        Ok(())
    }

    async fn remove_member(&self, _id: &str) -> Result<(), Failure> {
        Ok(())
    }

    async fn link_members(&self, _members: &[String]) -> Result<(), Failure> {
        Ok(())
    }

    async fn telling(&self) -> Result<Telling, Failure> {
        match self.gate {
            Init::Down => Err(down("seerr")),
            Init::Fresh | Init::Done => Ok(self
                .told
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()),
        }
    }

    async fn tell(&self, telling: &Telling) -> Result<(), Failure> {
        match self.gate {
            Init::Down => Err(down("seerr")),
            Init::Fresh | Init::Done => {
                if let Ok(mut told) = self.told.lock() {
                    *told = telling.clone();
                }
                Ok(())
            }
        }
    }

    async fn configure_identity(&self, _source: &IdentitySource) -> Result<(), Failure> {
        match self.configure {
            Configure::Ok => Ok(()),
            Configure::Rejects => Err(Failure::Refused {
                service: "seerr".to_owned(),
                detail: "HTTP 500: credentials rejected".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests;

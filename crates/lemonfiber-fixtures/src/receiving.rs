//! A curator service the seed driver wires root folders and download clients into, as a
//! test scripts it.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Category, Client, ClientProbe, DownloadClient, Failure, Identity, QualityProfile,
    RegisteredClient, RegisteredFolder, RootFolder,
};

/// How the service answers — the script a test picks before it starts.
pub enum Mode {
    /// Answers normally; registering a folder adds it.
    Normal,
    /// Not answering — listing and registering both fail as unavailable.
    Down,
    /// Reachable but refuses the credential when listed.
    RefusesList,
    /// Reachable, lists fine, but rejects a registration with its own words.
    RejectsRegister,
    /// Accepts a registration but the folder never appears on read-back.
    Swallows,
    /// Lists fine to observe and registers fine, then stops answering on the
    /// read-back that would confirm the write.
    DropsAfterRegister,
    /// Reachable, but does not serve the API version this build speaks — the
    /// listing fails as unsupported, a conflict a re-run cannot lift.
    Unsupported,
    /// Reachable, but answers the listing as something it never does.
    Disallowed,
    /// Lists fine, but refuses an in-place update — the failure a reset's revert meets
    /// when the service will not take lemonfiber's category back.
    RefusesUpdate,
}

/// A service that answers the seed driver from a script.
pub struct Receiving {
    mode: Mode,
    folders: Mutex<Vec<RegisteredFolder>>,
    clients: Mutex<Vec<RegisteredClient>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
    /// The client-test verdicts to answer with, or `None` to fail the test call —
    /// the signal a drift-severity escalation reads. Absent by default, since only a
    /// drift asks for it.
    probes: Mutex<Option<Vec<ClientProbe>>>,
}

impl Receiving {
    /// A service answering in this mode, already holding these root folders.
    #[must_use]
    pub fn with(mode: Mode, folders: Vec<RegisteredFolder>) -> Self {
        Self {
            mode,
            folders: Mutex::new(folders),
            clients: Mutex::new(Vec::new()),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
            probes: Mutex::new(None),
        }
    }

    /// A service answering in this mode, already holding these download clients.
    #[must_use]
    pub fn with_clients(mode: Mode, clients: Vec<RegisteredClient>) -> Self {
        Self {
            mode,
            folders: Mutex::new(Vec::new()),
            clients: Mutex::new(clients),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
            probes: Mutex::new(None),
        }
    }

    /// The client-test verdicts this service answers with when its clients are
    /// tested — how a drift is driven to reachable or unreachable.
    #[must_use]
    pub fn probing(self, probes: Vec<ClientProbe>) -> Self {
        Self {
            probes: Mutex::new(Some(probes)),
            ..self
        }
    }

    /// The folders the service is holding — what a run registered and the service
    /// kept, so a next run can be given the state an interrupted one left behind.
    #[must_use]
    pub fn registered(&self) -> Vec<RegisteredFolder> {
        self.folders
            .lock()
            .map(|folders| folders.clone())
            .unwrap_or_default()
    }
}

/// The failure a service that is not answering produces, in its own words.
#[must_use]
pub fn down(service: &str) -> Failure {
    Failure::Unavailable {
        service: service.to_owned(),
    }
}

#[async_trait]
impl Client for Receiving {
    async fn identity(&self) -> Result<Identity, Failure> {
        Ok(Identity {
            name: "sonarr".to_owned(),
            version: "4".to_owned(),
        })
    }

    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: unknown implementation".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut clients), Ok(mut id)) = (self.clients.lock(), self.next_id.lock()) {
                    clients.push(RegisteredClient {
                        id: id.to_string(),
                        host: client.host.clone(),
                        port: client.port,
                        category: Some(client.category.clone()),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        if matches!(self.mode, Mode::Down) {
            return Err(down("sonarr"));
        }
        if matches!(self.mode, Mode::RefusesUpdate) {
            return Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: cannot update".to_owned(),
            });
        }
        if let Ok(mut clients) = self.clients.lock() {
            if let Some(existing) = clients.iter_mut().find(|held| held.id == id) {
                existing.category = Some(client.category.clone());
            }
        }
        Ok(())
    }

    async fn set_client_field(
        &self,
        id: &str,
        field: &str,
        value: Option<&str>,
    ) -> Result<(), Failure> {
        if matches!(self.mode, Mode::Down) {
            return Err(down("sonarr"));
        }
        if let Ok(mut clients) = self.clients.lock() {
            if let Some(existing) = clients.iter_mut().find(|held| held.id == id) {
                existing.category = value.map(|value| Category {
                    field: field.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
        Ok(())
    }

    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure> {
        self.probes
            .lock()
            .ok()
            .and_then(|probes| probes.clone())
            .map_or_else(|| Err(down("sonarr")), Ok)
    }

    async fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Failure> {
        Ok(vec![QualityProfile {
            id: 1,
            name: "Any".to_owned(),
        }])
    }

    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "sonarr".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("sonarr")),
            _ => Ok(self
                .clients
                .lock()
                .map(|clients| clients.clone())
                .unwrap_or_default()),
        }
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: path is already used".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut folders), Ok(mut id)) = (self.folders.lock(), self.next_id.lock()) {
                    folders.push(RegisteredFolder {
                        id: id.to_string(),
                        path: folder.path.trim_end_matches('/').to_owned(),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "sonarr".to_owned(),
            }),
            Mode::Unsupported => Err(Failure::Unsupported {
                service: "sonarr".to_owned(),
                detail: "there is no /api/v3".to_owned(),
            }),
            Mode::Disallowed => Err(Failure::Disallowed {
                service: "sonarr".to_owned(),
                detail: "never a listing".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("sonarr")),
            _ => Ok(self
                .folders
                .lock()
                .map(|folders| folders.clone())
                .unwrap_or_default()),
        }
    }
}

#[cfg(test)]
mod tests;

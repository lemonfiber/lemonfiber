//! The seed drivers run against the curator fake in `lemonfiber_fixtures::receiving`, and the
//! folders and clients they are asked to wire.

use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{
    Category, Credential, DownloadClient, Protocol, RegisteredClient, RootFolder,
};
use lemonfiber_core::seed::{wire_root_folders, Backing, Placing, State};
pub use lemonfiber_fixtures::receiving::{Mode, Receiving};
use std::collections::BTreeMap;

/// A root folder to wire, at the given path.
pub fn folder(path: &str) -> RootFolder {
    RootFolder {
        path: path.to_owned(),
        media_type: "tv".to_owned(),
    }
}

/// Run the driver for one wanted folder, returning its resulting state and the
/// number of changes journalled. No folder is contested by another curator.
pub async fn seed(service: Receiving, wanted: &[RootFolder]) -> (Vec<State>, usize) {
    seed_contested(service, wanted, &BTreeMap::new()).await
}

/// Run the driver with a set of contested root-folder paths, so a folder another
/// curator also claims is refused rather than wired.
pub async fn seed_contested(
    service: Receiving,
    wanted: &[RootFolder],
    contested: &BTreeMap<String, Vec<String>>,
) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    // Every folder a test wants is under `/data`, the mounted data root, so none is
    // refused for falling outside it unless the test deliberately reaches beyond.
    let wirings = wire_root_folders(
        &service,
        "sonarr",
        wanted,
        Placing {
            contested,
            root: "/data",
            backing: None,
        },
        &mut journal,
        "t",
        false,
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// Drive the folder wiring as a rehearsal: the same pass over the same service, with
/// the registering left out.
pub async fn would_wire(service: &Receiving, wanted: &[RootFolder]) -> Vec<State> {
    wire_with(service, wanted, None, true).await
}

/// Drive the folder wiring against a borrowed service, so one fake can be carried
/// across two passes — standing in for the state a killed run leaves in the real
/// service between one run and the next. A fresh journal each pass, as production
/// keeps none across passes.
pub async fn wire_on(service: &Receiving, wanted: &[RootFolder]) -> Vec<State> {
    wire_with(service, wanted, None, false).await
}

/// Drive the uncontested folder wiring against a borrowed service, making each
/// folder's directory where `backing` says the data root is.
pub async fn wire_with(
    service: &Receiving,
    wanted: &[RootFolder],
    backing: Option<Backing<'_>>,
    rehearsing: bool,
) -> Vec<State> {
    let mut journal = Journal::new();
    wire_root_folders(
        service,
        "sonarr",
        wanted,
        Placing {
            contested: &BTreeMap::new(),
            root: "/data",
            backing,
        },
        &mut journal,
        "t",
        rehearsing,
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect()
}

/// A wanted download client, filed under the ordinary category.
pub fn client(name: &str, host: &str, port: u16) -> DownloadClient {
    client_with_category(name, host, port, "tv")
}

/// A wanted client whose category lemonfiber intends to file under `category` —
/// for the drift tests, where lemonfiber's desired value is the thing that moves.
pub fn client_with_category(name: &str, host: &str, port: u16, category: &str) -> DownloadClient {
    DownloadClient {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        protocol: Protocol("sabnzbd".to_owned()),
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        },
    }
}

/// A client the service holds under a category, for the wholesale-drift checks.
pub fn holding(id: &str, host: &str, port: u16, category: &str) -> RegisteredClient {
    RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        }),
    }
}

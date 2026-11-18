//! Taking back a download client lemonfiber created in a curator once nothing it points
//! at fills the curator's download ask.
//!
//! The seed keeps each client it creates as a field of its baseline, by the id the curator
//! gave it, with the name it gave it and the host and port it was made for. Only a client
//! kept that way is ever taken back, and only while the curator still holds it where it
//! was made for: a client the operator added, adopted or pointed somewhere else is theirs.

use serde::{Deserialize, Serialize};

use super::{unreached, Baseline, Client, DownloadClient, State, Wiring, CLIENT};
use crate::instant::Instant;
use crate::ports::service::Failure;

/// A download client the seed created: the name it gave it and where it pointed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Made {
    /// The name the curator shows it under.
    name: String,
    /// The host it was made to reach.
    host: String,
    /// The port it was made to reach.
    port: u16,
}

/// What taking back from one curator came to: how each client went, and the field of
/// each client the seed no longer keeps.
#[derive(Debug, Default)]
pub struct TakenBack {
    /// One entry for each client taken back, or that would be or could not be.
    pub wirings: Vec<Wiring>,
    /// The baseline fields to let go.
    pub let_go: Vec<String>,
}

/// The baseline field keeping the client the curator gave `id`.
fn field(id: &str) -> String {
    format!("{}{id}", prefix())
}

/// What every field keeping a created client starts with.
fn prefix() -> String {
    format!("made-{CLIENT}:")
}

/// Keep `want` as a client the seed created in `service` under `id`.
pub fn kept(records: &mut Baseline, service: &str, id: &str, want: &DownloadClient, at: Instant) {
    let made = Made {
        name: want.name.clone(),
        host: want.host.clone(),
        port: want.port,
    };
    if let Ok(value) = serde_json::to_string(&made) {
        records.record(service, &field(id), &value, at);
    }
}

/// Every client the seed kept as created in `service`, by the id the curator gave it.
fn made_in(expected: &Baseline, service: &str) -> Vec<(String, Made)> {
    expected
        .named(service, &prefix())
        .into_iter()
        .filter_map(|id| {
            let value = expected.expected(service, &field(&id))?;
            let made = serde_json::from_str(value).ok()?;
            Some((id, made))
        })
        .collect()
}

/// Take back from `service` each client the seed created there that the curator still
/// holds where it was made for, where no service in `filled` answers.
///
/// A curator that does not answer its list has nothing taken back. A client it no longer
/// holds, or holds pointed somewhere else, is let go of without asking anything.
pub async fn take_back_clients(
    client: &dyn Client,
    service: &str,
    expected: &Baseline,
    filled: &[(String, u16)],
    rehearsing: bool,
) -> TakenBack {
    let mut taken = TakenBack::default();
    let made = made_in(expected, service);
    if made.is_empty() {
        return taken;
    }
    let Ok(held) = client.download_clients().await else {
        return taken;
    };
    for (id, made) in made {
        let still = held
            .iter()
            .any(|one| one.id == id && one.host == made.host && one.port == made.port);
        if !still {
            taken.let_go.push(field(&id));
            continue;
        }
        if filled
            .iter()
            .any(|(host, port)| *host == made.host && *port == made.port)
        {
            continue;
        }
        let connection = format!("{} into {service}", made.name);
        if rehearsing {
            taken
                .wirings
                .push(Wiring::settled(connection, State::WouldTakeBack));
            continue;
        }
        match client.forget_download_client(&id).await {
            Ok(()) => {
                taken
                    .wirings
                    .push(Wiring::settled(connection, State::TakenBack));
                taken.let_go.push(field(&id));
            }
            Err(Failure::Unsupported { .. }) => {
                taken
                    .wirings
                    .push(kept_behind(connection, &made.name, service));
            }
            Err(failure) => taken
                .wirings
                .push(Wiring::settled(connection, unreached(&failure))),
        }
    }
    taken
}

/// A client a curator cannot take back, left in it and said so.
fn kept_behind(connection: String, name: &str, service: &str) -> Wiring {
    let mut wiring = Wiring::settled(
        connection,
        State::Skipped {
            reason: format!("{service} cannot take a download client back"),
        },
    );
    wiring.escalate(
        format!("{service} still holds {name}, which nothing here answers as any more"),
        format!("Remove {name} from {service}'s download clients"),
    );
    wiring
}

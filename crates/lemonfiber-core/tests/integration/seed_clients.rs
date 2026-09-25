//! Wiring download clients into a \\*arr, against a fake service.
//!
//! The same driver as the root folders, with the difference that is the whole
//! point of it: a client is matched by the endpoint it reaches, not by its label,
//! so one the operator renamed is recognised rather than duplicated.

use common::service::*;

use crate::common;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, DownloadClient, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, State};

// ---- Download clients: the same driver, matched by endpoint not label. ----

/// Run the client driver for the wanted clients, returning their resulting states
/// and the number of changes journalled. The baseline it records into is discarded
/// — the tests that assert on it drive the driver directly.
async fn seed_clients(service: FakeService, wanted: &[DownloadClient]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
            rehearsing: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// Run the client driver, returning the resulting states and the baseline it
/// recorded into — for the tests that assert what lemonfiber remembered it wrote.
/// The expected snapshot is empty, as on a first seed.
async fn seed_clients_recording(
    service: FakeService,
    wanted: &[DownloadClient],
) -> (Vec<State>, Baseline) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
            rehearsing: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records)
}

/// Run the client driver against a given baseline and pass kind, returning the
/// resulting states, what this run recorded, and the number of changes journalled —
/// for the adoption tests, which turn on the baseline's origin and the adopt flag.
async fn seed_clients_with(
    service: FakeService,
    wanted: &[DownloadClient],
    expected: &Baseline,
    adopt: bool,
) -> (Vec<State>, Baseline, usize) {
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected,
            records: &mut records,
            adopt,
            reset: false,
            rehearsing: false,
        },
        "2",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records, journal.changes().len())
}

fn a_pre_existing_client() -> Vec<RegisteredClient> {
    vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }]
}

mod adopting;
mod baseline;
mod drift;
mod failures;
mod intent;
mod registering;
mod resetting;

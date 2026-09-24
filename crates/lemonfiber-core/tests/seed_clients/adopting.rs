//! Taking on a value that was already there.

use crate::common::service::*;
use crate::{a_pre_existing_client, seed_clients_with};
use lemonfiber_core::baseline::{Baseline, Origin, Record};
use lemonfiber_core::ports::service::{Category, RegisteredClient};
use lemonfiber_core::seed::State;

#[tokio::test]
async fn an_adopt_pass_takes_on_a_pre_existing_unmanaged_value() {
    // The deliberate act: adopting an existing setup baselines from what is found.
    // The same pre-existing value, run through an adopt pass, is taken on — recorded
    // as the operator's own, marked adopted, with nothing written to the service.
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, a_pre_existing_client()),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting what is found writes nothing to the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "their-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's own value is adopted as the baseline",
    );
}

#[tokio::test]
async fn an_adopted_value_lemonfiber_also_wants_stays_adopted_not_re_recorded() {
    // The case the origin exists to guard: an adopted value that happens to equal what
    // lemonfiber would write must stay adopted, not be read as merely in-sync and taken
    // back as lemonfiber's own. It reads as adopted, and this run records nothing over
    // it — so the adopted baseline is not clobbered with a written one, which a later
    // run, once lemonfiber's desired moved, would read as stale and revert.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, _changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "an adopted value equal to desired is not re-recorded as written",
    );
}

#[tokio::test]
async fn an_adopted_value_the_service_still_holds_is_kept_not_made_stale() {
    // The run after adoption: the baseline now holds the operator's value, marked
    // adopted, and the service still holds it, while lemonfiber's desired differs. A
    // written value here would be stale — lemonfiber's own, to bring up to date — but
    // an adopted one is theirs to keep, so it is left as it is and not overwritten.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "their-tv", "1");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "an adopted value is not overwritten");
}

#[tokio::test]
async fn an_adopt_pass_promotes_a_drifted_value_and_records_it_as_adopted() {
    // lemonfiber wrote "tv" and still wants it, but the operator changed it: a normal
    // seed reports drift. An adopt pass instead promotes their edit — reporting it
    // adopted and recording what the service holds as the accepted baseline, so a
    // later seed keeps it rather than flagging it again.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting an edit rewrites nothing in the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "my-own-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's edit is recorded as the adopted baseline",
    );
}

#[tokio::test]
async fn a_seed_after_an_adopt_pass_keeps_the_adopted_edit() {
    // What the adopt pass recorded above, read on the next ordinary seed: the value
    // is adopted, so the seed keeps it rather than reverting to lemonfiber's default —
    // the promotion survives future seeds.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "my-own-tv", "2");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "the adopted edit is kept, not reverted");
}

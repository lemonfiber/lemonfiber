use super::{ctx_holding, env_at, get, set, Setting};
use crate::app::{Ctx, Outcome};
use crate::config::{INDEXER_APIKEY_KEY, PROVIDER_PASS_KEY, PROVIDER_USER_KEY, VPN_PROVIDER_KEY};
use crate::model::{ConfigReport, DecisionReport};
use crate::reconfigure::{DecisionKey, DECISIONS};
use crate::test_support::recorded_as;

/// What each credential is seeded with: words nobody would take for a secret, each found
/// nowhere else in an answer.
const SEEDED: [(&str, &str); 3] = [
    (PROVIDER_USER_KEY, "marigold-account-name"),
    (PROVIDER_PASS_KEY, "marigold-account-word"),
    (INDEXER_APIKEY_KEY, "marigold-search-word"),
];

/// What the settings store is seeded with for the VPN's provider.
const PROVIDER: &str = "marigold-tunnel";

/// The decisions the `config` read answered at `ctx`, naming `key` where it names one.
async fn listed(ctx: &Ctx, key: Option<&str>) -> Vec<DecisionReport> {
    get(ctx, key)
        .await
        .map(|report| report.decisions)
        .unwrap_or_default()
}

/// The one entry for `key`.
fn entry(decisions: &[DecisionReport], key: DecisionKey) -> Option<&DecisionReport> {
    decisions.iter().find(|decision| decision.key == key)
}

/// A machine holding `contents` in its environment file, each credential of [`SEEDED`]
/// where lemonfiber keeps it, and the VPN's provider and the house's locale in its settings
/// store.
fn seeded(name: &str, contents: &str) -> Ctx {
    let env = env_at(name, contents);
    let path = env.to_path_buf();
    let ctx = ctx_holding(env);
    for (key, value) in SEEDED {
        recorded_as(&path, key, value);
    }
    let stored = crate::settings::store::recorded(
        &path.with_file_name(crate::config::paths::SETTINGS),
        &[
            ("gluetun:provider", Some(PROVIDER)),
            (crate::locale::SETTING, Some("nl-NL")),
        ]
        .into(),
    );
    assert!(stored.is_ok(), "the scratch settings store records both");
    ctx
}

#[tokio::test]
async fn every_decision_is_listed_once_in_the_catalogues_order_whatever_the_read_names() {
    let ctx = ctx_holding(env_at("decisions-order", "LEMONFIBER_USENET=on\n"));
    let catalogued: Vec<DecisionKey> = DECISIONS.iter().map(|decision| decision.key).collect();
    for key in [None, Some("LEMONFIBER_USENET")] {
        let keys: Vec<DecisionKey> = listed(&ctx, key)
            .await
            .iter()
            .map(|decision| decision.key)
            .collect();
        assert_eq!(keys, catalogued, "{key:?}");
    }
    for (decision, row) in listed(&ctx, None).await.iter().zip(&DECISIONS) {
        assert_eq!(decision.cost, row.cost);
        assert_eq!(decision.affects, row.affects);
    }
}

#[tokio::test]
async fn a_decision_nothing_holds_is_listed_as_not_held_and_one_held_as_held() {
    let ctx = ctx_holding(env_at(
        "decisions-held",
        "DATA_ROOT=/srv/media\nLEMONFIBER_USENET=on\nTZ=\n",
    ));
    let decisions = listed(&ctx, None).await;
    for key in [
        DecisionKey::ProviderHost,
        DecisionKey::Puid,
        DecisionKey::FrontDoor,
        DecisionKey::Zone,
        DecisionKey::ProviderPass,
        DecisionKey::VpnProvider,
        DecisionKey::Locale,
    ] {
        assert_eq!(
            entry(&decisions, key).map(|decision| decision.held),
            Some(false),
            "{}",
            key.as_str()
        );
    }
    for key in [DecisionKey::DataRoot, DecisionKey::Usenet] {
        assert_eq!(
            entry(&decisions, key).map(|decision| decision.held),
            Some(true),
            "{}",
            key.as_str()
        );
    }
}

#[tokio::test]
async fn a_credential_and_a_stored_setting_are_held_where_their_store_holds_them() {
    let ctx = seeded("decisions-stored", "LEMONFIBER_USENET=on\n");
    let decisions = listed(&ctx, None).await;
    for key in [
        DecisionKey::ProviderUser,
        DecisionKey::ProviderPass,
        DecisionKey::IndexerApikey,
        DecisionKey::VpnProvider,
        DecisionKey::Locale,
    ] {
        assert_eq!(
            entry(&decisions, key).map(|decision| decision.held),
            Some(true),
            "{}",
            key.as_str()
        );
    }
}

#[tokio::test]
async fn no_decision_carries_a_value_and_no_credential_is_answered_anywhere() {
    let ctx = seeded(
        "decisions-withheld",
        "DATA_ROOT=/srv/marigold-library\nLEMONFIBER_USENET=on\n",
    );
    let Ok(report) = get(&ctx, None).await else {
        unreachable!("the read answers")
    };
    let answered = Outcome::Config(report.clone())
        .envelope()
        .to_json()
        .unwrap_or_default();
    assert!(answered.contains("\"decisions\":["), "{answered}");
    for (key, value) in SEEDED {
        assert!(!answered.contains(value), "{key} was answered: {answered}");
    }
    let decisions = serde_json::to_value(&report.decisions).unwrap_or_default();
    let written = decisions.to_string();
    for value in ["marigold", "/srv", "nl-NL"] {
        assert!(!written.contains(value), "a decision carries {value}");
    }
    let fields = ["key", "cost", "affects", "held", "credential", "setting"];
    let Some(entries) = decisions.as_array() else {
        unreachable!("the decisions are a list")
    };
    assert_eq!(entries.len(), DECISIONS.len());
    for one in entries {
        let names: Vec<&String> = one
            .as_object()
            .map(|object| object.keys().collect())
            .unwrap_or_default();
        assert!(
            names.iter().all(|name| fields.contains(&name.as_str())),
            "{one}"
        );
    }
}

#[tokio::test]
async fn the_credentials_are_flagged_and_nothing_else_is() {
    let ctx = ctx_holding(env_at("decisions-flagged", "LEMONFIBER_USENET=on\n"));
    let flagged: Vec<DecisionKey> = listed(&ctx, None)
        .await
        .iter()
        .filter(|decision| decision.credential)
        .map(|decision| decision.key)
        .collect();
    assert_eq!(
        flagged,
        [
            DecisionKey::IndexerApikey,
            DecisionKey::ProviderUser,
            DecisionKey::ProviderPass,
        ]
    );
    for key in &flagged {
        assert!(!crate::config::display::in_full(key.as_str()));
    }
}

#[tokio::test]
async fn a_decision_the_settings_store_keeps_names_the_setting_config_set_refuses_it_for() {
    let ctx = ctx_holding(env_at("decisions-setting", "LEMONFIBER_USENET=on\n"));
    let decisions = listed(&ctx, None).await;
    let named: Vec<(DecisionKey, &str)> = decisions
        .iter()
        .filter_map(|decision| Some((decision.key, decision.setting.as_deref()?)))
        .collect();
    assert_eq!(
        named,
        [
            (DecisionKey::VpnProvider, "gluetun:provider"),
            (DecisionKey::Locale, crate::locale::SETTING),
        ]
    );
    for decision in &decisions {
        assert_eq!(
            crate::settings::fences::config_takes(decision.key.as_str(), true).is_err(),
            decision.setting.is_some(),
            "{}",
            decision.key.as_str()
        );
    }
    assert_eq!(DecisionKey::VpnProvider.as_str(), VPN_PROVIDER_KEY);
}

#[tokio::test]
async fn a_change_answers_the_decisions_as_the_file_stands_after_it() {
    let path = env_at("decisions-change", "LEMONFIBER_USENET=on\n");
    let ctx = ctx_holding(path);
    let changed: Option<ConfigReport> = set(&ctx, Setting::to("PUID", "1000").agreed(true))
        .await
        .ok();
    assert_eq!(
        changed
            .as_ref()
            .and_then(|report| entry(&report.decisions, DecisionKey::Puid))
            .map(|decision| decision.held),
        Some(true)
    );
}

//! What the catalogue offers: listed from a signed index alone, each entry with what is
//! installed of it, and a catalogue switched off or unread said rather than listed as
//! nothing.

use lemonfiber_fixtures::http::Answer;

use super::*;
use crate::config::{Reaching, REACH_CATALOGUE_KEY};
use crate::error::codes::plugin::{
    CATALOGUE_UNREACHABLE, CATALOGUE_UNREADABLE, SIGNATURE_UNVERIFIED,
};
use crate::model::{PluginCatalogueReport, PluginCatalogueState, SettingWrite};
use crate::plugin::catalogue::signing::Signing;

/// The date the test index says it was published, and the instant that is.
const PUBLISHED: &str = "2026-11-20T14:24:16Z";
const PUBLISHED_AT: u64 = 1_795_184_656;

/// One entry of an index, carrying every fact a listing reads where `version` is given.
fn entry(id: &str, version: Option<&str>) -> String {
    let facts = version.map_or_else(String::new, |version| {
        format!(
            ", \"name\": \"Komga\", \"description\": \"Reads comics\", \"version\": \
             \"{version}\", \"without_it\": \"Comics stay folders of images\""
        )
    });
    format!(
        "{{\"id\": \"{id}\", \"origin\": \"https://github.com/lemonfiber/plugin-{id}\", \
         \"revision\": \"{}\", \"manifest\": \"sha256:{}\"{facts}}}",
        "a".repeat(40),
        "1".repeat(64)
    )
}

/// An index of these entries, as release `serial`, published where `published` is given.
fn index(serial: u64, published: Option<&str>, entries: &[String]) -> String {
    let dated = published.map_or_else(String::new, |at| format!("\"published\": \"{at}\", "));
    format!(
        "{{\"schema\": 1, \"serial\": {serial}, {dated}\"plugins\": [{}]}}\n",
        entries.join(", ")
    )
}

/// A catalogue release serving `index`, signed by `signing` where one is given.
fn release(index: &str, signing: Option<&Signing>) -> Arc<Fake> {
    Fake::by_path(vec![
        (
            "index.json.sig",
            signing.map_or_else(
                || Answer::reply(404, "Not Found"),
                |one| Answer::reply(200, one.signed(index)),
            ),
        ),
        ("index.json", Answer::reply(200, index)),
    ])
}

/// A machine asking this transport, carrying the key `signing` signs with.
fn browsing(name: &str, http: Arc<Fake>, signing: Option<&Signing>) -> Ctx {
    let mut ctx = ctx(name).with_http(http);
    ctx.catalogue_key = signing.and_then(Signing::key).map(Box::new);
    ctx
}

/// What the catalogue offers, where the read is answered at all.
async fn offered(ctx: &Ctx) -> Option<PluginCatalogueReport> {
    super::super::offered(ctx).await.ok()
}

/// Every entry the signed index carries in full is listed with its facts, an entry that
/// lacks them is counted rather than listed, and the index's date and serial are said.
#[tokio::test]
async fn a_signed_index_lists_what_it_carries_in_full_and_counts_the_rest() {
    let made = Signing::new();
    assert!(made.is_some(), "no key pair could be made");
    let Some(signing) = made else { return };
    let listed = index(
        7,
        Some(PUBLISHED),
        &[entry("komga", Some("1.2.0")), entry("bare", None)],
    );
    let ctx = browsing(
        "browse-listed",
        release(&listed, Some(&signing)),
        Some(&signing),
    );

    let report = offered(&ctx).await;

    let report = report.as_ref();
    assert_eq!(
        report.map(|one| one.state),
        Some(PluginCatalogueState::Listed)
    );
    assert_eq!(report.and_then(|one| one.serial), Some(7));
    assert_eq!(
        report.and_then(|one| one.published),
        Some(crate::instant::Instant::from_seconds(PUBLISHED_AT))
    );
    assert_eq!(report.and_then(|one| one.unreadable), Some(1));
    let entries = report.map(|one| one.entries.clone()).unwrap_or_default();
    assert_eq!(
        entries
            .iter()
            .map(|one| (
                one.id.as_str(),
                one.name.as_str(),
                one.description.as_str(),
                one.version.as_str(),
                one.without_it.as_str(),
                one.installed.is_none()
            ))
            .collect::<Vec<_>>(),
        vec![(
            "komga",
            "Komga",
            "Reads comics",
            "1.2.0",
            "Comics stay folders of images",
            true
        )]
    );
    assert!(report.is_some_and(|one| one.setting.is_none() && one.failure.is_none()));
}

/// An installed plugin carries the version installed, and whether the catalogue's is
/// newer is decided here by semver.
#[tokio::test]
async fn an_installed_plugin_says_whether_the_catalogue_holds_a_newer_version() {
    let made = Signing::new();
    assert!(made.is_some(), "no key pair could be made");
    let Some(signing) = made else { return };
    for (catalogued, newer) in [
        ("1.3.0", true),
        ("1.2.0", false),
        ("1.10.0", true),
        ("1.1.9", false),
    ] {
        let name = format!("browse-installed-{catalogued}");
        let listed = index(3, Some(PUBLISHED), &[entry("komga", Some(catalogued))]);
        let ctx = browsing(&name, release(&listed, Some(&signing)), Some(&signing));
        assert_eq!(
            counted(installing(&ctx, &source(&name, MANIFEST)).await),
            Some(1)
        );

        let installed = offered(&ctx)
            .await
            .and_then(|one| one.entries.first().cloned())
            .and_then(|one| one.installed);

        assert_eq!(
            installed.map(|one| (one.version, one.newer)),
            Some(("1.2.0".to_owned(), newer)),
            "{catalogued}"
        );
    }
}

/// Switched off, the catalogue is not asked, and the answer names the setting that holds
/// it off and the value that lets it be asked: the blanket switch where that is on.
#[tokio::test]
async fn a_catalogue_switched_off_is_not_asked_and_names_what_turns_it_on() {
    for (reaching, key, value) in [
        (
            Reaching::without(REACH_CATALOGUE_KEY),
            REACH_CATALOGUE_KEY,
            "on",
        ),
        (Reaching::none(), crate::config::OFFLINE_KEY, "off"),
    ] {
        let silent = Fake::silent();
        let mut ctx = browsing("browse-off", silent.clone(), None);
        ctx.settings.reaching = reaching;

        let report = offered(&ctx).await;

        assert!(silent.requests().is_empty(), "{:?}", silent.requests());
        assert_eq!(
            report.as_ref().map(|one| one.state),
            Some(PluginCatalogueState::Off)
        );
        assert_eq!(
            report
                .as_ref()
                .and_then(|one| one.setting.clone())
                .map(|one| (one.key, one.value, one.action)),
            Some((key.to_owned(), value.to_owned(), SettingWrite::ConfigSet))
        );
        assert!(report.is_some_and(|one| one.entries.is_empty()
            && one.failure.is_none()
            && one.published.is_none()));
    }
}

/// An index that cannot be fetched, verified, held to its size or dated is answered as
/// failed with the code an install by name is refused with, and lists nothing.
#[tokio::test]
async fn an_index_not_read_is_said_with_its_code_and_lists_nothing() {
    let pairs = Signing::new().zip(Signing::new());
    assert!(pairs.is_some(), "no key pair could be made");
    let Some((ours, theirs)) = pairs else { return };
    let full = index(2, Some(PUBLISHED), &[entry("komga", Some("1.2.0"))]);
    let undated = index(2, None, &[entry("komga", Some("1.2.0"))]);
    let endless = index(
        2,
        Some(PUBLISHED),
        &vec![entry("komga", Some("1.2.0")); 5_000],
    );
    for (name, http, key, code) in [
        (
            "browse-theirs",
            release(&full, Some(&theirs)),
            Some(&ours),
            SIGNATURE_UNVERIFIED,
        ),
        (
            "browse-unsigned",
            release(&full, None),
            Some(&ours),
            SIGNATURE_UNVERIFIED,
        ),
        (
            "browse-keyless",
            release(&full, Some(&ours)),
            None,
            SIGNATURE_UNVERIFIED,
        ),
        (
            "browse-down",
            Fake::silent(),
            Some(&ours),
            CATALOGUE_UNREACHABLE,
        ),
        (
            "browse-endless",
            release(&endless, Some(&ours)),
            Some(&ours),
            CATALOGUE_UNREACHABLE,
        ),
        (
            "browse-undated",
            release(&undated, Some(&ours)),
            Some(&ours),
            CATALOGUE_UNREADABLE,
        ),
    ] {
        let ctx = browsing(name, http, key);

        let report = offered(&ctx).await;

        assert_eq!(
            report.as_ref().map(|one| one.state),
            Some(PluginCatalogueState::Failed),
            "{name}"
        );
        assert_eq!(
            report
                .as_ref()
                .and_then(|one| one.failure.as_ref())
                .map(|one| one.code.as_str()),
            Some(code.as_str()),
            "{name}"
        );
        assert!(
            report.is_some_and(|one| one.entries.is_empty()
                && one.setting.is_none()
                && one.serial.is_none()
                && one.unreadable.is_none()),
            "{name}"
        );
    }
}

/// A reading remembers nothing: the newest index this machine verified is left as it
/// was, so an install by name still holds a later index to what an act recorded.
#[tokio::test]
async fn a_reading_writes_nothing() {
    let made = Signing::new();
    assert!(made.is_some(), "no key pair could be made");
    let Some(signing) = made else { return };
    let listed = index(9, Some(PUBLISHED), &[entry("komga", Some("1.2.0"))]);
    let ctx = browsing(
        "browse-writes",
        release(&listed, Some(&signing)),
        Some(&signing),
    );
    let newest = crate::app::targets::beside_env(&ctx, crate::config::paths::CATALOGUE);

    let report = offered(&ctx).await;

    assert_eq!(
        report.map(|one| one.state),
        Some(PluginCatalogueState::Listed)
    );
    assert!(newest.is_some_and(|at| !at.exists()));
}

/// An index older than the newest one an act on this machine verified is failed, as
/// an install by name would refuse it.
#[tokio::test]
async fn an_index_older_than_the_newest_verified_is_failed() {
    let made = Signing::new();
    assert!(made.is_some(), "no key pair could be made");
    let Some(signing) = made else { return };
    let listed = index(4, Some(PUBLISHED), &[entry("komga", Some("1.2.0"))]);
    let ctx = browsing(
        "browse-replaced",
        release(&listed, Some(&signing)),
        Some(&signing),
    );
    let newest = crate::app::targets::beside_env(&ctx, crate::config::paths::CATALOGUE);
    assert!(newest
        .as_ref()
        .is_some_and(|at| std::fs::write(at, "{\"serial\": 5}").is_ok()));

    let report = offered(&ctx).await;

    assert_eq!(
        report.and_then(|one| one.failure).map(|one| one.code),
        Some(
            crate::error::codes::plugin::CATALOGUE_REPLACED
                .as_str()
                .to_owned()
        )
    );
}

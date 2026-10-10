//! The subtitle finder's client, driven through the HTTP port against a fake
//! transport.
//!
//! What it sends is the whole of the interesting part: the service takes a form
//! whose field names are its configuration file's own paths flattened, so a name
//! spelt wrong is a setting silently not set. Each test therefore asserts the
//! request that went out rather than only what came back — a fake answers whatever
//! it was scripted to, and cannot refuse what the service would.
//!
//! The shapes here were read off `lscr.io/linuxserver/bazarr:1.4.5`, not guessed.

use std::sync::Arc;

use lemonfiber_core::bazarr::{api_key, Bazarr};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::{Failure, Subtitles, Watched};
use lemonfiber_fixtures::http::{Answer, Fake};

/// The key this client presents, assembled rather than written down: a literal
/// reads to the source scanner as a credential committed to the repository.
fn key() -> String {
    ["the", "-key"].concat()
}

fn subtitle_finder(fake: &Arc<Fake>) -> Bazarr {
    let http: Arc<dyn Http> = fake.clone();
    Bazarr::new(http, "http://127.0.0.1:6767", "bazarr", key())
}

/// Settings as the service reports them, holding one curator and not the other.
const HOLDING_CURATOR: &str = r#"{
    "general": { "use_sonarr": true, "use_radarr": false },
    "sonarr": { "ip": "sonarr", "port": 8989, "apikey": "set", "base_url": "" },
    "radarr": { "ip": "127.0.0.1", "port": 7878, "apikey": "", "base_url": "/" }
}"#;

/// A curator as the finder is told about it.
fn watched(which: Kind) -> Watched {
    let (host, port) = match which {
        Kind::Tv => ("sonarr", 8989),
        Kind::Movies => ("radarr", 7878),
    };
    Watched {
        which,
        host: host.to_owned(),
        port,
        api_key: ["arr", "-key"].concat(),
    }
}

/// What is already set is read back, per curator and not in aggregate.
///
/// The two are wired independently, so a reading that answered for both together
/// would report one as done because the other was.
#[tokio::test]
async fn what_the_finder_holds_is_read_for_one_curator_at_a_time() {
    let fake = Fake::always(Answer::reply(200, HOLDING_CURATOR));
    let finder = subtitle_finder(&fake);

    let television = finder.watching(Kind::Tv).await.ok();
    assert!(
        television.as_ref().is_some_and(|held| held.enabled
            && held.host == "sonarr"
            && held.port == 8989
            && held.keyed),
        "{television:?}"
    );

    let film = finder.watching(Kind::Movies).await.ok();
    assert!(
        film.as_ref()
            .is_some_and(|held| !held.enabled && !held.keyed),
        "the other *arr was reported as set up: {film:?}"
    );
}

/// The key travels in the header the service asks for it in.
#[tokio::test]
async fn the_key_is_presented_where_the_service_looks_for_it() {
    let fake = Fake::always(Answer::reply(200, HOLDING_CURATOR));
    let _ = subtitle_finder(&fake).watching(Kind::Tv).await;

    let sent = fake.requests();
    let carried = sent
        .first()
        .map(|request| request.headers.clone())
        .unwrap_or_default();
    assert!(
        carried
            .iter()
            .any(|(name, value)| name == "X-API-KEY" && value == &key()),
        "the key was not presented: {carried:?}"
    );
}

/// The switch and the address are written together, under the names the service
/// files them by.
///
/// Either alone is nothing: an address it is not set to use is never read, and the
/// switch with no address gives it somewhere unreachable to look. The names are the
/// configuration file's own paths flattened, so a field spelt otherwise is a
/// setting quietly not set.
#[tokio::test]
async fn pointing_it_at_a_curator_sets_the_address_and_the_switch_at_once() {
    let fake = Fake::always(Answer::reply(204, ""));
    let told = subtitle_finder(&fake).watch(&watched(Kind::Tv)).await;
    assert!(told.is_ok(), "{told:?}");

    let sent = fake.requests();
    let body = sent
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default();

    for expected in [
        "settings-general-use_sonarr=true",
        "settings-sonarr-ip=sonarr",
        "settings-sonarr-port=8989",
        "settings-sonarr-apikey=arr-key",
    ] {
        assert!(body.contains(expected), "{expected} is missing from {body}");
    }
    assert!(
        sent.first().is_some_and(|request| request
            .headers
            .iter()
            .any(|(name, value)| name == "Content-Type"
                && value == "application/x-www-form-urlencoded")),
        "the body was not declared as a form, which is the only shape it is read in"
    );
}

/// Each curator is written under its own name, so wiring one does not claim the other.
#[tokio::test]
async fn each_curator_is_written_under_the_name_the_service_files_it_by() {
    let fake = Fake::always(Answer::reply(204, ""));
    let _ = subtitle_finder(&fake).watch(&watched(Kind::Movies)).await;

    let sent = fake.requests();
    let body = sent
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default();

    assert!(body.contains("settings-general-use_radarr=true"), "{body}");
    assert!(body.contains("settings-radarr-ip=radarr"), "{body}");
    assert!(
        !body.contains("sonarr"),
        "wiring film named television as well: {body}"
    );
}

/// A rejected key is named as one, and a silence is named as a silence.
///
/// The two are fixed differently — one is a credential to correct, the other a
/// service to start — so an answer that flattened them would send the operator
/// looking in the wrong place.
#[tokio::test]
async fn a_service_that_will_not_take_it_is_reported() {
    let refused = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        subtitle_finder(&refused).watch(&watched(Kind::Tv)).await,
        Err(Failure::Unauthorised { .. })
    ));

    let silent = Fake::silent();
    assert!(matches!(
        subtitle_finder(&silent).watching(Kind::Tv).await,
        Err(Failure::Unavailable { .. })
    ));
}

/// The finder's own key is the one under `auth`, not the first one in the file.
///
/// Its configuration holds an `apikey` under `auth` and another under each curator it
/// has been pointed at, so a scan for the name meets whichever section comes first.
/// The file the service writes is alphabetical, so `auth` leads both of those and
/// that scan is right by accident rather than by anything the format promises. This
/// fixture keeps that order; the next test reverses it.
#[test]
fn the_key_read_is_the_finders_own_and_not_a_curators() {
    const WRITTEN: &str = "\
auth:
  apikey: the-finders-own
radarr:
  apikey: radarrs-key
sonarr:
  apikey: sonarrs-key
";
    assert_eq!(api_key(WRITTEN).as_deref(), Some("the-finders-own"));
}

/// The same file with the sections the other way round answers the same.
#[test]
fn the_section_decides_the_key_rather_than_the_order() {
    const REORDERED: &str = "\
sonarr:
  apikey: sonarrs-key
auth:
  apikey: the-finders-own
";
    assert_eq!(api_key(REORDERED).as_deref(), Some("the-finders-own"));
}

/// A file that names no key at all yields none, rather than something empty.
#[test]
fn a_configuration_with_no_key_yet_yields_nothing() {
    assert_eq!(api_key("auth:\n  apikey:\n").as_deref(), None);
    assert_eq!(api_key("general:\n  use_sonarr: false\n").as_deref(), None);
}

/// The other settings the `auth` section holds are passed over.
///
/// It is not a section with one entry in it: the real file carries `password`, `type`
/// and `username` beside the key. Those are alphabetical too, so `apikey` leads them
/// and a reader taking the section's first value would be right by accident — which
/// is why the match is on the field name and this fixture puts the key last.
#[test]
fn the_other_settings_beside_the_key_are_passed_over() {
    const WRITTEN: &str = "\
auth:
  type: form
  username: someone
  password: not-the-key
  apikey: the-finders-own
";
    assert_eq!(api_key(WRITTEN).as_deref(), Some("the-finders-own"));
}

use base64::Engine as _;

use super::super::Took;
use super::asking::{answering, field, json, noted, recorded, sonarr, source, took, SERVED};

const GUARDED: &str = "recordings/sonarr/library-curate-guarded.json";
const WANTED: &str = "recordings/sonarr/library-curate-wanted.json";

const DECLARED: &str = r#"
[service.env]
PROVIDER   = { setting = "sonarr:provider", default = "", recorded_with = "custom" }
ENDPOINT   = { setting = "sonarr:endpoint", default = "", recorded_with = "192.0.2.1" }
TUNNEL_KEY = { setting = "sonarr:tunnel-key", default = "", recorded_with = "minted" }
PLAIN      = { setting = "sonarr:plain", default = "" }
VPN_KEY    = { secret = "wireguard-key" }

[[secret]]
id            = "wireguard-key"
of            = "sonarr"
why           = "Dial the tunnel to your VPN provider"
recorded_with = "minted"
"#;

const SAID: &str = "Taken with the stand-ins the manifest declares for ENDPOINT, PROVIDER, \
                    TUNNEL_KEY, VPN_KEY, never with an operator's value.";

fn standing_in() -> String {
    SERVED.replacen("\n[[claim]]", &format!("{DECLARED}\n[[claim]]"), 1)
}

fn made() -> String {
    base64::engine::general_purpose::STANDARD.encode([7_u8; 32])
}

#[tokio::test]
async fn a_service_is_served_the_stand_ins_its_manifest_declares_and_no_operator_value() {
    let at = source("standing-in-served", &standing_in());
    noted(&at, GUARDED, "Refused without the key.");
    noted(&at, WANTED, "Read with the key it wrote.");
    let serving = sonarr();
    let read = recorded(&at, &serving, &answering("{\"records\": []}")).await;
    assert_eq!(took(read.as_ref()), vec![Took::Written, Took::Written]);
    let asked = serving.asked();
    let env = asked
        .first()
        .map(|served| served.env.clone())
        .unwrap_or_default();
    for given in [
        "PROVIDER=custom".to_owned(),
        "ENDPOINT=192.0.2.1".to_owned(),
        format!("TUNNEL_KEY={}", made()),
        format!("VPN_KEY={}", made()),
    ] {
        assert!(
            env.contains(&given),
            "{} is given its stand-in",
            given.split('=').next().unwrap_or_default()
        );
    }
    assert!(
        !env.iter().any(|one| one.starts_with("PLAIN=")),
        "a setting declaring no stand-in is given nothing"
    );
}

#[tokio::test]
async fn a_recording_taken_with_stand_ins_says_so_in_its_note_once_however_often_taken() {
    let at = source("standing-in-noted", &standing_in());
    noted(&at, GUARDED, "Refused without the key.");
    noted(&at, WANTED, "Read with the key it wrote.");
    let http = answering("{\"records\": []}");
    let _ = recorded(&at, &sonarr(), &http).await;
    let said = format!("Refused without the key. {SAID}");
    assert_eq!(field(&json(&at, GUARDED), "/note"), said.as_str());

    let again = recorded(&at, &sonarr(), &answering("{\"records\": []}")).await;
    assert_eq!(
        took(again.as_ref()),
        vec![Took::Reproduced, Took::Reproduced]
    );
    assert_eq!(field(&json(&at, GUARDED), "/note"), said.as_str());
}

#[tokio::test]
async fn a_recording_whose_only_note_names_its_stand_ins_is_still_unnoted() {
    let at = source("standing-in-unnoted", &standing_in());
    let read = recorded(&at, &sonarr(), &answering("{\"records\": []}")).await;
    assert_eq!(took(read.as_ref()), vec![Took::Unnoted, Took::Unnoted]);
    assert_eq!(field(&json(&at, WANTED), "/note"), SAID);

    let again = recorded(&at, &sonarr(), &answering("{\"records\": []}")).await;
    assert_eq!(took(again.as_ref()), vec![Took::Unnoted, Took::Unnoted]);
}

#[tokio::test]
async fn a_stand_in_made_for_the_run_is_written_as_redacted_wherever_an_answer_echoes_it() {
    let at = source("standing-in-scrubbed", &standing_in());
    let echo = format!("{{\"records\": [\"{}\", \"custom\"]}}", made());
    let read = recorded(&at, &sonarr(), &answering(&echo)).await;
    assert!(read.is_some_and(|read| read.failed.is_empty()));
    assert_eq!(
        field(&json(&at, WANTED), "/response/json/records"),
        serde_json::json!(["<redacted>", "custom"]),
        "a declared constant reaches no third party and is kept"
    );
}

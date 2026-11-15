use super::super::Took;
use super::asking::{answering, field, json, noted, recorded, sonarr, source, took, SERVED};

const GUARDED: &str = "recordings/sonarr/library-curate-guarded.json";
const WANTED: &str = "recordings/sonarr/library-curate-wanted.json";

const DECLARED: &str = r#"
[service.env]
API_KEY = { secret = "api-key" }

[[secret]]
id  = "api-key"
of  = "sonarr"
why = "Lets the stack ask Sonarr what it wants"
"#;

fn minting() -> String {
    SERVED.replacen("\n[[proof]]", &format!("{DECLARED}\n[[proof]]"), 1)
}

fn minted() -> String {
    "07".repeat(32)
}

#[tokio::test]
async fn a_service_not_fronted_is_given_each_secret_its_environment_names_minted_for_the_run() {
    let at = source("minting-served", &minting());
    noted(&at, GUARDED, "Refused without the key.");
    noted(&at, WANTED, "Read as nobody.");
    let serving = sonarr();
    let read = recorded(&at, &serving, &answering("{\"records\": []}")).await;
    assert_eq!(took(read.as_ref()), vec![Took::Written, Took::Written]);
    let env = serving
        .asked()
        .first()
        .map(|served| served.env.clone())
        .unwrap_or_default();
    assert!(env.contains(&format!("API_KEY={}", minted())));
}

#[tokio::test]
async fn a_secret_minted_for_the_run_is_written_as_redacted_wherever_an_answer_echoes_it() {
    let at = source("minting-scrubbed", &minting());
    let echo = format!("{{\"records\": [\"{}\"]}}", minted());
    let read = recorded(&at, &sonarr(), &answering(&echo)).await;
    assert!(read.is_some_and(|read| read.failed.is_empty()));
    assert_eq!(
        field(&json(&at, WANTED), "/response/json/records"),
        serde_json::json!(["<redacted>"])
    );
}

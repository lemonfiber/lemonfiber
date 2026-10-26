use super::{answering, engine, recorded, source, FRONTED};

const DECLARED: &str = r#"
[service.env]
REGION = { setting = "jellyfin:region", default = "", recorded_with = "eu" }
TOKEN  = { secret = "tunnel-token" }
"#;

const SECRET: &str = r#"
[[secret]]
id            = "tunnel-token"
of            = "jellyfin"
why           = "Reach the tunnel the server sits behind"
recorded_with = "a-documented-token"
"#;

fn standing_in() -> String {
    FRONTED.replacen(
        "\n[[service]]\nid          = \"jellyfin-adapter\"",
        &format!("{DECLARED}\n[[service]]\nid          = \"jellyfin-adapter\""),
        1,
    ) + SECRET
}

#[tokio::test]
async fn an_upstream_fronted_by_an_adapter_is_served_the_stand_ins_its_manifest_declares() {
    let at = source("fronted-standing-in", &standing_in());
    let serving = engine();
    let _ = recorded(&at, &serving, &answering(), Some(7)).await;
    let asked = serving.asked();
    let env = asked
        .first()
        .map(|served| served.env.clone())
        .unwrap_or_default();
    for given in ["REGION=eu", "TOKEN=a-documented-token"] {
        assert!(
            env.contains(&given.to_owned()),
            "{} is given its stand-in",
            given.split('=').next().unwrap_or_default()
        );
    }
    assert_eq!(
        env.iter().filter(|one| one.starts_with("TOKEN=")).count(),
        1,
        "a secret stood in for is given once"
    );
}

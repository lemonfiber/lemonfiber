use super::*;

/// A recipe telling the stack's television curator something, carrying the credential
/// lemonfiber holds for it.
const TELLING: &str = r#"
[[recipe]]
id    = "tell"
title = "Tell Sonarr"
why   = "To say hello"

[[recipe.input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[recipe.step]]
id     = "status"
call   = { method = "GET", to = "sonarr", path = "/api/v3/system/status", headers = { X-Api-Key = "{{key}}" } }
expect = { status = 200 }

[[recipe.pair]]
value = "key"
to    = "sonarr"

[requires]
capabilities = ["recipe.run"]
"#;

/// The key [`TELLING`] carried to the curator, installed under `name` in a context the
/// published key `k3y` is recorded in and `prepared` leaves.
async fn told_with(name: &str, prepared: fn(&crate::app::Ctx)) -> Vec<String> {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let http = Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/api/v3/system/status",
            lemonfiber_fixtures::http::Answer::reply(200, "{}"),
        ),
    ]);
    let ctx = proving(name, runner, http.clone());
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let key = crate::config::for_service("sonarr", crate::config::API_KEY_SUFFIX);
    crate::test_support::recorded_as(&env, &key, "k3y");
    prepared(&ctx);
    let at = crate::plugin::Source::Path(source(name, &format!("{PROVING}{TELLING}")));
    let asked = Asked::Install {
        source: at,
        consent: super::super::Consent::default(),
    };
    let install = report(answered(&ctx, asked).await).and_then(|one| one.install);
    assert_eq!(install.map(|one| one.recorded), Some(true));
    http.requests()
        .into_iter()
        .filter(|one| one.url.contains("/api/v3/system/status"))
        .flat_map(|one| one.headers)
        .filter(|(name, _)| name == "X-Api-Key")
        .map(|(_, value)| value)
        .collect()
}

/// A credential the credential store holds for a service of the stack's is the key
/// lemonfiber publishes for it, carried to that service.
#[tokio::test]
async fn a_credential_input_is_the_key_lemonfiber_publishes_for_its_service() {
    assert_eq!(told_with("credential", |_| {}).await, ["k3y"]);
}

/// Where a first-party part moved its own service in place of the stack's, the credential
/// is the key lemonfiber minted for it, which the adapter in front of it is handed too.
#[tokio::test]
async fn a_credential_input_of_a_moved_part_is_the_key_lemonfiber_minted_for_it() {
    let carried = told_with("credential-moved", |ctx| {
        crate::test_support::curators::TELEVISION.recorded(ctx);
        crate::test_support::curators::TELEVISION.minted(ctx, "minted-k3y");
    })
    .await;
    assert_eq!(carried, ["minted-k3y"]);
}

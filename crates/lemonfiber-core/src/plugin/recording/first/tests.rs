use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;

use super::super::tests::asking::SERVED;
use super::{before, performed, Before, Held, Presented, Reaching};
use crate::plugin::installed::{Installed, Placed};
use crate::plugin::program::{Starting, Throwaway};
use crate::ports::http::{Http, Method};
use crate::ports::serving::Served;

const SERVARR: &str =
    "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/config/config.xml\", version = 3 }";

/// What lemonfiber mints from seven repeated.
const MINTED: &str = "0707070707070707070707070707070707070707070707070707070707070707";

fn placed(api: &str) -> Option<Placed> {
    let text = SERVED.replace(SERVARR, api);
    let manifest = lemonfiber_plugin::Manifest::from_toml(&text);
    assert!(manifest.is_ok(), "{manifest:?}");
    manifest
        .ok()
        .and_then(|manifest| Installed::of(&manifest).services.into_iter().next())
}

fn copy(name: &str) -> Option<(Scratch, Throwaway)> {
    let source = Scratch::new(name);
    let copy = Throwaway::of(Some(Starting {
        root: &source,
        service: "sonarr",
    }))
    .ok()?;
    Some((source, copy))
}

fn served() -> Served {
    Served {
        container: "4f1c2d3e5a6b7c8d".to_owned(),
        network: "n".to_owned(),
        hostname: "4f1c2d3e5a6b".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: Some(49153),
    }
}

async fn first_run(
    placed: &Placed,
    copy: &Throwaway,
    serving: &Serves,
    http: Arc<Fake>,
) -> Result<Option<Held>, String> {
    let random = FixedRandom(Some(vec![7; 32]));
    let settled = before(placed, &random)?;
    let served = served();
    performed(&Reaching {
        placed,
        copy,
        served: &served,
        serving,
        keyed: http as Arc<dyn Http>,
        random: &random,
        before: &settled,
    })
    .await
}

#[tokio::test]
async fn a_service_writing_its_own_key_is_first_run_by_reading_it_and_presented_as_its_adapter_does(
) {
    let Some((_source, copy)) = copy("first-servarr") else {
        return;
    };
    let _ = std::fs::write(
        copy.path().join("config.xml"),
        "<Config><ApiKey>abc123abc123</ApiKey></Config>",
    );
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    assert_eq!(
        first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await,
        Ok(Some(Held {
            presented: Presented::Header("X-Api-Key", "abc123abc123".to_owned()),
            made: vec!["abc123abc123".to_owned()],
        }))
    );
}

#[tokio::test]
async fn a_key_the_service_reads_from_a_query_is_presented_there() {
    let Some((_source, copy)) = copy("first-sabnzbd") else {
        return;
    };
    let written = "7".repeat(32);
    let _ = std::fs::write(
        copy.path().join("sabnzbd.ini"),
        format!("[misc]\napi_key = {written}\n"),
    );
    let Some(placed) = placed(
        "api         = { kind = \"sabnzbd\", key_source = \"config-ini\", path = \"/config/sabnzbd.ini\" }",
    ) else {
        return;
    };
    let held = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert_eq!(
        held.map(|held| held.map(|held| held.presented)),
        Ok(Some(Presented::Query("apikey", written)))
    );
}

#[tokio::test]
async fn a_service_with_no_api_or_no_credential_has_no_first_run() {
    let Some((_source, copy)) = copy("first-none") else {
        return;
    };
    for api in [
        "",
        "api         = { kind = \"servarr\", key_source = \"none\", version = 3 }",
    ] {
        if let Some(placed) = placed(api) {
            assert_eq!(
                first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await,
                Ok(None),
                "{api}"
            );
        }
    }
}

#[tokio::test]
async fn a_book_curator_is_handed_a_key_lemonfiber_mints_before_it_starts() {
    let Some(curator) = placed("api         = { kind = \"bindery\", key_source = \"generated\" }")
    else {
        return;
    };
    let random = FixedRandom(Some(vec![7; 32]));
    let settled = before(&curator, &random);
    assert_eq!(
        settled.as_ref().map(|settled| settled.env.clone()),
        Ok(vec![format!("BINDERY_API_KEY={MINTED}")])
    );
    assert!(before(&curator, &FixedRandom(None)).is_err());
    let Some((_source, copy)) = copy("first-bindery") else {
        return;
    };
    assert_eq!(
        first_run(&curator, &copy, &Serves::on(1), Fake::silent()).await,
        Ok(Some(Held {
            presented: Presented::Header("X-Api-Key", MINTED.to_owned()),
            made: vec![MINTED.to_owned()],
        }))
    );
    let Some(other) = placed(SERVARR) else {
        return;
    };
    assert_eq!(before(&other, &random), Ok(Before::default()));
}

#[tokio::test]
async fn a_torrent_clients_announced_password_is_replaced_and_its_session_carries_the_operator() {
    let Some((_source, copy)) = copy("first-qbittorrent") else {
        return;
    };
    let Some(placed) =
        placed("api         = { kind = \"qbittorrent\", key_source = \"generated\" }")
    else {
        return;
    };
    let serving = Serves::on(1)
        .logging("starting\nA temporary password is provided for this session: tmpPass123\n");
    let http = Fake::by_route(vec![
        (
            Method::Post,
            "/api/v2/auth/login",
            Answer::reply(200, "Ok."),
        ),
        (
            Method::Post,
            "/api/v2/app/setPreferences",
            Answer::reply(200, ""),
        ),
    ]);
    let held = first_run(&placed, &copy, &serving, Arc::clone(&http)).await;
    assert_eq!(
        held,
        Ok(Some(Held {
            presented: Presented::Session,
            made: vec!["tmpPass123".to_owned(), MINTED.to_owned()],
        }))
    );
    let bodies: Vec<String> = http
        .requests()
        .into_iter()
        .map(|asked| asked.body.unwrap_or_default())
        .collect();
    assert_eq!(bodies.len(), 3);
    assert!(bodies
        .first()
        .is_some_and(|body| body.contains("password=tmpPass123")));
    assert!(bodies
        .get(2)
        .is_some_and(|body| body.contains(&format!("password={MINTED}"))));
}

#[tokio::test]
async fn a_listening_servers_root_account_is_made_and_signed_in_as() {
    let Some((_source, copy)) = copy("first-audiobookshelf") else {
        return;
    };
    let Some(placed) =
        placed("api         = { kind = \"audiobookshelf\", key_source = \"generated\" }")
    else {
        return;
    };
    let http =
        Fake::by_route(vec![
        (Method::Get, "/status", Answer::reply(200, "{\"isInit\": false}")),
        (Method::Post, "/init", Answer::reply(200, "")),
        (
            Method::Post,
            "/login",
            Answer::reply(
                200,
                "{\"user\": {\"accessToken\": \"acc.ess.token\", \"refreshToken\": \"re.fresh\"}}",
            ),
        ),
    ]);
    let held = first_run(&placed, &copy, &Serves::on(1), Arc::clone(&http)).await;
    assert_eq!(
        held,
        Ok(Some(Held {
            presented: Presented::Header("Authorization", "Bearer acc.ess.token".to_owned()),
            made: vec![
                MINTED.to_owned(),
                "acc.ess.token".to_owned(),
                "re.fresh".to_owned()
            ],
        }))
    );
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/login")
            && asked
                .headers
                .contains(&("x-return-tokens".to_owned(), "true".to_owned()))));
    let claimed = Fake::by_route(vec![(
        Method::Get,
        "/status",
        Answer::reply(200, "{\"isInit\": true}"),
    )]);
    assert!(first_run(&placed, &copy, &Serves::on(1), claimed)
        .await
        .is_err_and(|why| why.contains("already holds an account")));
}

#[tokio::test]
async fn an_aggregators_authentication_is_turned_on_and_its_restart_waited_out() {
    let Some((_source, copy)) = copy("first-nzbhydra2") else {
        return;
    };
    let Some(placed) = placed(
        "api         = { kind = \"nzbhydra2\", key_source = \"config-yaml\", path = \"/config/nzbhydra.yml\" }",
    ) else {
        return;
    };
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/config",
            vec![
                Answer::reply(
                    200,
                    "{\"auth\": {}, \"main\": {\"apiKey\": \"INSTANCEKEY123\"}, \"indexers\": []}",
                ),
                Answer::reply(200, "{}"),
                Answer::reply(401, ""),
            ],
        ),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, "{\"ok\": true}")],
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            vec![Answer::reply(200, "")],
        ),
    ]);
    let held = first_run(&placed, &copy, &Serves::on(1), Arc::clone(&http)).await;
    let pair = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{}:{MINTED}", crate::config::USENET_AGGREGATOR_ADMIN_USER),
    );
    assert_eq!(
        held,
        Ok(Some(Held {
            presented: Presented::Header("Authorization", format!("Basic {pair}")),
            made: vec![MINTED.to_owned(), "INSTANCEKEY123".to_owned(), pair],
        }))
    );
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Put
            && asked
                .body
                .as_deref()
                .is_some_and(|body| body.contains(MINTED))));
}

#[tokio::test]
async fn a_media_servers_first_run_is_its_adapters_and_refused_without_one() {
    let Some((_source, copy)) = copy("first-jellyfin") else {
        return;
    };
    let Some(placed) = placed("api         = { kind = \"jellyfin\", key_source = \"generated\" }")
    else {
        return;
    };
    let said = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert!(
        said.as_ref().is_err_and(|why| why.contains("adapter")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_key_file_outside_the_configuration_directory_is_refused() {
    let Some((_source, copy)) = copy("first-outside") else {
        return;
    };
    let Some(placed) = placed(
        "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/etc/config.xml\", version = 3 }",
    ) else {
        return;
    };
    let said = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("not beneath its configuration directory")),
        "{said:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_key_file_the_container_made_a_link_is_refused_rather_than_followed() {
    let Some((_source, copy)) = copy("first-linked") else {
        return;
    };
    let elsewhere = Scratch::new("first-linked-elsewhere");
    let _ = std::fs::write(
        elsewhere.join("host.xml"),
        "<Config><ApiKey>hostkeyhostkey</ApiKey></Config>",
    );
    let _ = std::os::unix::fs::symlink(elsewhere.join("host.xml"), copy.path().join("config.xml"));
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    let said = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("is a link or a device")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_subtitle_finders_key_is_read_from_its_configuration_and_presented_in_its_header() {
    let Some((_source, copy)) = copy("first-bazarr") else {
        return;
    };
    let written = "7".repeat(32);
    let _ = std::fs::create_dir_all(copy.path().join("config"));
    let _ = std::fs::write(
        copy.path().join("config/config.yaml"),
        format!("auth:\n  apikey: {written}\n"),
    );
    let Some(placed) = placed(
        "api         = { kind = \"bazarr\", key_source = \"config-yaml\", path = \"/config/config/config.yaml\" }",
    ) else {
        return;
    };
    let held = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert_eq!(
        held.map(|held| held.map(|held| held.presented)),
        Ok(Some(Presented::Header(crate::bazarr::KEY, written)))
    );
}

#[tokio::test(start_paused = true)]
async fn a_key_never_written_fails_the_first_run_at_its_limit() {
    let Some((_source, copy)) = copy("first-unwritten") else {
        return;
    };
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    let said = first_run(&placed, &copy, &Serves::on(1), Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("wrote no key to /config/config.xml within 60")),
        "{said:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_torrent_client_that_announces_no_password_fails_at_its_limit() {
    let Some((_source, copy)) = copy("first-unannounced") else {
        return;
    };
    let Some(placed) =
        placed("api         = { kind = \"qbittorrent\", key_source = \"generated\" }")
    else {
        return;
    };
    let said = first_run(
        &placed,
        &copy,
        &Serves::on(1).logging("started"),
        Fake::silent(),
    )
    .await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("announced no temporary password within 60")),
        "{said:?}"
    );
}

fn aggregating(name: &str) -> Option<(Scratch, Throwaway, Placed)> {
    let (source, copy) = copy(name)?;
    let placed = placed(
        "api         = { kind = \"nzbhydra2\", key_source = \"config-yaml\", path = \"/config/nzbhydra.yml\" }",
    )?;
    Some((source, copy, placed))
}

#[tokio::test]
async fn an_aggregator_that_will_not_take_its_authentication_fails_the_first_run() {
    let Some((_source, copy, placed)) = aggregating("first-unguarded") else {
        return;
    };
    for status in [500, 400] {
        let http = Fake::by_route(vec![
            (
                Method::Get,
                "/internalapi/config",
                Answer::reply(200, "{\"auth\": {}, \"main\": {}, \"indexers\": []}"),
            ),
            (
                Method::Put,
                "/internalapi/config",
                Answer::reply(status, "{\"message\": \"no\"}"),
            ),
        ]);
        let said = first_run(&placed, &copy, &Serves::on(1), http).await;
        assert!(said.is_err(), "{status}: {said:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn an_aggregator_still_answering_anybody_after_its_restart_fails_at_its_limit() {
    let Some((_source, copy, placed)) = aggregating("first-exposed") else {
        return;
    };
    let http = Fake::by_route(vec![
        (
            Method::Get,
            "/internalapi/config",
            Answer::reply(200, "{\"auth\": {}, \"main\": {}, \"indexers\": []}"),
        ),
        (
            Method::Put,
            "/internalapi/config",
            Answer::reply(200, "{\"ok\": true}"),
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            Answer::reply(200, ""),
        ),
    ]);
    let said = first_run(&placed, &copy, &Serves::on(1), http).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("still answered its configuration to anybody 60")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_listening_server_that_will_not_make_the_account_or_hand_back_a_token_fails() {
    let Some((_source, copy)) = copy("first-audiobookshelf-refused") else {
        return;
    };
    let Some(placed) =
        placed("api         = { kind = \"audiobookshelf\", key_source = \"generated\" }")
    else {
        return;
    };
    let unreachable = Fake::by_route(vec![(
        Method::Get,
        "/status",
        Answer::reply(200, "{\"isInit\": false}"),
    )]);
    assert!(first_run(&placed, &copy, &Serves::on(1), unreachable)
        .await
        .is_err());
    let tokenless = Fake::by_route(vec![
        (
            Method::Get,
            "/status",
            Answer::reply(200, "{\"isInit\": false}"),
        ),
        (Method::Post, "/init", Answer::reply(200, "")),
        (Method::Post, "/login", Answer::reply(200, "{\"user\": {}}")),
    ]);
    let said = first_run(&placed, &copy, &Serves::on(1), tokenless).await;
    assert!(
        said.as_ref().is_err_and(|why| why.contains("no token")),
        "{said:?}"
    );
}

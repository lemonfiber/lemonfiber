use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_plugin::{Health, HealthKind};

use super::super::tests::asking::SERVED;
use super::super::Recorder;
use super::{healthy, settled};
use crate::plugin::installed::{Installed, Placed};
use crate::plugin::program::{Starting, Throwaway};
use crate::ports::serving::{Served, Standing};

fn placed(text: &str) -> Option<Placed> {
    lemonfiber_plugin::Manifest::from_toml(text)
        .ok()
        .and_then(|manifest| Installed::of(&manifest).services.into_iter().next())
}

#[test]
fn a_service_is_served_as_lemonfiber_runs_it_with_throwaway_files_and_its_port() {
    let source = Scratch::new("served-settled");
    let Ok(copy) = Throwaway::of(Some(Starting {
        root: &source,
        service: "sonarr",
    })) else {
        return;
    };
    let Some(sonarr) = placed(SERVED) else {
        return;
    };
    let data = copy.emptied("data").ok();
    let serve = settled(&sonarr, &copy, data.as_deref());
    assert_eq!(
        serve.image,
        "lscr.io/linuxserver/sonarr@sha256:dffc730adcb8b4f4342792fbb27fcad9c62fb8660523f2aeb082416980d7fe0c"
    );
    assert_eq!(
        serve.env,
        vec![
            "TZ=Europe/Amsterdam".to_owned(),
            "PUID=1000".to_owned(),
            "PGID=1000".to_owned()
        ]
    );
    assert_eq!(serve.user, None);
    assert_eq!(serve.listens, Some(8989));
    assert!(!serve.read_only);
    assert_eq!(
        serve
            .binds
            .iter()
            .map(|bound| (bound.source.clone(), bound.target.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (copy.path().to_path_buf(), "/config"),
            (data.clone().unwrap_or_default(), "/data"),
        ]
    );
    let confined = SERVED
        .replace(
            "user_mode   = \"puid-env\"",
            "user_mode   = \"uid\"\nposture     = \"confined\"\ntmpfs       = [\"/tmp\"]",
        )
        .replace("takes_data  = true\n", "");
    let Some(confined) = placed(&confined) else {
        return;
    };
    let serve = settled(&confined, &copy, None);
    assert_eq!(serve.user.as_deref(), Some("1000:1000"));
    assert!(serve.read_only);
    assert_eq!(serve.cap_add, Vec::<String>::new());
    assert_eq!(serve.tmpfs, vec!["/tmp".to_owned()]);
    assert_eq!(serve.binds.len(), 1);
    let published = SERVED
        .replace("port        = 8989", "port        = 7878")
        .replace("listens     = 8989\n", "");
    let Some(published) = placed(&published) else {
        return;
    };
    assert_eq!(settled(&published, &copy, None).listens, Some(7878));
}

fn served(port: Option<u16>) -> Served {
    Served {
        container: "c0ffee0c0ffee".to_owned(),
        network: "n".to_owned(),
        hostname: "c0ffee0c0ffe".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port,
    }
}

async fn health_of(
    health: Option<&Health>,
    standing: Standing,
    http: &Fake,
    port: Option<u16>,
) -> Result<(), String> {
    let program = Proving::unreachable();
    let serving = Serves::on(1).standing(standing);
    let silent = || -> std::sync::Arc<dyn crate::ports::http::Http> { Fake::silent() };
    healthy(
        Recorder {
            program: program.as_ref(),
            serving: &serving,
            transport: &silent,
            random: &lemonfiber_fixtures::support::FixedRandom(None),
        },
        http,
        health,
        &served(port),
    )
    .await
}

fn health(kind: HealthKind, path: Option<&str>) -> Health {
    Health {
        kind,
        path: path.map(str::to_owned),
        timeout_s: Some(1),
    }
}

#[tokio::test]
async fn an_http_health_passes_on_an_answer_that_is_not_an_error_at_its_path() {
    let http = Fake::by_path(vec![("/ping", Answer::reply(204, ""))]);
    let ping = health(HealthKind::Http, Some("/ping"));
    assert_eq!(
        health_of(Some(&ping), Standing::Running, &http, Some(9)).await,
        Ok(())
    );
    assert!(http
        .requests()
        .first()
        .is_some_and(|asked| asked.url == "http://127.0.0.1:9/ping"));
    let refusing = Fake::by_path(vec![("/ping", Answer::reply(403, ""))]);
    assert_eq!(
        health_of(Some(&ping), Standing::Running, &refusing, Some(9)).await,
        Ok(()),
        "the application refusing a caller is the application up"
    );
    for status in [404, 500, 502] {
        let failing = Fake::by_path(vec![("/ping", Answer::reply(status, ""))]);
        assert!(
            health_of(Some(&ping), Standing::Running, &failing, Some(9))
                .await
                .is_err(),
            "{status}"
        );
    }
}

#[tokio::test]
async fn a_health_path_that_is_not_a_route_is_asked_at_the_root() {
    let http = Fake::by_path(vec![("9/", Answer::reply(200, ""))]);
    let elsewhere = health(HealthKind::Http, Some("@elsewhere.example/x"));
    assert_eq!(
        health_of(Some(&elsewhere), Standing::Running, &http, Some(9)).await,
        Ok(())
    );
    assert!(http
        .requests()
        .iter()
        .all(|asked| asked.url == "http://127.0.0.1:9/"));
}

#[tokio::test]
async fn a_tcp_health_passes_on_any_answer_at_all() {
    let http = Fake::always(Answer::reply(404, ""));
    let tcp = health(HealthKind::Tcp, None);
    assert_eq!(
        health_of(Some(&tcp), Standing::Running, &http, Some(9)).await,
        Ok(())
    );
    let silent = Fake::always(Answer::Silent);
    assert_eq!(
        health_of(Some(&tcp), Standing::Running, &silent, Some(9)).await,
        Err("its health did not pass within 1 seconds".to_owned())
    );
}

#[tokio::test]
async fn a_container_health_is_the_containers_own_word() {
    let silent = Fake::always(Answer::Silent);
    let own = health(HealthKind::Container, None);
    for (standing, passes) in [
        (Standing::Healthy, true),
        (Standing::Running, true),
        (Standing::Starting, false),
        (Standing::Unhealthy, false),
    ] {
        assert_eq!(
            health_of(Some(&own), standing, &silent, Some(9))
                .await
                .is_ok(),
            passes,
            "{standing:?}"
        );
    }
    assert_eq!(
        health_of(None, Standing::Running, &silent, None).await,
        Ok(())
    );
    let ping = health(HealthKind::Http, Some("/ping"));
    assert_eq!(
        health_of(Some(&ping), Standing::Running, &silent, None).await,
        Ok(()),
        "a service publishing no port is taken at the container's word"
    );
}

#[tokio::test]
async fn a_container_that_exits_fails_at_once_naming_its_status() {
    let silent = Fake::always(Answer::Silent);
    let started = std::time::Instant::now();
    assert_eq!(
        health_of(None, Standing::Exited(137), &silent, Some(9)).await,
        Err("it exited 137 before its health passed".to_owned())
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

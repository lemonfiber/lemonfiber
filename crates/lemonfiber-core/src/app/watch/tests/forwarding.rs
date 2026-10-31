use std::time::Duration;

use lemonfiber_fixtures::ports::Following;

use super::{supervise, Drive};
use crate::config::Settings;
use crate::ports::filesystem::Presence;
use lemonfiber_fixtures::http::Fake;

use crate::test_support::{a_context, env_without_password, guard};

#[tokio::test(start_paused = true)]
async fn a_watch_asks_the_guard_once_a_minute_while_the_volume_holds() {
    let http = Fake::by_path(vec![guard::forwarding(None)]);
    let env = env_without_password("watch-forwarding");
    let ctx = guard::keyed(
        a_context()
            .engine(std::sync::Arc::new(guard::running()))
            .clock(Following::started())
            .settings(Settings {
                env_file: Some(env.clone()),
                stack_dir: env.parent().map(|config| config.join("data").join("stack")),
                data_root: Some("/data".into()),
                port_forward: crate::config::PortForward {
                    enabled: true,
                    provider: None,
                },
                ..Settings::default()
            })
            .build()
            .with_http(http.clone()),
    );
    let every = Duration::from_secs(5);
    let holding = usize::try_from(crate::app::forwarding::EVERY.as_secs() * 2 / every.as_secs())
        .unwrap_or_default();
    let mut readings = vec![Presence::On(9); holding];
    readings.push(Presence::Gone);
    let report = supervise(&ctx, &Drive::playing(readings), &["dl".to_owned()], every).await;

    assert!(report.is_ok_and(|report| report.reason.contains("no longer present")));
    let forwarded = guard::operation("forwarded");
    assert_eq!(
        http.requests()
            .iter()
            .filter(|request| request.url.contains(forwarded))
            .count(),
        2
    );
}

#[tokio::test(start_paused = true)]
async fn a_watch_whose_guard_cannot_tell_goes_on_watching() {
    let ctx = a_context()
        .engine(std::sync::Arc::new(
            lemonfiber_fixtures::support::Reporting::absent(),
        ))
        .clock(Following::started())
        .settings(Settings {
            data_root: Some("/data".into()),
            port_forward: crate::config::PortForward {
                enabled: true,
                provider: None,
            },
            ..Settings::default()
        })
        .build();
    let readings = vec![Presence::On(9), Presence::On(9), Presence::Gone];
    let report = supervise(
        &ctx,
        &Drive::playing(readings),
        &["dl".to_owned()],
        Duration::from_secs(5),
    )
    .await;
    assert!(report.is_ok_and(|report| report.reason.contains("no longer present")));
}

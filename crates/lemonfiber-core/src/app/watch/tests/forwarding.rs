use std::time::Duration;

use lemonfiber_fixtures::ports::Following;

use super::{supervise, Drive};
use crate::config::Settings;
use crate::ports::filesystem::Presence;
use crate::test_support::{a_context, AskedGuard};

#[tokio::test(start_paused = true)]
async fn a_watch_asks_the_guard_once_a_minute_while_the_volume_holds() {
    let guard = AskedGuard::forwarding(None);
    let ctx = a_context()
        .engine(guard.clone())
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
    let every = Duration::from_secs(5);
    let holding = usize::try_from(crate::app::forwarding::EVERY.as_secs() * 2 / every.as_secs())
        .unwrap_or_default();
    let mut readings = vec![Presence::On(9); holding];
    readings.push(Presence::Gone);
    let report = supervise(&ctx, &Drive::playing(readings), &["dl".to_owned()], every).await;

    assert!(report.is_ok_and(|report| report.reason.contains("no longer present")));
    assert_eq!(guard.asked(), 2);
}

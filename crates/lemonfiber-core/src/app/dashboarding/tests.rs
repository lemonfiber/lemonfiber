use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lemonfiber_fixtures::http::{Answer as Reply, Fake};
use lemonfiber_fixtures::phones::{a_phone_key, a_phone_secret};
use lemonfiber_fixtures::scratch::Scratch;

use super::Early;
use crate::alert::{Alert, Moment, Notification};
use crate::app::surfacing::Answer;
use crate::app::{dispatch, Command, Ctx, Outcome, Surfacing};
use crate::config::Settings;
use crate::dashboard::Snapshot;
use crate::error::Severity;
use crate::ports::http::Http;
use crate::push::run::{asked, Asked};

fn waiting(name: &str) -> (Ctx, PathBuf) {
    let scratch = Scratch::named(&format!("dashboarding-{name}"));
    let dir = scratch.to_path_buf();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let ctx = crate::test_support::a_context()
        .holding(scratch)
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build();
    let registered = asked(
        &ctx,
        Asked::Register {
            name: "pixel".to_owned(),
            endpoint: "https://push.example/a".to_owned(),
            p256dh: a_phone_key(),
            auth: a_phone_secret(),
            owner: None,
        },
    );
    assert!(registered.is_ok(), "{registered:?}");
    (ctx, dir.join("config"))
}

fn kept(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Some((entry.path(), std::fs::read(entry.path()).ok()?)))
        .collect()
}

async fn gathered(ctx: &Ctx, asked: Surfacing) -> Option<Snapshot> {
    match dispatch(Command::Surface(asked), ctx).await {
        Ok(Outcome::Surfaced(Answer::Dashboard(snapshot))) => Some(*snapshot),
        _ => None,
    }
}

#[tokio::test]
async fn the_operators_gather_owes_each_new_alert_and_reads_the_last_gather_again() {
    let (ctx, dir) = waiting("operator");
    let before = kept(&dir);

    assert!(gathered(&ctx, Surfacing::Gather { now: false })
        .await
        .is_some());
    let first = kept(&dir);
    assert_ne!(
        first, before,
        "the first gather records what the phones were told"
    );

    assert!(gathered(&ctx, Surfacing::Gather { now: false })
        .await
        .is_some());
    assert!(gathered(&ctx, Surfacing::Gather { now: true })
        .await
        .is_some());
    assert_eq!(kept(&dir), first, "gathering again owes nothing new");
}

#[tokio::test]
async fn a_look_and_a_rehearsed_gather_owe_nobody_anything() {
    let (ctx, dir) = waiting("looking");
    let before = kept(&dir);

    assert!(gathered(&ctx, Surfacing::Glance { now: false })
        .await
        .is_some());
    assert!(gathered(&ctx, Surfacing::Glance { now: true })
        .await
        .is_some());
    let rehearsing = ctx.clone().rehearsing();
    assert!(gathered(&rehearsing, Surfacing::Gather { now: false })
        .await
        .is_some());

    assert_eq!(kept(&dir), before);
}

struct Heard(Mutex<usize>);

impl Early for Heard {
    fn first(&self, _snapshot: Snapshot) {
        if let Ok(mut heard) = self.0.lock() {
            *heard += 1;
        }
    }
}

#[tokio::test]
async fn the_first_wave_goes_where_the_surface_waits_for_it_or_nowhere() {
    let heard = Arc::new(Heard(Mutex::new(0)));
    let ctx = crate::test_support::a_context()
        .build()
        .with_early(Arc::clone(&heard) as Arc<dyn Early>);

    assert!(gathered(&ctx, Surfacing::Glance { now: false })
        .await
        .is_some());
    super::Unheard.first(
        crate::dashboard::run::waves(&ctx, None, |_| {})
            .await
            .snapshot,
    );

    assert!(
        heard.0.lock().is_ok_and(|heard| *heard <= 1),
        "at most once a gather"
    );
}

fn onset(check: &str) -> Alert {
    Alert {
        id: Some(Alert::named(check, 1)),
        check: check.to_owned(),
        kind: "service.down".to_owned(),
        notification: Notification::Problem,
        moment: Moment::Onset,
        severity: Severity::Warning,
        summary: "it broke".to_owned(),
        meaning: "nothing works".to_owned(),
        remedies: Vec::new(),
        affected: vec![check.to_owned()],
        exit: None,
    }
}

#[tokio::test]
async fn two_clones_of_a_run_send_a_delivery_still_under_way_once() {
    let distributor = Fake::distributing(vec![("", Reply::reply(201, ""))]);
    let (ctx, _dir) = waiting("clones");
    let ctx = ctx.with_http(Arc::clone(&distributor) as Arc<dyn Http>);
    let (one, other) = (ctx.clone(), ctx.clone());
    one.dashboarding.owing(&one, &[]);

    let new = [onset("service.sonarr")];
    one.dashboarding.owing(&one, &new);
    other.dashboarding.owing(&other, &new);

    let sent = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while distributor.octets().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(sent.is_ok(), "nothing was sent");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(distributor.octets().len(), 1, "sent once, not once a clone");
}

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::agreement::{EphemeralPrivateKey, ECDH_P256};
use ring::rand::SystemRandom;

use super::{any_waiting, asked, Asked, Waking};
use crate::alert::{Alert, Moment, Notification};
use crate::app::{Ctx, Outcome};
use crate::config::Settings;
use crate::error::codes::push::{
    NOT_A_PUSH_NAME, PUSH_KEYS_UNREADABLE, PUSH_NAME_TAKEN, PUSH_NOT_HTTPS, PUSH_NOT_REGISTERED,
};
use crate::error::Severity;
use crate::ports::http::{Http, Request, Response, Unreachable};
use crate::push::Kept;

/// An address sent to, with the headers and the bytes it was sent.
type Took = (String, Vec<(String, String)>, Vec<u8>);

/// A distributor answering `status`, holding what each request carried.
struct Distributor {
    /// What it answers.
    status: u16,
    /// Each message it took.
    took: Mutex<Vec<Took>>,
}

#[async_trait]
impl Http for Distributor {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        Err(Unreachable::once(&request.url, "only octets go to a phone"))
    }

    async fn send_octets(&self, request: &Request, octets: &[u8]) -> Result<Response, Unreachable> {
        if let Ok(mut took) = self.took.lock() {
            took.push((
                request.url.clone(),
                request.headers.clone(),
                octets.to_vec(),
            ));
        }
        Ok(Response {
            status: self.status,
            headers: Vec::new(),
            body: String::new(),
        })
    }
}

impl Distributor {
    /// How many messages it took.
    fn took(&self) -> usize {
        self.took.lock().map_or(0, |took| took.len())
    }
}

/// A machine keeping its files beneath a directory named for `name`, reaching `distributor`.
fn machine(name: &str, status: u16) -> (Ctx, Arc<Distributor>) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let distributor = Arc::new(Distributor {
        status,
        took: Mutex::new(Vec::new()),
    });
    let ctx = crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_http(Arc::clone(&distributor) as Arc<dyn Http>);
    (ctx, distributor)
}

/// A key a phone could hold, in base64url.
fn p256dh() -> String {
    EphemeralPrivateKey::generate(&ECDH_P256, &SystemRandom::new())
        .and_then(|key| key.compute_public_key())
        .map(|public| URL_SAFE_NO_PAD.encode(public.as_ref()))
        .unwrap_or_default()
}

/// A registration under `name`, at `endpoint`.
fn registering(name: &str, endpoint: &str) -> Asked {
    Asked::Register {
        name: name.to_owned(),
        endpoint: endpoint.to_owned(),
        p256dh: p256dh(),
        auth: URL_SAFE_NO_PAD.encode([9_u8; 16]),
    }
}

/// The code a request was refused with.
fn refused(ctx: &Ctx, asking: Asked) -> Option<crate::error::Code> {
    asked(ctx, asking).err().map(|problem| problem.code)
}

/// An alert of one check, going one way.
fn alert(check: &str, moment: Moment) -> Alert {
    Alert {
        id: Some(Alert::named(check, 1)),
        check: check.to_owned(),
        kind: "service.down".to_owned(),
        notification: Notification::Problem,
        moment,
        severity: Severity::Warning,
        summary: "it broke".to_owned(),
        meaning: "nothing works".to_owned(),
        remedies: Vec::new(),
        affected: vec![check.to_owned()],
        exit: None,
    }
}

/// What is listed by name.
fn names(ctx: &Ctx) -> Vec<String> {
    match asked(ctx, Asked::List) {
        Ok(Outcome::PushEndpoints(listing)) => {
            listing.endpoints.into_iter().map(|one| one.name).collect()
        }
        _ => Vec::new(),
    }
}

/// What the journal holds, as written.
fn journal(ctx: &Ctx) -> String {
    crate::app::targets::layout(ctx)
        .and_then(|paths| std::fs::read_to_string(paths.journal()).ok())
        .unwrap_or_default()
}

/// A registration is listed and journaled by name, and what it holds of the phone is not
/// answered with or journaled.
#[test]
fn a_registration_is_listed_and_journaled_without_its_keys() {
    let (ctx, _) = machine("push-registers", 201);
    let answered = asked(&ctx, registering("pixel", "https://push.example/abc"));
    assert!(matches!(answered, Ok(Outcome::PushEndpoint(ref one)) if one.name == "pixel"));
    assert_eq!(names(&ctx), ["pixel"]);

    let said = journal(&ctx);
    assert!(said.contains("pixel"), "{said}");
    assert!(
        !said.contains(&URL_SAFE_NO_PAD.encode([9_u8; 16])),
        "{said}"
    );
    assert!(!said.contains("push.example"), "{said}");
}

/// The record is where the layout says it is.
#[test]
fn the_record_is_where_the_layout_says() {
    let (ctx, _distributor) = machine("push-layout", 201);
    assert_eq!(
        crate::app::targets::layout(&ctx).map(|paths| paths.push_endpoints()),
        super::at(&ctx)
    );
}

/// Each thing that cannot be registered is refused by what is wrong with it.
#[test]
fn what_cannot_be_registered_is_refused_by_name() {
    let (ctx, _) = machine("push-refuses", 201);
    assert_eq!(
        refused(&ctx, registering("Not A Name", "https://push.example/a")),
        Some(NOT_A_PUSH_NAME)
    );
    for address in ["http://push.example/a", "not an address", "https://"] {
        assert_eq!(
            refused(&ctx, registering("pixel", address)),
            Some(PUSH_NOT_HTTPS),
            "{address}"
        );
    }
    assert_eq!(
        refused(
            &ctx,
            Asked::Register {
                name: "pixel".to_owned(),
                endpoint: "https://push.example/a".to_owned(),
                p256dh: "not a key".to_owned(),
                auth: "not a secret".to_owned(),
            }
        ),
        Some(PUSH_KEYS_UNREADABLE)
    );
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let taken = asked(&ctx, registering("pixel", "https://push.example/b"));
    assert!(
        taken.is_err_and(
            |problem| problem.code == PUSH_NAME_TAKEN && problem.summary.contains("pixel")
        )
    );
    assert_eq!(names(&ctx), ["pixel"]);
}

/// A removal takes the endpoint and is journaled; one nothing holds is refused.
#[test]
fn a_removal_takes_the_endpoint_and_one_nothing_holds_is_refused() {
    let (ctx, _) = machine("push-removes", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let removal = || Asked::Remove {
        name: "pixel".to_owned(),
    };

    let rehearsing = ctx.clone().rehearsing();
    assert!(matches!(
        asked(&rehearsing, removal()),
        Ok(Outcome::PushEndpoints(ref listing)) if listing.rehearsed && listing.endpoints.len() == 1
    ));
    assert!(asked(&ctx, removal()).is_ok());
    assert!(names(&ctx).is_empty());
    assert!(
        journal(&ctx).contains("push-endpoint-removed"),
        "{}",
        journal(&ctx)
    );

    assert_eq!(refused(&ctx, removal()), Some(PUSH_NOT_REGISTERED));
    assert_eq!(refused(&rehearsing, removal()), Some(PUSH_NOT_REGISTERED));
}

/// A rehearsed registration says what it would come to and keeps nothing, and is still
/// refused a name another holds.
#[test]
fn a_rehearsed_registration_keeps_nothing() {
    let (ctx, _) = machine("push-rehearses", 201);
    let rehearsing = ctx.clone().rehearsing();
    assert!(matches!(
        asked(&rehearsing, registering("pixel", "https://push.example/a")),
        Ok(Outcome::PushEndpoint(ref one)) if one.rehearsed
    ));
    assert!(names(&ctx).is_empty());

    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert_eq!(
        refused(&rehearsing, registering("pixel", "https://push.example/b")),
        Some(PUSH_NAME_TAKEN)
    );
}

/// Whether `settled` comes true soon.
async fn soon(settled: impl Fn() -> bool) -> bool {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !settled() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok()
}

/// What is owed each endpoint, as kept.
fn owed(ctx: &Ctx) -> Vec<usize> {
    super::at(ctx)
        .and_then(|path| super::read(&path).ok())
        .map(|kept| kept.endpoints.iter().map(|one| one.owed).collect())
        .unwrap_or_default()
}

/// What was said before the first reading is not sent, and each onset and each resolution
/// after it is sent once to every endpoint, as encrypted octets.
#[tokio::test]
async fn each_onset_and_resolution_is_sent_once_to_every_endpoint() {
    let (ctx, distributor) = machine("push-wakes", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert!(asked(&ctx, registering("tablet", "https://push.example/b")).is_ok());
    let ctx = Arc::new(ctx);
    let waking = Waking::default();

    let before = [alert("service.sonarr", Moment::Onset)];
    waking.owing(&ctx, &before);
    waking.owing(&ctx, &before);
    assert_eq!(owed(&ctx), [0, 0], "said before anybody was told");

    let onset = [
        alert("service.radarr", Moment::Onset),
        alert("service.sonarr", Moment::Onset),
    ];
    waking.owing(&ctx, &onset);
    assert!(soon(|| distributor.took() == 2).await);

    let resolved = [
        alert("service.radarr", Moment::Resolved),
        alert("service.radarr", Moment::Onset),
        alert("service.sonarr", Moment::Onset),
    ];
    waking.owing(&ctx, &resolved);
    assert!(soon(|| distributor.took() == 4).await);
    assert!(soon(|| owed(&ctx) == [0, 0]).await);

    let took = distributor
        .took
        .lock()
        .map(|took| took.clone())
        .unwrap_or_default();
    assert!(took.iter().all(|(_, headers, octets)| {
        headers
            .iter()
            .any(|(name, value)| name == "Content-Encoding" && value == "aes128gcm")
            && !octets.is_empty()
            && !String::from_utf8_lossy(octets).contains("service")
    }));
}

/// An endpoint its distributor says is gone is forgotten, journaled, and sent nothing more.
#[tokio::test]
async fn an_endpoint_its_distributor_says_is_gone_is_forgotten() {
    for status in [404, 410] {
        let (ctx, distributor) = machine(&format!("push-gone-{status}"), status);
        assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
        let ctx = Arc::new(ctx);
        let waking = Waking::default();
        waking.owing(&ctx, &[]);
        assert!(any_waiting(&ctx));

        waking.owing(
            &ctx,
            &[
                alert("service.radarr", Moment::Onset),
                alert("service.sonarr", Moment::Onset),
            ],
        );

        assert!(soon(|| !any_waiting(&ctx)).await, "{status}");
        assert!(names(&ctx).is_empty(), "{status}");
        assert_eq!(distributor.took(), 1, "{status}");
        assert!(journal(&ctx).contains("pixel"));
    }
}

/// An endpoint that answers anything else stays, each message tried once.
#[tokio::test]
async fn an_endpoint_that_answers_anything_else_stays() {
    let (ctx, distributor) = machine("push-stays", 500);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let ctx = Arc::new(ctx);
    let waking = Waking::default();
    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &[alert("service.sonarr", Moment::Onset)]);
    assert!(soon(|| distributor.took() == 1).await);
    assert!(soon(|| owed(&ctx) == [0]).await);
    assert_eq!(names(&ctx), ["pixel"]);
}

/// Nothing is owed or sent where waking is switched off.
#[tokio::test]
async fn nothing_is_sent_where_waking_is_switched_off() {
    let (mut ctx, distributor) = machine("push-offline", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert!(any_waiting(&ctx));
    ctx.settings.reaching = crate::config::Reaching::without(crate::config::REACH_WAKING_KEY);
    assert!(!any_waiting(&ctx));
    let ctx = Arc::new(ctx);
    let waking = Waking::default();
    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &[alert("service.sonarr", Moment::Onset)]);
    tokio::task::yield_now().await;
    assert_eq!(distributor.took(), 0);
    assert_eq!(owed(&ctx), [0]);
}

/// A transport on which a message hangs, but for one to an address saying it answers.
struct Hanging {
    /// How many messages reached an address that hangs.
    reached: AtomicUsize,
    /// What takes a message to an address that answers.
    answering: Arc<Distributor>,
}

#[async_trait]
impl Http for Hanging {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        Err(Unreachable::once(&request.url, "only octets go to a phone"))
    }

    async fn send_octets(&self, request: &Request, octets: &[u8]) -> Result<Response, Unreachable> {
        if request.url.contains("answers") {
            return self.answering.send_octets(request, octets).await;
        }
        self.reached.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

/// A machine whose transport hangs, with each of `endpoints` registered by name and address.
fn hanging_machine(name: &str, endpoints: &[(&str, &str)]) -> (Arc<Ctx>, Arc<Hanging>) {
    let (ctx, answering) = machine(name, 201);
    let hanging = Arc::new(Hanging {
        reached: AtomicUsize::new(0),
        answering,
    });
    let ctx = ctx.with_http(Arc::clone(&hanging) as Arc<dyn Http>);
    for (endpoint, address) in endpoints {
        assert!(asked(&ctx, registering(endpoint, address)).is_ok());
    }
    (Arc::new(ctx), hanging)
}

/// `count` alerts, each new.
fn alerts(count: usize) -> Vec<Alert> {
    (0..count)
        .map(|one| alert(&format!("service.{one}"), Moment::Onset))
        .collect()
}

/// What is owed an endpoint that hangs stops growing at the most a stream replays, and is
/// worked through one message per wait.
#[tokio::test(start_paused = true)]
async fn what_is_owed_behind_a_hanging_endpoint_is_bounded_and_worked_through() {
    let (ctx, hanging) = hanging_machine("push-bound", &[("pixel", "https://push.example/a")]);
    let waking = Waking::default();
    waking.owing(&ctx, &[]);

    let many = crate::alert::KEPT + 50;
    waking.owing(&ctx, &alerts(many));
    tokio::task::yield_now().await;
    assert_eq!(hanging.reached.load(Ordering::SeqCst), 1);
    assert_eq!(owed(&ctx), [crate::alert::KEPT - 1]);

    tokio::time::sleep(super::WAITED_FOR + Duration::from_millis(1)).await;
    assert_eq!(hanging.reached.load(Ordering::SeqCst), 2);
}

/// An endpoint that hangs holds up no other endpoint's messages.
#[tokio::test]
async fn an_endpoint_that_hangs_holds_up_no_other() {
    let (ctx, hanging) = hanging_machine(
        "push-apart",
        &[
            ("pixel", "https://push.example/hangs"),
            ("tablet", "https://push.example/answers"),
        ],
    );
    let waking = Waking::default();
    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &alerts(3));

    assert!(soon(|| hanging.answering.took() == 3).await);
    assert_eq!(hanging.reached.load(Ordering::SeqCst), 1);
}

/// A second reading while a delivery is under way starts no second delivery to that endpoint.
#[tokio::test]
async fn a_delivery_under_way_is_not_started_again() {
    let (ctx, hanging) = hanging_machine("push-once", &[("pixel", "https://push.example/a")]);
    let waking = Waking::default();
    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &alerts(1));
    assert!(soon(|| hanging.reached.load(Ordering::SeqCst) == 1).await);

    waking.owing(&ctx, &alerts(2));
    tokio::task::yield_now().await;
    assert_eq!(hanging.reached.load(Ordering::SeqCst), 1);
    assert_eq!(owed(&ctx), [1]);
}

/// What is still owed survives the run that owed it: a new run delivers it at its first
/// reading.
#[tokio::test]
async fn what_is_owed_is_delivered_by_the_next_run() {
    let (ctx, distributor) = machine("push-resumes", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let path = super::at(&ctx).unwrap_or_default();
    let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        kept.told = Some(Vec::new());
        for one in &mut kept.endpoints {
            one.owed = 2;
        }
        Ok(())
    });
    assert!(written.is_ok());

    Waking::default().owing(&Arc::new(ctx), &[]);

    assert!(soon(|| distributor.took() == 2).await);
}

/// An endpoint removed while it is owed messages is sent none of them.
#[test]
fn an_endpoint_removed_is_owed_nothing() {
    let (ctx, _distributor) = machine("push-removed-owed", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let path = super::at(&ctx).unwrap_or_default();
    assert!(asked(
        &ctx,
        Asked::Remove {
            name: "pixel".to_owned()
        }
    )
    .is_ok());
    assert!(super::next_owed(&path, "pixel").is_none());
}

/// Keys a record no longer holds readably send nothing, and the endpoint stays.
#[tokio::test]
async fn keys_that_cannot_be_read_send_nothing() {
    let (ctx, distributor) = machine("push-broken", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let path = super::at(&ctx).unwrap_or_default();
    let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        for one in &mut kept.endpoints {
            one.p256dh = "broken".to_owned();
        }
        Ok(())
    });
    assert!(written.is_ok());
    let ctx = Arc::new(ctx);
    let waking = Waking::default();
    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &[alert("service.sonarr", Moment::Onset)]);

    assert!(soon(|| owed(&ctx) == [0]).await);
    assert_eq!(distributor.took(), 0);
    assert_eq!(names(&ctx), ["pixel"]);
}

/// What was asked of the endpoints prints no key and no address.
#[test]
fn what_was_asked_prints_no_key_and_no_address() {
    let asking = Asked::Register {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/the-address".to_owned(),
        p256dh: "the-public-key".to_owned(),
        auth: "the-secret".to_owned(),
    };
    let said = format!("{asking:?}");
    assert!(said.contains("pixel"), "{said}");
    for withheld in ["the-address", "the-public-key", "the-secret"] {
        assert!(!said.contains(withheld), "{said}");
    }
    assert_eq!(format!("{:?}", Asked::List), "List");
    assert!(format!(
        "{:?}",
        Asked::Remove {
            name: "pixel".to_owned()
        }
    )
    .contains("pixel"));
}

/// A machine keeping no configuration has nowhere to keep an endpoint.
#[test]
fn a_machine_keeping_no_configuration_keeps_no_endpoint() {
    let ctx = crate::test_support::a_context().build();
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_err());
    assert!(asked(&ctx, Asked::List).is_err());
    assert!(asked(
        &ctx,
        Asked::Remove {
            name: "pixel".to_owned()
        }
    )
    .is_err());
}

/// A removal the journal cannot record is refused, and the endpoint stays.
#[test]
fn a_removal_the_journal_cannot_record_is_refused() {
    let (ctx, _distributor) = machine("push-unjournalled", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&journal);
    assert!(std::fs::create_dir_all(&journal).is_ok());

    let removal = Asked::Remove {
        name: "pixel".to_owned(),
    };
    assert!(asked(&ctx, removal).is_err());
    assert_eq!(names(&ctx), ["pixel"]);
}

/// A record that cannot be read refuses every request, rehearsed or not, and is left as it
/// was.
#[test]
fn a_record_that_cannot_be_read_refuses_every_request() {
    let (ctx, _distributor) = machine("push-unreadable", 201);
    let path = super::at(&ctx).unwrap_or_default();
    assert!(std::fs::write(&path, "{ not a record").is_ok());
    let removal = || Asked::Remove {
        name: "pixel".to_owned(),
    };

    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_err());
    assert!(asked(&ctx, Asked::List).is_err());
    assert!(asked(&ctx, removal()).is_err());
    assert!(asked(&ctx.clone().rehearsing(), removal()).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_default(),
        "{ not a record"
    );
}

/// Removing one endpoint of two keeps what the other has been sent, so it is not sent
/// it again.
#[test]
fn removing_one_endpoint_of_two_keeps_what_was_sent() {
    let (ctx, _distributor) = machine("push-one-of-two", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert!(asked(&ctx, registering("tablet", "https://push.example/b")).is_ok());
    let path = super::at(&ctx).unwrap_or_default();
    let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        kept.told = Some(vec!["service.sonarr#1 started".to_owned()]);
        Ok(())
    });
    assert!(written.is_ok());

    assert!(asked(
        &ctx,
        Asked::Remove {
            name: "pixel".to_owned()
        }
    )
    .is_ok());

    let kept = super::read(&path).unwrap_or_default();
    assert_eq!(kept.told, Some(vec!["service.sonarr#1 started".to_owned()]));
    assert_eq!(names(&ctx), ["tablet"]);
}

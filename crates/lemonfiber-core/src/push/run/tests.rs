use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::agreement::{EphemeralPrivateKey, ECDH_P256};
use ring::rand::SystemRandom;

use super::{any_waiting, asked, woken, Asked};
use crate::alert::{Alert, Moment, Notification};
use crate::app::{Ctx, Outcome};
use crate::config::Settings;
use crate::error::codes::push::{
    NOT_A_PUSH_NAME, PUSH_KEYS_UNREADABLE, PUSH_NAME_TAKEN, PUSH_NOT_HTTPS, PUSH_NOT_REGISTERED,
};
use crate::error::Severity;
use crate::ports::http::{Http, Request, Response, Unreachable};

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

/// What was said before the first reading is not sent, and each onset and each resolution
/// after it is sent once to every endpoint, as encrypted octets.
#[tokio::test]
async fn each_onset_and_resolution_is_sent_once_to_every_endpoint() {
    let (ctx, distributor) = machine("push-wakes", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert!(asked(&ctx, registering("tablet", "https://push.example/b")).is_ok());

    let before = [alert("service.sonarr", Moment::Onset)];
    woken(&ctx, &before).await;
    assert_eq!(distributor.took(), 0, "said before anybody was told");

    woken(&ctx, &before).await;
    assert_eq!(distributor.took(), 0, "still the same alert");

    let onset = [
        alert("service.radarr", Moment::Onset),
        alert("service.sonarr", Moment::Onset),
    ];
    woken(&ctx, &onset).await;
    assert_eq!(distributor.took(), 2);

    let resolved = [
        alert("service.radarr", Moment::Resolved),
        alert("service.radarr", Moment::Onset),
        alert("service.sonarr", Moment::Onset),
    ];
    woken(&ctx, &resolved).await;
    assert_eq!(distributor.took(), 4);

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

/// An endpoint its distributor says is gone is forgotten and journaled.
#[tokio::test]
async fn an_endpoint_its_distributor_says_is_gone_is_forgotten() {
    for status in [404, 410] {
        let (ctx, _distributor) = machine(&format!("push-gone-{status}"), status);
        assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
        woken(&ctx, &[]).await;
        assert!(any_waiting(&ctx));

        woken(&ctx, &[alert("service.sonarr", Moment::Onset)]).await;

        assert!(names(&ctx).is_empty(), "{status}");
        assert!(!any_waiting(&ctx));
        assert!(journal(&ctx).contains("pixel"));
    }
}

/// An endpoint that answers anything else, or nothing, stays.
#[tokio::test]
async fn an_endpoint_that_answers_anything_else_stays() {
    let (ctx, distributor) = machine("push-stays", 500);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    woken(&ctx, &[]).await;
    woken(&ctx, &[alert("service.sonarr", Moment::Onset)]).await;
    assert_eq!(distributor.took(), 1);
    assert_eq!(names(&ctx), ["pixel"]);
}

/// Nothing is read or sent where nothing may leave the machine.
#[tokio::test]
async fn nothing_is_sent_where_nothing_may_leave() {
    let (mut ctx, distributor) = machine("push-offline", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());
    assert!(any_waiting(&ctx));
    woken(&ctx, &[]).await;

    ctx.settings.reaching = crate::config::Reaching::without(crate::config::REACH_WAKING_KEY);
    assert!(!any_waiting(&ctx));
    woken(&ctx, &[alert("service.sonarr", Moment::Onset)]).await;
    assert_eq!(distributor.took(), 0);
}

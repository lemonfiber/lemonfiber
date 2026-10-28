//! Replacing the media server's internal certificate with the request gate as the one
//! holder of a media server credential: every step that cannot be taken stops the
//! replacement where it stands, with the gate still handed every certificate kept.

use lemonfiber_fixtures::http::Answer;

use super::*;
use crate::certificate::internal::{self, Internal, Replacing, NEXT};
use crate::credential::Settled;
use crate::seed::run::replacing::{self, Stack};

/// The answer to `serve_tls` that says the media server serves it now.
fn now() -> Answer {
    media::taken()
}

/// The answer to `serve_tls` that says the media server serves it once it restarts.
fn on_restart() -> Answer {
    Answer::reply(205, "")
}

/// Where an adapter says whether its upstream answers.
const READY: &str = "/lemonfiber/adapter/v1/ready";

/// A media server whose gate holds its key, answering `serve_tls` as `serving` does in
/// turn and `ready` as `ready` says.
fn answering_in_turn(serving: Vec<Answer>, ready: bool) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (media::serve("declared"), vec![media::answered(&declared())]),
        (media::serve("filed_as"), vec![media::answered(&["held"])]),
        (media::serve("revoke"), vec![media::taken()]),
        (media::serve("serve_tls"), serving),
        (
            READY,
            vec![Answer::reply(200, if ready { "true" } else { "false" })],
        ),
    ])
}

/// The same, answering `serve_tls` as `serving` does every time.
fn answering(serving: Answer, ready: bool) -> Arc<Fake> {
    answering_in_turn(vec![serving], ready)
}

/// A machine whose gate holds its routes and whose media server keeps a certificate,
/// answering over `http`; with that certificate.
fn machine(name: &str, http: Arc<Fake>) -> (Ctx, std::path::PathBuf, Internal) {
    machine_administered(name, true, http)
}

/// The same, with the media server administered where `administered` says.
fn machine_administered(
    name: &str,
    administered: bool,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf, Internal) {
    let (mut ctx, at) = gate_ctx(name, administered, CURATORS, Some("held"), http);
    ctx.settings.stack_dir = Some(at.join("stack"));
    let made = internal::kept_or_made(&lemonfiber_adapters::Disk, &config(&at), "jellyfin");
    let Ok(made) = made else {
        unreachable!("the internal certificate is made: {made:?}");
    };
    (ctx, at, made)
}

/// The media server's configuration directory, under `project`.
fn config(project: &std::path::Path) -> std::path::PathBuf {
    project.join("config").join("jellyfin")
}

/// Every certificate kept under `project`.
fn kept(project: &std::path::Path) -> Replacing {
    internal::replacing(&lemonfiber_adapters::Disk, &config(project)).unwrap_or_default()
}

/// The SHA-256s the gate's route to the media server holds.
fn gate_pins(project: &std::path::Path) -> Vec<String> {
    routes(project)
        .and_then(|held| held.route("jellyfin").cloned())
        .map(|route| route.pins.listed().to_vec())
        .unwrap_or_default()
}

/// The SHA-256s of `kept`, in the order a holder is handed them.
fn both(one: &Internal, other: &Internal) -> Vec<String> {
    let mut pins = vec![one.fingerprint.clone(), other.fingerprint.clone()];
    pins.sort();
    pins
}

/// The stack written at `project`: its services, who fills each ask, and its media server.
fn parts(
    project: &std::path::Path,
) -> (
    Vec<lemonfiber_manifest::Service>,
    crate::wiring::Fillers,
    crate::app::targets::MediaServer,
) {
    let fillers = routed_at(stack(true), project);
    let Some(server) = crate::app::targets::MediaServer::of(&fillers) else {
        unreachable!("the moved media server fills the identity source");
    };
    (stack(true), fillers, server)
}

/// Replace the certificate on `ctx` with the stack written at `project`.
async fn replace(ctx: &Ctx, project: &std::path::Path) -> Vec<crate::seed::Wiring> {
    let (services, fillers, server) = parts(project);
    replacing::replaced(
        ctx,
        Stack {
            services: &services,
            fillers: &fillers,
            project: Some(project),
        },
        &server,
    )
    .await
}

/// Take the replacement under way on `ctx` as far as it goes, as the seed does.
async fn advance(ctx: &Ctx, project: &std::path::Path) -> Vec<crate::seed::Wiring> {
    let (services, fillers, server) = parts(project);
    replacing::advanced(
        ctx,
        Stack {
            services: &services,
            fillers: &fillers,
            project: Some(project),
        },
        Some(&server),
    )
    .await
}

/// Whether any of `wirings` failed, saying `said`.
fn failed_saying(wirings: &[crate::seed::Wiring], said: &str) -> bool {
    wirings
        .iter()
        .any(|one| matches!(&one.state, State::Failed { detail } if detail.contains(said)))
}

/// Every replacement journalled under `ctx`.
fn journalled(ctx: &Ctx) -> Vec<crate::journal::Kind> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter(|change| change.operation == replacing::OPERATION)
                .map(|change| change.kind.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(start_paused = true)]
async fn a_replacement_the_gate_holds_is_served_journalled_and_the_old_one_withdrawn() {
    let http = answering(now(), true);
    let (ctx, at, old) = machine("rotating-landed", http.clone());

    let wirings = replace(&ctx, &at).await;

    let after = kept(&at);
    let Some(new) = after.current.clone() else {
        unreachable!("a certificate is in force: {wirings:?}");
    };
    assert_ne!(new, old);
    assert_eq!((after.next, after.previous), (None, None), "{wirings:?}");
    assert_eq!(gate_pins(&at), vec![new.fingerprint.clone()]);
    assert_eq!(
        journalled(&ctx),
        vec![crate::journal::Kind::Certified {
            service: "jellyfin".to_owned(),
            previous: Some(old.fingerprint),
            current: new.fingerprint,
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn a_replacement_the_gate_cannot_be_handed_is_never_served() {
    let http = answering(now(), true);
    let (ctx, at, old) = machine("rotating-unhanded", http.clone());
    let (services, fillers, server) = parts(&at);

    let wirings = replacing::replaced(
        &ctx,
        Stack {
            services: &services,
            fillers: &fillers,
            project: None,
        },
        &server,
    )
    .await;

    assert!(
        failed_saying(&wirings, "not yet hold the replacement"),
        "{wirings:?}"
    );
    let after = kept(&at);
    assert_eq!(
        after.current,
        Some(old.clone()),
        "the old one is still served"
    );
    assert!(after.next.is_some(), "the replacement waits");
    assert!(!http.asked_for(media::serve("serve_tls")));
    assert!(journalled(&ctx).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_replacement_that_cannot_be_journalled_is_never_served() {
    let http = answering(now(), true);
    let (ctx, at, old) = machine("rotating-unjournalled", http.clone());
    let mut nowhere = ctx.clone();
    nowhere.settings.stack_dir = None;

    let unrecorded = replace(&nowhere, &at).await;

    assert!(
        failed_saying(&unrecorded, "nowhere to journal"),
        "{unrecorded:?}"
    );
    let journal = crate::app::targets::layout(&ctx).map(|paths| paths.journal());
    let Some(journal) = journal else {
        unreachable!("the machine has a layout");
    };
    let _ = std::fs::remove_file(&journal);
    let _ = std::fs::create_dir_all(&journal);

    let wirings = replace(&ctx, &at).await;

    assert!(
        failed_saying(&wirings, "could not be journalled"),
        "{wirings:?}"
    );
    let after = kept(&at);
    assert_eq!(after.current, Some(old.clone()));
    let Some(next) = after.next else {
        unreachable!("the replacement waits");
    };
    assert_eq!(gate_pins(&at), both(&old, &next), "the gate holds both");
    assert!(!http.asked_for(media::serve("serve_tls")));
}

#[tokio::test(start_paused = true)]
async fn a_media_server_that_does_not_start_again_keeps_both_handed() {
    let http = answering(on_restart(), true);
    let (mut ctx, at, old) = machine("rotating-unrestarted", http.clone());
    ctx.seams.runner = Arc::new(lemonfiber_fixtures::support::Scripted(Err(
        crate::ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));

    let wirings = replace(&ctx, &at).await;

    assert!(
        failed_saying(&wirings, "starting it again failed"),
        "{wirings:?}"
    );
    let after = kept(&at);
    let Some(new) = after.current.clone() else {
        unreachable!("the replacement is in place");
    };
    assert_eq!(
        after.previous,
        Some(old.clone()),
        "the old one is not withdrawn"
    );
    assert_eq!(gate_pins(&at), both(&old, &new));
}

#[tokio::test(start_paused = true)]
async fn a_media_server_that_never_presents_the_new_one_keeps_both_handed() {
    let http = answering_in_turn(vec![now(), on_restart()], true);
    let (ctx, at, old) = machine("rotating-unpresented", http.clone());

    let wirings = replace(&ctx, &at).await;

    assert!(
        failed_saying(&wirings, "Jellyfin does not yet present it"),
        "{wirings:?}"
    );
    let after = kept(&at);
    let Some(new) = after.current.clone() else {
        unreachable!("the replacement is in place");
    };
    assert_eq!(
        after.previous,
        Some(old.clone()),
        "the old one is not withdrawn"
    );
    assert_eq!(gate_pins(&at), both(&old, &new));
}

#[tokio::test(start_paused = true)]
async fn a_media_server_that_is_not_ready_keeps_both_handed() {
    let http = answering(now(), false);
    let (ctx, at, old) = machine("rotating-unready", http.clone());

    let wirings = replace(&ctx, &at).await;

    assert!(
        failed_saying(&wirings, "Jellyfin does not yet present it"),
        "{wirings:?}"
    );
    let after = kept(&at);
    let Some(new) = after.current.clone() else {
        unreachable!("the replacement is in place");
    };
    assert_eq!(after.previous, Some(old.clone()));
    assert_eq!(gate_pins(&at), both(&old, &new));
}

#[tokio::test(start_paused = true)]
async fn a_certificate_kept_that_cannot_be_read_stops_everything_and_hands_nothing_less() {
    let http = answering(now(), true);
    let (ctx, at, old) = machine("rotating-unread", http.clone());
    let _ = seeded(&ctx, &http, true, &at).await;
    assert_eq!(gate_pins(&at), vec![old.fingerprint.clone()]);
    let _ = std::fs::write(config(&at).join(NEXT), "not a certificate");

    let (state, _) = seeded(&ctx, &http, true, &at).await;
    let wirings = replace(&ctx, &at).await;
    let advanced = advance(&ctx, &at).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert!(failed_saying(&wirings, "does not read"), "{wirings:?}");
    assert!(failed_saying(&advanced, "does not read"), "{advanced:?}");
    assert_eq!(
        gate_pins(&at),
        vec![old.fingerprint],
        "the gate keeps what it holds"
    );
    assert!(!http.asked_for(media::serve("serve_tls")));
}

/// The certificate's line in the inventory of the machine at `project`.
fn line(ctx: &Ctx, project: &std::path::Path) -> Option<crate::credential::Held> {
    let services = stack(true);
    let fillers = routed_at(stack(true), project);
    crate::app::credentials::certifying::held(
        ctx,
        Stack {
            services: &services,
            fillers: &fillers,
            project: Some(project),
        },
    )
}

/// What replacing the certificate on the operator's ask came to, answering `agreement`.
async fn rotated(
    ctx: &Ctx,
    project: &std::path::Path,
    agreement: Option<&str>,
) -> Result<crate::credential::Rotation, Box<crate::error::Problem>> {
    let services = stack(true);
    let fillers = routed_at(stack(true), project);
    let Some(held) = line(ctx, project) else {
        unreachable!("the media server's certificate is a line of the inventory");
    };
    crate::app::credentials::certifying::rotate(
        ctx,
        &held,
        Stack {
            services: &services,
            fillers: &fillers,
            project: Some(project),
        },
        agreement,
    )
    .await
}

#[tokio::test(start_paused = true)]
async fn the_certificate_is_listed_by_the_sha256_in_force_and_says_how_it_stands() {
    let http = answering(now(), true);
    let (ctx, at) = gate_ctx("rotating-listed", true, CURATORS, Some("held"), http);
    let absent = line(&ctx, &at);
    assert_eq!(
        absent
            .as_ref()
            .map(|held| (held.state, held.fingerprint.clone())),
        Some((crate::credential::State::Absent, None))
    );
    assert_eq!(
        absent.map(|held| held.consumers),
        Some(vec!["the request gate".to_owned()])
    );

    let Ok(made) = internal::kept_or_made(&lemonfiber_adapters::Disk, &config(&at), "jellyfin")
    else {
        unreachable!("a certificate is made");
    };
    let active = line(&ctx, &at);
    assert_eq!(
        active.map(|held| (held.state, held.fingerprint, held.advisory)),
        Some((
            crate::credential::State::Active,
            Some(made.fingerprint.clone()),
            None
        ))
    );

    let _ = internal::staged(&lemonfiber_adapters::Disk, &config(&at), "jellyfin");
    let rotating = line(&ctx, &at);
    assert!(
        rotating.is_some_and(|held| held.state == crate::credential::State::Rotating
            && held
                .advisory
                .is_some_and(|said| said.contains("the request gate is handed")))
    );

    let _ = std::fs::write(config(&at).join(NEXT), "not a certificate");
    let invalid = line(&ctx, &at);
    assert!(
        invalid.is_some_and(|held| held.state == crate::credential::State::Invalid
            && held.fingerprint.is_none()
            && held
                .advisory
                .is_some_and(|said| said.contains("nothing is replaced")))
    );
}

#[tokio::test(start_paused = true)]
async fn a_replacement_is_read_first_and_carried_out_on_the_offer_it_named() {
    let http = answering(now(), true);
    let (ctx, at, old) = machine("rotating-offered", http.clone());

    let read = rotated(&ctx, &at, None).await;
    let Ok(read) = read else {
        unreachable!("a reading is answered");
    };
    assert!(read.rehearsed(), "{read:?}");
    assert!(read
        .settled
        .detail()
        .is_some_and(|said| said.contains("may restart")));
    let Some(offer) = read.offer.clone() else {
        unreachable!("the reading names an offer");
    };
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    assert!(rotated(&rehearsing, &at, Some(&offer))
        .await
        .is_ok_and(|rehearsed| rehearsed.rehearsed()));
    assert_eq!(kept(&at).next, None, "nothing is made by a reading");

    let moved = rotated(&ctx, &at, Some("0-0-0")).await;
    assert_eq!(
        moved.err().map(|problem| problem.code),
        Some(crate::error::codes::cred::REPLACEMENT_MOVED)
    );

    let landed = rotated(&ctx, &at, Some(&offer)).await;
    let Ok(landed) = landed else {
        unreachable!("the replacement is carried out");
    };
    assert!(
        matches!(landed.settled, Settled::Replaced { .. }),
        "{landed:?}"
    );
    assert_eq!(
        landed.consumers,
        vec![crate::credential::Propagation::updated("the request gate")]
    );
    assert_ne!(kept(&at).current, Some(old));
}

#[tokio::test(start_paused = true)]
async fn a_replacement_that_stops_part_way_is_never_reported_as_landed() {
    let http = answering(on_restart(), false);
    let (mut ctx, at, _) = machine("rotating-stopped", http);
    ctx.seams.runner = Arc::new(lemonfiber_fixtures::support::Scripted(Err(
        crate::ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));
    let offer = rotated(&ctx, &at, None)
        .await
        .ok()
        .and_then(|read| read.offer)
        .unwrap_or_default();

    let served_old = rotated(&ctx, &at, Some(&offer)).await;
    assert!(
        served_old.as_ref().is_ok_and(|one| matches!(
            &one.settled,
            Settled::ReplacedUnproven { detail } if detail.contains("starting it again failed")
        )),
        "{served_old:?}"
    );

    let mut nowhere = ctx.clone();
    nowhere.settings.stack_dir = None;
    let _ = internal::withdrawn(&lemonfiber_adapters::Disk, &config(&at));
    let offer = rotated(&nowhere, &at, None)
        .await
        .ok()
        .and_then(|read| read.offer)
        .unwrap_or_default();
    let unserved = rotated(&nowhere, &at, Some(&offer)).await;
    assert!(
        unserved.as_ref().is_ok_and(|one| matches!(
            &one.settled,
            Settled::Unproven { detail } if detail.contains("nowhere to journal")
        )),
        "{unserved:?}"
    );

    let _ = std::fs::write(config(&at).join(NEXT), "not a certificate");
    let unread = rotated(&nowhere, &at, Some(&offer)).await;
    assert!(
        unread.as_ref().is_ok_and(|one| matches!(
            &one.settled,
            Settled::Unproven { detail } if detail.contains("does not read")
        )),
        "{unread:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stack_without_a_media_server_has_no_certificate_to_replace() {
    let (ctx, at) = gate_ctx(
        "rotating-serverless",
        true,
        CURATORS,
        None,
        answering(now(), true),
    );
    let Some(held) = line(&ctx, &at) else {
        unreachable!("the media server's certificate is a line of the inventory");
    };
    let nothing = crate::wiring::Fillers::default();
    let services = stack(true);
    let stopped = crate::app::credentials::certifying::rotate(
        &ctx,
        &held,
        Stack {
            services: &services,
            fillers: &nothing,
            project: Some(&at),
        },
        None,
    )
    .await;
    assert!(stopped.is_ok_and(|one| matches!(
        &one.settled,
        Settled::Unproven { detail } if detail.contains("no media server")
    )));
    assert!(crate::app::credentials::certifying::held(
        &ctx,
        Stack {
            services: &services,
            fillers: &nothing,
            project: Some(&at),
        },
    )
    .is_none());
}

#[tokio::test(start_paused = true)]
async fn a_media_server_lemonfiber_writes_no_certificate_for_has_none_replaced() {
    let (ctx, at, old) = machine("rotating-unconfined", answering(now(), true));
    let (services, fillers, mut server) = parts(&at);
    server.served.confined_to = None;
    let stack = Stack {
        services: &services,
        fillers: &fillers,
        project: Some(&at),
    };

    assert!(replacing::replaced(&ctx, stack, &server).await.is_empty());
    assert!(replacing::advanced(&ctx, stack, Some(&server))
        .await
        .is_empty());
    assert!(replacing::advanced(&ctx, stack, None).await.is_empty());
    assert_eq!(kept(&at).current, Some(old));
}

#[tokio::test(start_paused = true)]
async fn a_media_server_that_cannot_be_asked_to_serve_it_goes_on_serving_the_old_one() {
    let (ctx, at, old) = machine("rotating-unplaced", answering(now(), true));
    let (services, fillers, mut server) = parts(&at);
    server.served.config_at = None;
    let stack = Stack {
        services: &services,
        fillers: &fillers,
        project: Some(&at),
    };

    let unplaced = replacing::replaced(&ctx, stack, &server).await;

    assert!(
        failed_saying(&unplaced, "says nowhere its container holds"),
        "{unplaced:?}"
    );
    assert_eq!(kept(&at).current, Some(old.clone()));

    let http = answering(now(), true);
    let (ctx, at, old) = machine_administered("rotating-unadministered", false, http.clone());

    let unasked = replace(&ctx, &at).await;

    assert!(
        failed_saying(&unasked, "cannot be asked over its contract"),
        "{unasked:?}"
    );
    assert_eq!(kept(&at).current, Some(old));
    assert!(!http.asked_for(media::serve("serve_tls")));
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn a_certificate_that_cannot_be_withdrawn_stays_handed_beside_the_new_one() {
    use std::os::unix::fs::PermissionsExt as _;
    let (mut ctx, at, old) = machine("rotating-unwithdrawn", answering(now(), false));
    let _ = replace(&ctx, &at).await;
    ctx.seams.http = answering(now(), true);
    let directory = internal::path(&config(&at))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500));

    let wirings = advance(&ctx, &at).await;

    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    assert!(
        wirings
            .iter()
            .any(|one| matches!(one.state, State::Failed { .. })),
        "{wirings:?}"
    );
    let after = kept(&at);
    let Some(new) = after.current.clone() else {
        unreachable!("the replacement is in place");
    };
    assert_eq!(after.previous, Some(old.clone()));
    assert_eq!(gate_pins(&at), both(&old, &new));
}

#[tokio::test(start_paused = true)]
async fn a_replacement_with_nobody_to_hand_it_says_so() {
    let (_, at, _) = machine("rotating-unheld", answering(now(), true));
    let (_, fillers, server) = parts(&at);
    let alone = stack(false);

    let said = replacing::rehearsed(
        Stack {
            services: &alone,
            fillers: &fillers,
            project: Some(&at),
        },
        &server,
    );

    assert!(
        said.contains("no service holds a media server credential"),
        "{said}"
    );
}

/// A transport that writes over the certificate at `spoiled` once the media server is
/// asked to serve one, as a disk that fails part-way through a replacement leaves it.
struct Spoiling {
    inner: Arc<Fake>,
    spoiled: std::path::PathBuf,
}

#[async_trait::async_trait]
impl crate::ports::http::Http for Spoiling {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        if request.url.ends_with(&media::serve("serve_tls")) {
            let _ = std::fs::write(&self.spoiled, "not a certificate");
        }
        self.inner.send(request).await
    }
}

#[tokio::test(start_paused = true)]
async fn a_replacement_whose_outcome_cannot_be_read_back_is_never_reported_as_landed() {
    let http = answering(now(), true);
    let (mut ctx, at, _) = machine("rotating-spoiled", http.clone());
    let offer = rotated(&ctx, &at, None)
        .await
        .ok()
        .and_then(|read| read.offer)
        .unwrap_or_default();
    ctx.seams.http = Arc::new(Spoiling {
        inner: http,
        spoiled: internal::path(&config(&at)),
    });

    let spoiled = rotated(&ctx, &at, Some(&offer)).await;

    assert!(
        spoiled.as_ref().is_ok_and(|one| matches!(
            &one.settled,
            Settled::Unproven { detail } if detail.contains("does not read")
        )),
        "{spoiled:?}"
    );
}

//! The stream guard, applied the envelope it enforces over its contract, and the pass
//! service told the media server's live-stream paths.

use super::super::enveloping::{seed_envelope, ENVELOPE, LIVE};
use super::*;
use crate::pass::{File, Live, LivePath, PINNED, SERVICE};
use crate::project::entry::CONFIGURATION_TREE;
use crate::test_support::media;

/// The guard's service, brought by a plugin speaking `stream.guard`.
const GUARD: &str = "stream-guard";

/// Where the guard is asked what it enforces.
fn guarding() -> &'static str {
    path(
        lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
        "guarding",
    )
}

/// Where the guard is handed what it enforces.
fn guard() -> &'static str {
    path(
        lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
        "guard",
    )
}

fn path(capability: &str, operation: &str) -> &'static str {
    Box::leak(lemonfiber_contract::path(capability, 1, operation).into_boxed_str())
}

/// A machine with the media server moved into its plugin and a plugin's guard in front
/// of it, both reached on loopback and keyed, answering as `http` does, rehearsing where
/// `dry_run`; answers where the project is.
fn machine(name: &str, http: Arc<Fake>, dry_run: bool) -> (Ctx, std::path::PathBuf) {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::contracted_context(&project, GUARD, true)
        .over(crate::test_support::embedded())
        .build()
        .with_http(http);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    ctx.dry_run = dry_run;
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    let adapter = crate::plugin::key_file(&directory, media::ADAPTER);
    let _ = std::fs::create_dir_all(adapter.parent().unwrap_or(&adapter));
    let _ = std::fs::write(&adapter, media::KEY);
    let guard_key = crate::plugin::key_file(&directory, GUARD);
    let _ = std::fs::create_dir_all(guard_key.parent().unwrap_or(&guard_key));
    let _ = std::fs::write(&guard_key, crate::test_support::CONTRACTED_KEY);
    (ctx, directory)
}

/// `installed` speaking no contract at all.
fn silenced(mut installed: crate::plugin::Installed) -> crate::plugin::Installed {
    for placed in &mut installed.services {
        placed.speaks.clear();
    }
    installed
}

/// The media server's plugin as `media` and the guard's, written at `project`, the guard
/// speaking no contract where `silent`.
fn unspoken(
    project: &std::path::Path,
    media: Option<crate::plugin::Installed>,
    silent: bool,
) -> crate::wiring::Fillers {
    let guard = crate::test_support::contracted(
        "guarding",
        GUARD,
        lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
    );
    let installed: Vec<crate::plugin::Installed> = media
        .into_iter()
        .chain(std::iter::once(if silent {
            silenced(guard)
        } else {
            guard
        }))
        .collect();
    crate::test_support::stack_fillers(Vec::new(), &installed, Some(project), &[])
}

/// The media server's plugin and the guard's, written at `project`.
fn fillers(project: &std::path::Path) -> crate::wiring::Fillers {
    unspoken(project, media::moved(), false)
}

/// `project` running the pass service, its image pinned.
fn passing(project: &std::path::Path) {
    let _ = std::fs::create_dir_all(project);
    let _ = std::fs::write(
        project.join(PINNED),
        crate::pass::pin(
            crate::stack::RUNNING,
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        ),
    );
}

/// Where the pass service's live-stream paths sit beneath `project`.
fn live_at(project: &std::path::Path) -> std::path::PathBuf {
    project
        .join(CONFIGURATION_TREE)
        .join(SERVICE)
        .join(File::Live.name())
}

/// A media server declaring `declared`, and a guard enforcing `held` and taking whatever
/// it is handed.
fn declaring(declared: &crate::ports::service::Declared, held: &[Answer]) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (media::serve("declared"), vec![media::answered(declared)]),
        (guarding(), held.to_vec()),
        (guard(), vec![Answer::reply(204, "")]),
    ])
}

/// A media server declaring what a guard can enforce, and a guard enforcing `held` and
/// taking whatever it is handed.
fn answering(held: &[Answer]) -> Arc<Fake> {
    declaring(&lemonfiber_fixtures::declaring::enforceable(), held)
}

/// The connection named `name` among `wirings`, by its state.
fn state<'a>(wirings: &'a [Wiring], name: &str) -> Option<&'a State> {
    wirings
        .iter()
        .find(|wiring| wiring.connection == name)
        .map(|wiring| &wiring.state)
}

/// Every envelope the guard was handed.
fn envelopes(http: &Fake) -> Vec<serde_json::Value> {
    http.requests()
        .iter()
        .filter(|request| request.url.ends_with(guard()))
        .filter_map(|request| {
            serde_json::from_str::<serde_json::Value>(request.body.as_deref()?)
                .ok()?
                .get("guarded")
                .cloned()
        })
        .collect()
}

/// The live-stream paths the media server's declaration names, as the pass service reads
/// them.
fn declared_live() -> String {
    Live::of(
        lemonfiber_fixtures::declaring::enforceable()
            .live
            .into_iter()
            .map(|path| LivePath {
                pattern: path.pattern,
                stream: path.stream,
            })
            .collect(),
    )
    .written()
}

#[tokio::test]
async fn a_guard_speaking_the_contract_is_handed_the_envelope_and_then_holds_it() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-handed", Arc::clone(&http), false);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert_eq!(
        state(&wirings, ENVELOPE),
        Some(&State::Wired),
        "{wirings:?}"
    );
    assert_eq!(
        state(&wirings, LIVE),
        None,
        "no pass service runs, so none is told"
    );
    let sent = envelopes(&http);
    let [envelope] = sent.as_slice() else {
        unreachable!("one envelope was not handed: {sent:?}");
    };
    let field = |name: &str| envelope.get(name).and_then(serde_json::Value::as_str);
    assert_eq!(field("pass"), Some("http://10.80.96.34:8090/pass"));
    assert_eq!(field("proxy"), Some("10.80.96.3"));
    assert!(
        field("upstream").is_some_and(|at| at.starts_with("http://jellyfin:")),
        "{envelope}"
    );
    assert!(
        field("adapter").is_some_and(|at| at.starts_with("http://jellyfin-adapter:")),
        "{envelope}"
    );
    assert_eq!(
        envelope.get("declared"),
        serde_json::to_value(lemonfiber_fixtures::declaring::enforceable())
            .ok()
            .as_ref()
    );

    let held = envelope.clone();
    let again = answering(&[media::answered(&held)]);
    let (ctx, _) = machine("envelope-held", Arc::clone(&again), false);
    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;
    assert_eq!(
        state(&wirings, ENVELOPE),
        Some(&State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(envelopes(&again).is_empty());
}

#[tokio::test]
async fn the_pass_service_is_told_the_declared_live_stream_paths_where_it_runs() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-live", http, false);
    passing(&project);
    let live = live_at(&project);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert_eq!(state(&wirings, LIVE), Some(&State::Wired), "{wirings:?}");
    assert_eq!(std::fs::read_to_string(&live).ok(), Some(declared_live()));
    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;
    assert_eq!(
        state(&wirings, LIVE),
        Some(&State::AlreadyWired),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_hand_and_hands_nothing() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-rehearsed", Arc::clone(&http), true);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(state(&wirings, ENVELOPE), Some(State::WouldWire { yours: None, ours: Some(ours) })
            if ours.contains("passes asked at http://10.80.96.34:8090/pass")),
        "{wirings:?}"
    );
    assert!(envelopes(&http).is_empty());
}

#[tokio::test]
async fn a_guard_refusing_the_envelope_fails_saying_so() {
    let http = Fake::by_path_in_turn(vec![
        (
            media::serve("declared"),
            vec![media::answered(
                &lemonfiber_fixtures::declaring::enforceable(),
            )],
        ),
        (guarding(), vec![Answer::reply(200, "null")]),
        (guard(), vec![media::refusing()]),
    ]);
    let (ctx, project) = machine("envelope-refused", http, false);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(state(&wirings, ENVELOPE), Some(State::Failed { detail }) if detail.contains("did not take")),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_media_server_that_cannot_be_asked_what_it_serves_is_said_and_nothing_is_handed() {
    let http = Fake::by_path_in_turn(vec![
        (media::serve("declared"), vec![media::unavailable()]),
        (guarding(), vec![Answer::reply(200, "null")]),
        (guard(), vec![Answer::reply(204, "")]),
    ]);
    let (ctx, project) = machine("envelope-undeclared", Arc::clone(&http), false);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(
            state(&wirings, ENVELOPE),
            Some(State::Skipped { .. } | State::Failed { .. })
        ),
        "{wirings:?}"
    );
    assert!(envelopes(&http).is_empty());
}

#[tokio::test]
async fn nothing_is_said_where_nothing_guards_or_nothing_serves() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-nothing", Arc::clone(&http), false);
    let unguarded = crate::test_support::stack_fillers(
        Vec::new(),
        &media::moved().into_iter().collect::<Vec<_>>(),
        Some(&project),
        &[],
    );
    let unserved = unspoken(&project, None, false);

    assert!(seed_envelope(&ctx, &unguarded, Some(&project))
        .await
        .is_empty());
    assert!(seed_envelope(&ctx, &unserved, Some(&project))
        .await
        .is_empty());
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_guard_speaking_no_contract_beside_no_pass_service_needs_no_declaration() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-unneeded", Arc::clone(&http), false);
    let fillers = unspoken(&project, media::moved(), true);

    assert!(seed_envelope(&ctx, &fillers, Some(&project))
        .await
        .is_empty());
    assert!(http.requests().is_empty());

    let unanswered =
        Fake::by_path_in_turn(vec![(media::serve("declared"), vec![media::unavailable()])]);
    let (ctx, project) = machine("envelope-unneeded-live", unanswered, false);
    passing(&project);
    let fillers = unspoken(&project, media::moved(), true);
    let wirings = seed_envelope(&ctx, &fillers, Some(&project)).await;
    assert!(
        matches!(
            state(&wirings, LIVE),
            Some(State::Skipped { .. } | State::Failed { .. })
        ),
        "{wirings:?}"
    );
    assert_eq!(state(&wirings, ENVELOPE), None);
}

#[tokio::test]
async fn a_guard_speaking_no_contract_leaves_the_pass_service_told_alone() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-live-alone", Arc::clone(&http), false);
    passing(&project);

    let wirings = seed_envelope(
        &ctx,
        &unspoken(&project, media::moved(), true),
        Some(&project),
    )
    .await;

    assert_eq!(state(&wirings, LIVE), Some(&State::Wired), "{wirings:?}");
    assert_eq!(state(&wirings, ENVELOPE), None, "{wirings:?}");
    assert_eq!(
        std::fs::read_to_string(live_at(&project)).ok(),
        Some(declared_live())
    );
    assert!(envelopes(&http).is_empty());
}

#[tokio::test]
async fn a_rehearsal_says_what_is_held_beside_what_it_would_hand_and_writes_nothing() {
    let held = serde_json::json!({
        "declared": lemonfiber_fixtures::declaring::enforceable(),
        "upstream": "http://elsewhere:8096",
        "adapter": null,
        "proxy": "10.0.0.1",
        "pass": "http://10.0.0.2:8090/pass",
    });
    let http = answering(&[media::answered(&held)]);
    let (ctx, project) = machine("envelope-rehearsed-held", Arc::clone(&http), true);
    passing(&project);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(state(&wirings, ENVELOPE), Some(State::WouldWire { yours: Some(yours), ours: Some(ours) })
            if yours.ends_with("the media server at http://elsewhere:8096, the proxy at 10.0.0.1, \
                passes asked at http://10.0.0.2:8090/pass")
                && ours.contains(" through http://jellyfin-adapter:")),
        "{wirings:?}"
    );
    assert_eq!(
        state(&wirings, LIVE),
        Some(&State::WouldWire {
            yours: None,
            ours: Some(declared_live())
        })
    );
    assert!(envelopes(&http).is_empty());
    assert!(!live_at(&project).exists());
}

#[tokio::test]
async fn a_media_server_out_of_reach_of_its_contract_is_skipped_saying_why() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-unkeyed", Arc::clone(&http), false);
    let _ = std::fs::remove_file(crate::plugin::key_file(&project, media::ADAPTER));

    let unkeyed = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;
    let unspeaking = seed_envelope(
        &ctx,
        &unspoken(&project, media::moved().map(silenced), false),
        Some(&project),
    )
    .await;

    assert!(
        matches!(state(&unkeyed, ENVELOPE), Some(State::Skipped { reason })
            if reason.ends_with(" cannot be asked over media.serve")),
        "{unkeyed:?}"
    );
    assert!(
        matches!(state(&unspeaking, ENVELOPE), Some(State::Skipped { reason })
            if reason.ends_with(" speaks no media.serve contract, so nothing declares the paths it serves")),
        "{unspeaking:?}"
    );
    assert!(envelopes(&http).is_empty());
}

#[tokio::test]
async fn a_declaration_failing_its_check_is_neither_handed_nor_written() {
    let mut declared = lemonfiber_fixtures::declaring::enforceable();
    declared.refused = vec!["/media/admin".to_owned()];
    let http = declaring(&declared, &[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-undeclarable", Arc::clone(&http), false);
    passing(&project);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.connection == ENVELOPE
            && matches!(&one.state, State::Failed { detail }
                if detail.starts_with("the media server's declaration was refused: ")
                    && detail.contains("/media/admin"))),
        "{wirings:?}"
    );
    assert!(envelopes(&http).is_empty());
    assert!(!live_at(&project).exists());
}

#[tokio::test]
async fn a_guard_that_cannot_say_what_it_holds_is_handed_nothing() {
    let http = answering(&[media::unavailable()]);
    let (ctx, project) = machine("envelope-unheld", Arc::clone(&http), false);

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(state(&wirings, ENVELOPE), Some(State::Skipped { reason })
            if reason.starts_with("the service is not answering")),
        "{wirings:?}"
    );
    assert!(envelopes(&http).is_empty());
}

#[tokio::test]
async fn live_stream_paths_that_cannot_be_written_fail_saying_where() {
    let http = answering(&[Answer::reply(200, "null")]);
    let (ctx, project) = machine("envelope-unwritable", http, false);
    passing(&project);
    let _ = std::fs::create_dir_all(project.join(CONFIGURATION_TREE));
    assert!(std::fs::write(
        project.join(CONFIGURATION_TREE).join(SERVICE),
        "a file where a directory goes"
    )
    .is_ok());

    let wirings = seed_envelope(&ctx, &fillers(&project), Some(&project)).await;

    assert!(
        matches!(state(&wirings, LIVE), Some(State::Failed { detail })
            if detail.contains(File::Live.name())),
        "{wirings:?}"
    );
}

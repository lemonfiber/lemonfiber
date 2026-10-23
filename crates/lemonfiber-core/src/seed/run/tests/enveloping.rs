//! The stream guard, applied the envelope it enforces over its contract, and the pass
//! service told the media server's live-stream paths.

use super::super::enveloping::{seed_envelope, ENVELOPE, LIVE};
use super::*;
use crate::pass::{File, Live, LivePath, PINNED, SERVICE};
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

/// The media server's plugin and the guard's, written at `project`.
fn fillers(project: &std::path::Path) -> crate::wiring::Fillers {
    let installed: Vec<crate::plugin::Installed> = media::moved()
        .into_iter()
        .chain(std::iter::once(crate::test_support::contracted(
            "guarding",
            GUARD,
            lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
        )))
        .collect();
    crate::test_support::stack_fillers(Vec::new(), &installed, Some(project), &[])
}

/// A media server declaring what a guard can enforce, and a guard enforcing `held` and
/// taking whatever it is handed.
fn answering(held: &[Answer]) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            media::serve("declared"),
            vec![media::answered(
                &lemonfiber_fixtures::declaring::enforceable(),
            )],
        ),
        (guarding(), held.to_vec()),
        (guard(), vec![Answer::reply(204, "")]),
    ])
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
    let _ = std::fs::create_dir_all(&project);
    let _ = std::fs::write(
        project.join(PINNED),
        crate::pass::pin(
            crate::stack::RUNNING,
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        ),
    );
    let live = project.join("config").join(SERVICE).join(File::Live.name());

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
    let unserved = crate::test_support::stack_fillers(
        Vec::new(),
        &[crate::test_support::contracted(
            "guarding",
            GUARD,
            lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
        )],
        Some(&project),
        &[],
    );

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
    let mut uncontracted = crate::test_support::contracted(
        "guarding",
        GUARD,
        lemonfiber_contract::capabilities::stream::guard::CAPABILITY,
    );
    for placed in &mut uncontracted.services {
        placed.speaks.clear();
    }
    let installed: Vec<crate::plugin::Installed> = media::moved()
        .into_iter()
        .chain(std::iter::once(uncontracted))
        .collect();
    let fillers = crate::test_support::stack_fillers(Vec::new(), &installed, Some(&project), &[]);

    assert!(seed_envelope(&ctx, &fillers, Some(&project))
        .await
        .is_empty());
    assert!(http.requests().is_empty());

    let unanswered =
        Fake::by_path_in_turn(vec![(media::serve("declared"), vec![media::unavailable()])]);
    let (ctx, project) = machine("envelope-unneeded-live", unanswered, false);
    let _ = std::fs::create_dir_all(&project);
    let _ = std::fs::write(
        project.join(PINNED),
        crate::pass::pin(
            crate::stack::RUNNING,
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        ),
    );
    let fillers = crate::test_support::stack_fillers(Vec::new(), &installed, Some(&project), &[]);
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

use super::{push, Pushed, SETTING};
use crate::doctor::vpn::Forwarding;
use crate::journal::Kind;
use crate::test_support::{a_context, nowhere};

/// What is known about the port right now.
const fn known(granted: Option<u16>, listening: Option<u16>) -> Forwarding {
    Forwarding { granted, listening }
}

/// A write that succeeds, recording what it was asked for.
async fn accepts(_port: u16) -> Result<(), String> {
    Ok(())
}

/// A write the client refuses.
async fn refuses(_port: u16) -> Result<(), String> {
    Err("the client is not accepting settings".to_owned())
}

#[tokio::test]
async fn a_client_on_the_granted_port_is_left_alone() {
    // A write that changes nothing is still a write, and one made every run is
    // a client restarted every run.
    let pushed = push(known(Some(51413), Some(51413)), accepts).await;
    assert_eq!(pushed, Pushed::Unchanged);
    assert_eq!(pushed.said(), None, "and nothing is said about it");
}

#[tokio::test]
async fn a_port_that_changed_moves_the_client_and_is_recorded() {
    // The fault this exists for: the tunnel reconnected on a new port, and
    // everything looks correct while nobody outside can reach the client.
    let pushed = push(known(Some(51999), Some(51413)), accepts).await;
    assert_eq!(
        pushed,
        Pushed::Moved {
            from: Some(51413),
            to: 51999
        }
    );

    let change = pushed.change("1000");
    assert_eq!(
        change.map(|change| (change.target, change.kind)),
        Some((
            "qbittorrent".to_owned(),
            Kind::Set {
                key: SETTING.to_owned(),
                previous: Some("51413".to_owned()),
                current: "51999".to_owned(),
            }
        )),
        "an operator reading back why seeding stopped finds it"
    );
    let said = pushed.said().unwrap_or_default();
    assert!(said.contains("51413") && said.contains("51999"), "{said}");
}

#[tokio::test]
async fn a_client_that_never_said_which_port_it_was_on_is_still_set() {
    // Unknown is not "already correct". Leaving it would keep it unreachable
    // on the strength of not having been able to ask.
    let pushed = push(known(Some(51413), None), accepts).await;
    assert_eq!(
        pushed,
        Pushed::Moved {
            from: None,
            to: 51413
        }
    );
    assert!(pushed.said().is_some_and(|said| said.contains("51413")));
}

#[tokio::test]
async fn nothing_is_pushed_where_the_provider_granted_nothing() {
    // There is no port to move to, and inventing one would take the client off
    // a working default for no reason.
    let pushed = push(known(None, Some(6881)), refuses).await;
    assert_eq!(pushed, Pushed::Unchanged, "the write is never attempted");
}

#[tokio::test]
async fn a_refused_write_says_so_rather_than_being_recorded_as_done() {
    let pushed = push(known(Some(51413), Some(6881)), refuses).await;
    assert_eq!(
        pushed,
        Pushed::Refused {
            reason: "the client is not accepting settings".to_owned()
        },
        "the client's own words, not a paraphrase"
    );
    assert_eq!(pushed.change("1000"), None, "nothing happened to record");
    assert!(pushed
        .said()
        .is_some_and(|said| said.contains("not accepting settings")));
}

/// A context whose stack is this repo's own and whose transport answers
/// nothing — enough to reach the decision without a client.
fn ctx() -> crate::app::Ctx {
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .build()
}

#[tokio::test]
async fn a_client_lemonfiber_cannot_authenticate_to_is_left_alone() {
    // It can be neither read nor corrected, and guessing either way would be
    // worse than saying nothing. No password is recorded here.
    let pushed = super::reconcile(&ctx(), Some(51413), None).await;
    assert_eq!(pushed, Pushed::Unchanged);
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_changes_nothing() {
    let nowhere = a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .over(nowhere())
        .build();
    assert_eq!(
        super::reconcile(&nowhere, Some(51413), None).await,
        Pushed::Unchanged
    );
}

/// An environment file recording a qBittorrent password, at a scratch path
/// unique to the test so concurrent tests do not share one.
fn env_at(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("fwd-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        crate::config::store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            &crate::test_support::a_password(),
        )
        .is_ok(),
        "the scratch env file is written"
    );
    path
}

/// A context that can authenticate to a torrent client answering `replies`.
fn ctx_with_client(name: &str, replies: Vec<(u16, &'static str)>) -> crate::app::Ctx {
    let settings = crate::config::Settings {
        env_file: Some(env_at(name)),
        ..crate::config::Settings::default()
    };
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(settings)
        .build()
        .with_http(lemonfiber_fixtures::http::Fake::scripted(replies))
}

#[tokio::test]
async fn a_client_already_on_the_granted_port_is_read_and_left() {
    // Login, then the preferences read. No write follows, because a write that
    // changes nothing still restarts the client's listener.
    let ctx = ctx_with_client(
        "already",
        vec![(200, "Ok."), (200, r#"{"listen_port":51413}"#)],
    );
    assert_eq!(
        super::reconcile(&ctx, Some(51413), None).await,
        Pushed::Unchanged
    );
}

#[tokio::test]
async fn a_client_on_yesterdays_port_is_moved_to_the_one_granted_now() {
    // The whole point: the tunnel reconnected on a new port and everything
    // looks correct while nobody outside can reach the client.
    let ctx = ctx_with_client(
        "moved",
        vec![
            (200, "Ok."),
            (200, r#"{"listen_port":51413}"#),
            (200, "Ok."),
            (200, ""),
            (200, "Ok."),
            (200, r#"{"listen_port":51999}"#),
        ],
    );
    assert_eq!(
        super::reconcile(&ctx, Some(51999), None).await,
        Pushed::Moved {
            from: Some(51413),
            to: 51999
        }
    );
}

#[tokio::test]
async fn a_client_that_will_not_answer_is_still_set_rather_than_assumed_correct() {
    // Unknown is not "already right"; leaving it would keep it unreachable on
    // the strength of not having been able to ask.
    let ctx = ctx_with_client(
        "silent",
        vec![
            (500, "no"),
            (200, "Ok."),
            (200, ""),
            (200, "Ok."),
            (200, r#"{"listen_port":51999}"#),
        ],
    );
    assert_eq!(
        super::reconcile(&ctx, Some(51999), None).await,
        Pushed::Moved {
            from: None,
            to: 51999
        }
    );
}

#[tokio::test]
async fn starting_a_stack_with_no_forwarding_asked_for_changes_nothing() {
    // Nothing was requested, so nothing was granted, and a client on its own
    // default port is where the operator left it.
    // Collected rather than matched: the stack this repo embeds always parses,
    // so a fallback would be a branch no passing test can reach.
    let ctx = ctx();
    let manifests: Vec<_> = ctx
        .stack
        .checked_manifest(ctx.today())
        .into_iter()
        .collect();
    for manifest in &manifests {
        assert_eq!(super::after_start(&ctx, manifest).await, None);
    }
    assert_eq!(manifests.len(), 1, "the embedded stack parses");
}

#[tokio::test]
async fn a_client_that_refuses_the_write_says_so_in_its_own_words() {
    // Login, the read, login, then a refusal — reported rather than recorded
    // as done.
    let ctx = ctx_with_client(
        "refusing",
        vec![
            (200, "Ok."),
            (200, r#"{"listen_port":51413}"#),
            (200, "Ok."),
            (403, "Forbidden"),
        ],
    );
    // Compared through what it would say, rather than asserted with a message
    // argument: an argument only evaluates when the assertion fails, so it is a
    // line no passing test can cover.
    let pushed = super::reconcile(&ctx, Some(51999), None).await;
    let said = pushed.said().unwrap_or_default();
    assert!(
        said.starts_with("the forwarded port could not be pushed"),
        "{said}"
    );
    assert_eq!(pushed.change("1000"), None, "nothing happened to record");
}

#[tokio::test]
async fn the_clients_own_port_is_read_for_the_diagnosis() {
    // Asked by the caller because the VPN check speaks to containers, and this
    // is a service's own API.
    let ctx = ctx_with_client(
        "reading",
        vec![(200, "Ok."), (200, r#"{"listen_port":51413}"#)],
    );
    let manifests: Vec<_> = ctx
        .stack
        .checked_manifest(ctx.today())
        .into_iter()
        .collect();
    for manifest in &manifests {
        assert_eq!(
            super::listening_port(&ctx, manifest, None).await,
            Some(51413)
        );
    }
    assert_eq!(manifests.len(), 1, "the embedded stack parses");
}

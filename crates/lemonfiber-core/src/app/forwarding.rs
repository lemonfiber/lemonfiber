//! Keeping the download client on the port the provider actually granted.
//!
//! A tunnel that drops and comes back is commonly granted a different port. The
//! gateway records the new one, and the download client goes on listening on
//! yesterday's — at which point everything looks correct from inside and nobody
//! outside can reach it. Torrents still download, so nothing announces it; only
//! the seeding stops, which is the part an operator notices last.
//!
//! So the grant is compared with what the client is listening on, and a mismatch
//! is pushed rather than merely reported: this is the one VPN fault where the fix
//! is unambiguous, and where leaving it to be read about means it stays broken
//! until somebody happens to look.
//!
//! Recorded either way. A port that changed and was re-pushed is a thing that
//! happened to the stack, and an operator reading back why seeding stopped for an
//! hour needs to find it.

use std::path::Path;

use crate::doctor::vpn::Forwarding;
use crate::error::Diagnose;
use crate::journal::{Change, Kind};

use super::targets::{download_targets, torrent_client};
use super::Ctx;

/// The setting a re-pushed port is journalled under, so a change to it reads like
/// any other change lemonfiber made.
pub const SETTING: &str = "qbittorrent.listen_port";

/// What one pass over the forwarded port did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// The client was already on the granted port, or there was nothing to push.
    Unchanged,
    /// The client was moved to the granted port.
    Moved {
        /// What it was listening on before, where that was known.
        from: Option<u16>,
        /// What it is listening on now.
        to: u16,
    },
    /// A push was needed and could not be made.
    Refused {
        /// What the client said, in its own words.
        reason: String,
    },
}

impl Pushed {
    /// The change this amounts to, for the journal — nothing where nothing moved.
    #[must_use]
    pub fn change(&self, stamp: &str) -> Option<Change> {
        match self {
            Self::Moved { from, to } => Some(Change {
                at: stamp.to_owned(),
                operation: "vpn port forwarding".to_owned(),
                target: "qbittorrent".to_owned(),
                kind: Kind::Set {
                    key: SETTING.to_owned(),
                    previous: from.map(|port| port.to_string()),
                    current: to.to_string(),
                },
            }),
            Self::Unchanged | Self::Refused { .. } => None,
        }
    }

    /// The line an operator reads.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match self {
            Self::Unchanged => None,
            Self::Moved { from, to } => Some(match from {
                Some(before) => format!(
                    "the forwarded port changed from {before} to {to}; the download client was \
                     moved to it"
                ),
                None => format!("the download client was set to listen on the forwarded port {to}"),
            }),
            Self::Refused { reason } => Some(format!(
                "the forwarded port could not be pushed to the download client: {reason}"
            )),
        }
    }
}

/// Move the client onto the granted port where it is not already there.
///
/// `set` is the write itself, kept as a parameter so the decision is testable
/// without a client: everything above this line is about *whether* to write, and
/// this function is only about which write and what to record.
pub async fn push<F, E>(forwarding: Forwarding, set: F) -> Pushed
where
    F: FnOnce(u16) -> E,
    E: std::future::Future<Output = Result<(), String>>,
{
    let Some(port) = forwarding.to_push() else {
        return Pushed::Unchanged;
    };
    match set(port).await {
        Ok(()) => Pushed::Moved {
            from: forwarding.listening,
            to: port,
        },
        Err(reason) => Pushed::Refused { reason },
    }
}

/// Read the granted port and the client's own, and move the client where they
/// differ.
///
/// The one VPN fault whose fix is unambiguous, so it is applied rather than
/// reported. A client that cannot be authenticated to is left alone: it can be
/// neither read nor corrected, and guessing would be worse than saying nothing.
pub async fn reconcile(ctx: &Ctx, granted: Option<u16>, project: Option<&Path>) -> Pushed {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Pushed::Unchanged;
    };
    let targets = download_targets(&manifest.services, project);
    let Some(client) = torrent_client(ctx, &targets) else {
        return Pushed::Unchanged;
    };
    let forwarding = Forwarding {
        granted,
        listening: client.listen_port().await.ok(),
    };
    push(forwarding, |port| async move {
        client
            .set_listen_port(port)
            .await
            .map_err(|failure| failure.problem().summary)
    })
    .await
}

/// What the torrent client says it is listening on, where there is one and it can
/// be authenticated to.
pub async fn listening_port(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&std::path::Path>,
) -> Option<u16> {
    let targets = download_targets(&manifest.services, project);
    torrent_client(ctx, &targets)?.listen_port().await.ok()
}

/// What starting the stack does about the forwarded port.
///
/// The tunnel has just come up, which is exactly when the provider grants a port
/// — commonly a different one from last time. Applied here rather than offered
/// because the operator has already asked for an action, and a client left on
/// yesterday's port looks entirely healthy from inside while nobody outside can
/// reach it. A diagnosis, which is only looking, offers the same fix instead of
/// making it.
pub async fn after_start(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Option<String> {
    let granted = crate::doctor::vpn::granted_port(
        ctx.engine.as_ref(),
        &ctx.settings.project,
        manifest,
        ctx.settings.port_forward.enabled,
    )
    .await;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    reconcile(ctx, granted, project.as_deref()).await.said()
}

#[cfg(test)]
mod tests {
    use super::{push, Pushed, SETTING};
    use crate::doctor::vpn::Forwarding;
    use crate::journal::Kind;

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
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            crate::config::Settings::default(),
            crate::platform::Environment::MacOs,
        )
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
        let nowhere = crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            crate::config::Settings::default(),
            crate::platform::Environment::MacOs,
        );
        assert_eq!(
            super::reconcile(&nowhere, Some(51413), None).await,
            Pushed::Unchanged
        );
    }

    /// An environment file recording a qBittorrent password, at a scratch path
    /// unique to the test so concurrent tests do not share one.
    fn env_at(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-fwd-{}-{name}", std::process::id()));
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
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            settings,
            crate::platform::Environment::MacOs,
        )
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
}

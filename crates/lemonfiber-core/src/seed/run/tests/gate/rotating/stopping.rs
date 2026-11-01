//! Every way a replacement of the media server's internal certificate stops where it
//! stands: nothing to replace, nobody to ask or hand it, a disk that will not take it,
//! and a holder or a certificate gone missing part-way.

use super::*;

#[tokio::test(start_paused = true)]
async fn a_media_server_lemonfiber_writes_no_certificate_for_has_none_replaced() {
    let (ctx, at, old, _) = machine("rotating-unconfined", answering(now(), true));
    let (services, fillers, mut server) = parts(&at);
    server.served.confined_to = None;
    let stack = Stack {
        services: &services,
        fillers: &fillers,
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
    let (ctx, at, old, _) = machine("rotating-unplaced", answering(now(), true));
    let (services, fillers, mut server) = parts(&at);
    server.served.config_at = None;
    let stack = Stack {
        services: &services,
        fillers: &fillers,
    };

    let unplaced = replacing::replaced(&ctx, stack, &server).await;

    assert!(
        failed_saying(&unplaced, "says nowhere its container holds"),
        "{unplaced:?}"
    );
    assert_eq!(kept(&at).current, Some(old.clone()));

    let http = answering(now(), true);
    let (ctx, at, old, _) = machine_beside(
        "rotating-unadministered",
        false,
        Gate::admitting(holding(CURATORS, "held")),
        http.clone(),
    );

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
    let (mut ctx, at, old, gate) = machine("rotating-unwithdrawn", answering(now(), false));
    let _ = replace(&ctx, &at).await;
    ctx.seams.http = beside(answering(now(), true), &gate);
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
    assert_eq!(gate_pins(&gate), both(&old, &new));
}

#[tokio::test(start_paused = true)]
async fn a_replacement_with_nobody_to_hand_it_says_so() {
    let (_, at, _, _) = machine("rotating-unheld", answering(now(), true));
    let (_, fillers, server) = parts(&at);
    let alone = stack(false);

    let said = replacing::rehearsed(
        Stack {
            services: &alone,
            fillers: &fillers,
        },
        &server,
    );

    assert!(
        said.contains("no service holds a media server credential"),
        "{said}"
    );
}

/// A transport that writes `left` over the certificate at `spoiled`, or takes it away
/// where `left` is nothing, once the media server is asked to serve one, as a disk that
/// fails part-way through a replacement leaves it.
struct Spoiling {
    inner: Arc<dyn crate::ports::http::Http>,
    spoiled: std::path::PathBuf,
    left: Option<&'static str>,
}

#[async_trait::async_trait]
impl crate::ports::http::Http for Spoiling {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        if request.url.ends_with(&media::serve("serve_tls")) {
            let _ = match self.left {
                Some(left) => std::fs::write(&self.spoiled, left),
                None => std::fs::remove_file(&self.spoiled),
            };
        }
        self.inner.send(request).await
    }
}

#[tokio::test(start_paused = true)]
async fn a_replacement_whose_outcome_cannot_be_read_back_is_never_reported_as_landed() {
    let http = answering(now(), true);
    let (mut ctx, at, _, gate) = machine("rotating-spoiled", http.clone());
    let offer = rotated(&ctx, &at, None)
        .await
        .ok()
        .and_then(|read| read.offer)
        .unwrap_or_default();
    ctx.seams.http = Arc::new(Spoiling {
        inner: beside(http.clone(), &gate),
        spoiled: internal::path(&config(&at)),
        left: Some("not a certificate"),
    });

    let spoiled = rotated(&ctx, &at, Some(&offer)).await;

    assert!(
        spoiled.as_ref().is_ok_and(|one| matches!(
            &one.settled,
            Settled::Unproven { detail } if detail.contains("does not read")
        )),
        "{spoiled:?}"
    );

    let (mut ctx, at, _, gate) = machine("rotating-lost", http.clone());
    ctx.seams.http = Arc::new(Spoiling {
        inner: beside(http, &gate),
        spoiled: internal::path(&config(&at)),
        left: None,
    });

    let lost = replace(&ctx, &at).await;

    assert!(
        failed_saying(&lost, "keeps no certificate in force"),
        "{lost:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_replacement_taken_up_again_stops_where_a_holder_or_the_certificate_went_missing() {
    let (mut ctx, at, old, gate) = machine("rotating-taken-up", answering(now(), false));
    let _ = replace(&ctx, &at).await;
    ctx.seams.http = beside(answering(now(), true), &gate);
    let Some(new) = kept(&at).current else {
        unreachable!("the replacement is in place");
    };
    let unpinned: Vec<Gated> = gate
        .routes()
        .into_iter()
        .map(|route| match route.target {
            Target::MediaServer { pins } => Gated {
                target: Target::MediaServer {
                    pins: pins
                        .into_iter()
                        .map(|pin| {
                            if pin == new.fingerprint {
                                "0".repeat(64)
                            } else {
                                pin
                            }
                        })
                        .collect(),
                },
                ..route
            },
            Target::Curator { .. } => route,
        })
        .collect();
    let _ = crate::ports::service::Gating::admit(gate.as_ref(), &unpinned).await;

    let unheld = advance(&ctx, &at).await;

    assert!(
        failed_saying(&unheld, "the request gate does not yet hold its SHA-256"),
        "{unheld:?}"
    );
    assert_eq!(kept(&at).previous, Some(old));

    let _ = std::fs::remove_file(internal::path(&config(&at)));

    let lost = advance(&ctx, &at).await;

    assert!(
        failed_saying(&lost, "keeps no certificate in force"),
        "{lost:?}"
    );
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn a_replacement_that_cannot_be_put_in_place_is_never_served() {
    use std::os::unix::fs::PermissionsExt as _;
    let http = answering(now(), true);
    let (ctx, at, old, _) = machine("rotating-unpromoted", http.clone());
    let _ = internal::staged(&lemonfiber_adapters::Disk, &config(&at), "jellyfin");
    let _ = seeded(&ctx, &http, true, &at).await;
    let directory = internal::path(&config(&at))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500));

    let unpromoted = advance(&ctx, &at).await;

    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    assert!(
        failed_saying(&unpromoted, "could not be written"),
        "{unpromoted:?}"
    );
    assert_eq!(kept(&at).current, Some(old));
    assert!(!http.asked_for(media::serve("serve_tls")));
}

/// `http`, with `gate` answering every `request.gate` call beside it.
fn beside(http: Arc<Fake>, gate: &Arc<Gate>) -> Arc<dyn crate::ports::http::Http> {
    Arc::new(crate::test_support::guards::Serving::beside(http).gate(gate))
}

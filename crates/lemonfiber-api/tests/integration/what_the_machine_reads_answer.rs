//! What a stop would interrupt and how the surface is served, as a client reaches them:
//! who is answered, who is refused, and what the answer never carries.

use base64::Engine as _;
use lemonfiber_core::companion::served::Served;
use lemonfiber_core::error::codes::serve::NOT_SERVING;

use crate::door::{housed, keeping_at, surface, Chance, Recognising, Scratch};
use crate::reading::{
    answered, asked, holding, refused, running, stack, written, Command, Ctx, Refusal, Settings,
    StatusCode,
};
use crate::what_a_key_admits::{code, keeping_keys, presented, HERE};
use crate::what_a_member_may_ask_for::{as_a_member, MEMBER};

/// The two reads, as a client asks them.
const READS: [&str; 2] = ["/api/in-flight", "/api/served"];

/// The port a settled run serves on here.
const PORT: u16 = 8443;

/// A world keeping what pairing needs in `directory`.
fn keeping_in(directory: &Scratch) -> Ctx {
    holding(
        running(),
        stack(),
        Settings {
            companion: Some(directory.to_path_buf()),
            ..Settings::default()
        },
    )
}

/// `ctx` once the run has settled how it serves the surface.
async fn settled(ctx: Ctx, encrypted: bool) -> Ctx {
    let settling = Command::Served(Served {
        port: PORT,
        encrypted,
        network: false,
    });
    assert!(
        lemonfiber_core::app::dispatch(settling, &ctx).await.is_ok(),
        "the run settled how it serves"
    );
    ctx
}

/// The kind an answer's envelope carries.
fn kind(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|read| {
            read.get("kind")
                .and_then(|kind| kind.as_str().map(str::to_owned))
        })
        .unwrap_or_default()
}

/// What a stop would interrupt is answered to the operator, as the command renders it.
#[tokio::test]
async fn what_a_stop_would_interrupt_is_answered_as_the_command_renders_it() {
    let at = Scratch::new("machine-in-flight");
    let seen = asked(
        keeping_in(&at),
        "/api/in-flight?forms=tv&forms=dl&services=qbittorrent",
    )
    .await;
    let rendered = crate::reading::as_the_command_renders_it(
        &keeping_in(&at),
        Command::InFlight {
            forms: vec!["tv".to_owned(), "dl".to_owned()],
            services: vec!["qbittorrent".to_owned()],
        },
    )
    .await;
    assert_eq!(seen, rendered.map(|body| (StatusCode::OK, body)));
}

/// A parameter the stop does not name is refused rather than dropped.
#[tokio::test]
async fn a_parameter_the_stop_does_not_name_is_refused() {
    let at = Scratch::new("machine-in-flight-unwanted");
    let seen = asked(keeping_in(&at), "/api/in-flight?form=tv").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::BAD_REQUEST && body.contains("takes no form")
        ),
        "a form named as a log read names one was not refused"
    );
    let seen = asked(keeping_in(&at), "/api/served?port=1").await;
    assert!(
        seen.is_some_and(|(status, _)| status == StatusCode::BAD_REQUEST),
        "how the surface is served takes nothing"
    );
}

/// How the surface is served is refused by a run that settled nothing.
#[tokio::test]
async fn how_the_surface_is_served_is_refused_where_nothing_was_settled() {
    let at = Scratch::new("machine-unsettled");
    let seen = asked(keeping_in(&at), "/api/served").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.contains(&format!("\"code\":\"{}\"", NOT_SERVING.as_str()))),
        "a run that settled nothing answered as if it had"
    );
}

/// How the surface is served is answered from what the run settled, carrying no key, no
/// encoding of the certificate, no path and no token, and asking writes nothing: what
/// settling wrote is taken away, and the read puts none of it back.
#[tokio::test]
async fn how_the_surface_is_served_carries_nothing_secret_and_writes_nothing() {
    let at = Scratch::new("machine-served");
    let ctx = settled(keeping_in(&at), true).await;
    let read = |name: &str| std::fs::read_to_string(at.join(name)).unwrap_or_default();
    let (certificate, key) = (read("certificate.pem"), read("key.pem"));
    for name in ["certificate.pem", "key.pem", "served.json"] {
        assert!(std::fs::remove_file(at.join(name)).is_ok(), "{name}");
    }

    let seen = asked(ctx, "/api/served").await;

    let Some((status, body)) = seen else {
        unreachable!("the read answers")
    };
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(kind(&body), "served");
    assert!(
        std::fs::read_dir(&at).is_ok_and(|mut left| left.next().is_none()),
        "asking made or wrote something"
    );
    let base64_of = |pem: &str| -> String {
        pem.lines()
            .filter(|line| !line.starts_with("-----"))
            .collect::<String>()
    };
    let der = base64::engine::general_purpose::STANDARD
        .decode(base64_of(&certificate))
        .unwrap_or_default();
    assert!(
        body.contains(&lemonfiber_core::certificate::fingerprint(&der)),
        "the fingerprint is not the one presented: {body}"
    );
    let token = written().unwrap_or_default();
    for (what, secret) in [
        ("the key", base64_of(&key)),
        ("the certificate's encoding", base64_of(&certificate)),
        ("a PEM marker", "-----".to_owned()),
        ("the directory", at.to_string_lossy().into_owned()),
        ("the record's name", "served.json".to_owned()),
        ("the token", token),
    ] {
        let start: String = secret.chars().take(24).collect();
        assert!(
            !start.is_empty() && !body.contains(&start),
            "the answer carries {what}"
        );
    }
}

/// A request carrying no token, session or key reaches neither read.
#[tokio::test]
async fn a_request_carrying_nothing_reaches_neither_read() {
    for read in READS {
        let at = Scratch::new("machine-nobody");
        let ctx = settled(keeping_in(&at), false).await;
        assert_eq!(
            answered(ctx, read, &[("host", "127.0.0.1:8471")]).await,
            refused(
                Refusal::NotAdmitted,
                "This request carried no token, session or key this run admits."
            ),
            "{read}"
        );
    }
}

/// A member's own session is refused both reads.
#[tokio::test]
async fn a_members_session_is_refused_both_reads() {
    for read in READS {
        let (router, carried, _kept) = as_a_member("machine-member").await;
        let answer = crate::door::asked(router, "GET", read, &carried, "").await;
        assert_eq!(
            answer.status,
            StatusCode::FORBIDDEN,
            "{read}: {}",
            answer.body
        );
    }
}

/// A key whose scope reaches reads is answered both reads, and a key scoped to a member is
/// refused them.
#[tokio::test]
async fn a_read_key_is_answered_both_reads_and_a_member_key_neither() {
    for read in READS {
        let keys = keeping_keys("machine-keys");
        let ctx = settled(
            keeping_at(crate::door::world(None, Chance::cycling()), &keys.kept),
            false,
        )
        .await;
        let (router, _) = surface(ctx, &housed(Recognising::knowing(MEMBER)));
        for secret in [keys.read.as_str(), keys.act.as_str()] {
            let answer = presented(router.clone(), "GET", read, secret, Some((HERE, false))).await;
            assert_eq!(answer.status, StatusCode::OK, "{read}: {}", answer.body);
        }
        let theirs = presented(
            router,
            "GET",
            read,
            keys.member.as_str(),
            Some((HERE, false)),
        )
        .await;
        assert_eq!(
            theirs.status,
            StatusCode::FORBIDDEN,
            "{read}: {}",
            theirs.body
        );
        assert_eq!(code(&theirs), "ADMIT-6", "{read}: {}", theirs.body);
    }
}

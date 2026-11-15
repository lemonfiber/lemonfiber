use std::sync::Arc;

use lemonfiber_fixtures::keying::{Astrayed, Keys};

use super::{AstrayCheck, Named, ASTRAY, CHECK, TITLE};
use crate::app_keys::{DECLINE_APP, GATE_APP};
use crate::doctor::{Category, Check, Finding, Verdict};
use crate::error::{Problem, Remedy};
use crate::ports::service::{AppKeys, Astray};

fn plex() -> Named {
    Named {
        id: "plex".to_owned(),
        name: "Plex".to_owned(),
    }
}

fn account(id: &str) -> Astray {
    Astray {
        id: id.to_owned(),
        name: GATE_APP.to_owned(),
        removed_in: "Settings, Plex Home".to_owned(),
    }
}

async fn found(server: &Arc<Keys>) -> Vec<Finding> {
    AstrayCheck::new(Some(plex()), Some(server.clone() as Arc<dyn AppKeys>))
        .run()
        .await
}

fn about_plex(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Services, check, title, verdict).about("plex")
}

fn held(id: &str) -> Finding {
    about_plex(
        &format!("{CHECK}.{GATE_APP}.{id}"),
        &format!("{GATE_APP} held under {GATE_APP}"),
        Verdict::Warn(Problem::new(
            ASTRAY,
            format!(
                "Plex holds {GATE_APP} (id {id}) under {GATE_APP}, and lemonfiber did not make \
                 it or no longer knows it"
            ),
            "lemonfiber leaves it alone: it uses and removes only what it made, so this stays \
             in the media server until somebody removes it",
            Remedy::new("If nobody here made it, remove it in Plex: Settings, Plex Home"),
        )),
    )
}

#[tokio::test]
async fn every_name_the_core_mints_under_is_asked_and_nothing_else_is_called() {
    let server = Keys::answering(&[(GATE_APP, Astrayed::Holds(vec![account("41")]))]);
    found(&server).await;
    assert_eq!(
        server.calls(),
        vec![
            format!("astray {DECLINE_APP}"),
            format!("astray {GATE_APP}")
        ]
    );
}

#[tokio::test]
async fn each_one_held_is_a_warning_naming_the_server_what_and_where_to_remove_it() {
    let server = Keys::answering(&[(
        GATE_APP,
        Astrayed::Holds(vec![account("41"), account("42")]),
    )]);
    assert_eq!(found(&server).await, vec![held("41"), held("42")]);
}

#[tokio::test]
async fn nothing_held_adds_no_finding() {
    assert_eq!(found(&Keys::answering(&[])).await, Vec::new());
}

#[tokio::test]
async fn an_unsupported_answer_says_so_and_asks_no_further() {
    let server = Keys::answering(&[(DECLINE_APP, Astrayed::Unsupported)]);
    let findings = found(&server).await;
    assert_eq!(server.calls(), vec![format!("astray {DECLINE_APP}")]);
    assert_eq!(
        findings,
        vec![about_plex(
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: "Plex cannot say what it holds under the stack's services' names that \
                         lemonfiber did not make, so anything somebody else made there goes \
                         unreported"
                    .to_owned(),
            }
        )]
    );
}

#[tokio::test]
async fn an_unread_name_is_unverified_beside_what_the_others_hold() {
    let server = Keys::answering(&[
        (DECLINE_APP, Astrayed::Unavailable),
        (GATE_APP, Astrayed::Holds(vec![account("41")])),
    ]);
    assert_eq!(
        found(&server).await,
        vec![
            held("41"),
            about_plex(
                CHECK,
                TITLE,
                Verdict::Unverified {
                    reason: format!(
                        "Plex could not be asked what it holds under {DECLINE_APP}: plex is not \
                         answering"
                    ),
                    remedy: Remedy::new("Check the media server is up and has finished starting")
                        .with_detail("lemonfiber status"),
                }
            ),
        ]
    );
}

#[tokio::test]
async fn no_media_server_skips() {
    assert_eq!(
        AstrayCheck::new(None, None).run().await,
        vec![Finding::in_category(
            Category::Services,
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: "nothing on this stack serves what the household watches".to_owned(),
            }
        )]
    );
}

#[tokio::test]
async fn a_server_that_cannot_be_asked_is_unverified() {
    assert_eq!(
        AstrayCheck::new(Some(plex()), None).run().await,
        vec![about_plex(
            CHECK,
            TITLE,
            Verdict::Unverified {
                reason: "Plex could not be reached".to_owned(),
                remedy: Remedy::new("Check the media server is up and has finished starting")
                    .with_detail("lemonfiber status"),
            }
        )]
    );
}

#[test]
fn it_is_a_services_check() {
    assert_eq!(AstrayCheck::new(None, None).category(), Category::Services);
}

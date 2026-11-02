use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;

use super::{AstrayCheck, Named, ASTRAY, CHECK, TITLE};
use crate::app_keys::{DECLINE_APP, GATE_APP};
use crate::doctor::{Category, Check, Finding, Verdict};
use crate::error::{Problem, Remedy};
use crate::ports::service::{AppKeys, Astray, Dated, Failure};

/// How the fake media server answers `astray` for one name.
#[derive(Clone)]
enum Answer {
    Holds(Vec<Astray>),
    Unsupported,
    Unavailable,
}

/// A media server answering `astray` per name as given, and recording every call made of
/// it.
struct Server {
    answers: BTreeMap<&'static str, Answer>,
    calls: Mutex<Vec<String>>,
}

impl Server {
    fn answering(answers: &[(&'static str, Answer)]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers.iter().cloned().collect(),
            calls: Mutex::default(),
        })
    }

    fn called(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }

    fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl AppKeys for Server {
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        self.called(format!("filed_as {app}"));
        Ok(Vec::new())
    }
    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        self.called(format!("dated {app}"));
        Ok(Vec::new())
    }
    async fn mint(&self, app: &str) -> Result<String, Failure> {
        self.called(format!("mint {app}"));
        Ok(String::new())
    }
    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        self.called(format!("answers_to {key}"));
        Ok(())
    }
    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        self.called(format!("revoke {key}"));
        Ok(())
    }
    async fn astray(&self, app: &str) -> Result<Vec<Astray>, Failure> {
        self.called(format!("astray {app}"));
        match self
            .answers
            .get(app)
            .cloned()
            .unwrap_or(Answer::Holds(Vec::new()))
        {
            Answer::Holds(held) => Ok(held),
            Answer::Unsupported => Err(Failure::Unsupported {
                service: "plex".to_owned(),
                detail: "astray is not offered".to_owned(),
            }),
            Answer::Unavailable => Err(Failure::Unavailable {
                service: "plex".to_owned(),
            }),
        }
    }
}

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

async fn found(server: &Arc<Server>) -> Vec<Finding> {
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
    let server = Server::answering(&[(GATE_APP, Answer::Holds(vec![account("41")]))]);
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
    let server =
        Server::answering(&[(GATE_APP, Answer::Holds(vec![account("41"), account("42")]))]);
    assert_eq!(found(&server).await, vec![held("41"), held("42")]);
}

#[tokio::test]
async fn nothing_held_is_one_pass() {
    assert_eq!(
        found(&Server::answering(&[])).await,
        vec![about_plex(
            CHECK,
            TITLE,
            Verdict::Pass {
                note: Some(
                    "Plex holds nothing under the stack's services' names that lemonfiber did \
                     not make"
                        .to_owned()
                ),
            }
        )]
    );
}

#[tokio::test]
async fn an_unsupported_answer_says_so_and_asks_no_further() {
    let server = Server::answering(&[(DECLINE_APP, Answer::Unsupported)]);
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
    let server = Server::answering(&[
        (DECLINE_APP, Answer::Unavailable),
        (GATE_APP, Answer::Holds(vec![account("41")])),
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

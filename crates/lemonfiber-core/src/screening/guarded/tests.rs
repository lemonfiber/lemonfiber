use std::sync::Mutex;

use async_trait::async_trait;

use super::{guard, Unguarded};
use crate::ports::service::{Addressee, Failure, Guarded, Guarding, Presented, Undeclarable};

/// A guard that keeps every envelope it is handed, and refuses one where told to.
#[derive(Default)]
struct Kept {
    handed: Mutex<Vec<Guarded>>,
    refusing: bool,
}

#[async_trait]
impl Guarding for Kept {
    async fn guard(&self, guarded: &Guarded) -> Result<(), Failure> {
        if let Ok(mut handed) = self.handed.lock() {
            handed.push(guarded.clone());
        }
        if self.refusing {
            return Err(Failure::Refused {
                service: "guard".to_owned(),
                detail: "no".to_owned(),
            });
        }
        Ok(())
    }
    async fn guarding(&self) -> Result<Option<Guarded>, Failure> {
        Ok(None)
    }
    async fn certificate(&self) -> Result<Presented, Failure> {
        Ok(Presented {
            fingerprint: String::new(),
        })
    }
}

impl Kept {
    fn handed(&self) -> Vec<Guarded> {
        self.handed
            .lock()
            .map(|handed| handed.clone())
            .unwrap_or_default()
    }
}

const PROXY: &str = "10.80.96.3";

const PASS: &str = "http://10.80.96.1/pass";

const UPSTREAM: &str = "http://jellyfin:8096";

const ADAPTER: &str = "http://jellyfin-adapter:8080";

/// The envelope around Jellyfin's declaration, its question sent to the media server.
fn envelope() -> Guarded {
    Guarded {
        declared: declared::declared(),
        upstream: UPSTREAM.to_owned(),
        adapter: None,
        proxy: PROXY.to_owned(),
        pass: PASS.to_owned(),
    }
}

/// `envelope()` with its question sent to the adapter.
fn asking_the_adapter(adapter: Option<&str>) -> Guarded {
    let mut guarded = envelope();
    if let Some(question) = guarded.declared.question.as_mut() {
        question.to = Addressee::Adapter;
    }
    guarded.adapter = adapter.map(str::to_owned);
    guarded
}

#[tokio::test]
async fn the_guard_is_handed_the_declaration_verbatim_with_where_it_reaches_and_whom_it_trusts() {
    let kept = Kept::default();
    assert!(guard(&kept, envelope()).await.is_ok());
    assert_eq!(kept.handed(), [envelope()]);
    let adapter = Kept::default();
    let asked = asking_the_adapter(Some(ADAPTER));
    assert!(guard(&adapter, asked.clone()).await.is_ok());
    assert_eq!(adapter.handed(), [asked]);
}

#[tokio::test]
async fn a_declaration_that_fails_its_check_is_refused_and_never_handed_to_the_guard() {
    let kept = Kept::default();
    let mut guarded = envelope();
    guarded.declared.question = None;
    assert!(matches!(
        guard(&kept, guarded).await,
        Err(Unguarded::Undeclarable(Undeclarable::Unasked { .. }))
    ));
    assert!(kept.handed().is_empty());
}

#[tokio::test]
async fn an_envelope_that_does_not_say_where_its_question_goes_is_refused_and_never_handed() {
    let kept = Kept::default();
    let mut nowhere = Vec::new();
    for upstream in [
        "",
        "jellyfin:8096",
        "http://jellyfin:8096/under",
        "http://u@jellyfin",
    ] {
        let mut guarded = envelope();
        upstream.clone_into(&mut guarded.upstream);
        nowhere.push(guarded);
    }
    nowhere.push(asking_the_adapter(None));
    nowhere.push(asking_the_adapter(Some("jellyfin-adapter")));
    let mut beside = envelope();
    beside.adapter = Some("//elsewhere".to_owned());
    nowhere.push(beside);
    for guarded in nowhere {
        let said = format!("{guarded:?}");
        assert!(
            matches!(guard(&kept, guarded).await, Err(Unguarded::Unaddressed(_))),
            "{said}"
        );
    }
    assert!(kept.handed().is_empty());
}

#[tokio::test]
async fn a_guard_that_refuses_the_envelope_is_said_to_have_refused_it() {
    let kept = Kept {
        refusing: true,
        ..Kept::default()
    };
    assert!(matches!(
        guard(&kept, envelope()).await,
        Err(Unguarded::Untaken(_))
    ));
}

mod declared;

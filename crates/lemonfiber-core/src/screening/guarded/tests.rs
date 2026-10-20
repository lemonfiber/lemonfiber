use std::sync::Mutex;

use async_trait::async_trait;

use super::{guard, Unguarded};
use crate::ports::service::{Failure, Guarded, Guarding, Presented, Undeclarable};

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

#[tokio::test]
async fn the_guard_is_handed_the_declaration_verbatim_with_its_proxy_and_its_pass() {
    let kept = Kept::default();
    let declared = crate::jellyfin::declared::declared();
    let applied = guard(&kept, declared.clone(), PROXY.to_owned(), PASS.to_owned()).await;
    assert!(applied.is_ok());
    assert_eq!(
        kept.handed(),
        [Guarded {
            declared,
            proxy: PROXY.to_owned(),
            pass: PASS.to_owned(),
        }]
    );
}

#[tokio::test]
async fn a_declaration_that_fails_its_check_is_refused_and_never_handed_to_the_guard() {
    let kept = Kept::default();
    let mut declared = crate::jellyfin::declared::declared();
    declared.question = None;
    let applied = guard(&kept, declared, PROXY.to_owned(), PASS.to_owned()).await;
    assert!(matches!(
        applied,
        Err(Unguarded::Undeclarable(Undeclarable::Unasked { .. }))
    ));
    assert!(kept.handed().is_empty());
}

#[tokio::test]
async fn a_guard_that_refuses_the_envelope_is_said_to_have_refused_it() {
    let kept = Kept {
        refusing: true,
        ..Kept::default()
    };
    let applied = guard(
        &kept,
        crate::jellyfin::declared::declared(),
        PROXY.to_owned(),
        PASS.to_owned(),
    )
    .await;
    assert!(matches!(applied, Err(Unguarded::Untaken(_))));
}

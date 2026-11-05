//! The recordings a test plugin owes, written as an adapter answering every case inside
//! its contract: a case held to a status with the first status it declares, and every
//! other case with the refusal every operation declares, `unsupported`.
//!
//! Here rather than in `lemonfiber-fixtures`: the contracts' own tests depend on that
//! crate, so it cannot depend on them.

use std::path::Path;

use crate::ports::http::JSON;
use lemonfiber_conformance::{owed, Owed, ASKED_WITH};
use lemonfiber_contract::conformance::Expect;
use lemonfiber_contract::wire::{Kind, Refusal, PROBLEM};

/// Every recording the plugin whose source is at `root` owes, written beneath it.
pub(crate) fn recordings_owed(root: &Path) {
    let published = lemonfiber_contract::capabilities::all();
    let speakers = std::fs::read_to_string(root.join(lemonfiber_bundle::MANIFEST))
        .ok()
        .and_then(|text| lemonfiber_plugin::Manifest::from_toml(&text).ok())
        .map(|manifest| crate::plugin::conforming::speakers(&manifest))
        .unwrap_or_default();
    for one in owed(&speakers, &published) {
        let at = root.join(&one.recording);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(root));
        let _ = std::fs::write(at, recorded(&one).to_string());
    }
}

/// The recording of `one`, as an adapter answering inside its contract would be recorded.
fn recorded(one: &Owed<'_>) -> serde_json::Value {
    let response = match one.case.expect {
        Expect::Status(statuses) => serde_json::json!({ "status": statuses.first() }),
        Expect::Conforms => {
            let refusal = Refusal::new(Kind::Unsupported, "a test plugin's adapter");
            serde_json::json!({
                "status": Kind::Unsupported.status(),
                "headers": { "content-type": PROBLEM },
                "json": refusal,
            })
        }
    };
    serde_json::json!({
        "recorded_from": one.pinned.as_deref().unwrap_or_default(),
        "note": "A test plugin's adapter, answering inside its contract.",
        "request": { "method": ASKED_WITH, "path": one.case.operation.path(), "accept": JSON },
        "response": response,
    })
}

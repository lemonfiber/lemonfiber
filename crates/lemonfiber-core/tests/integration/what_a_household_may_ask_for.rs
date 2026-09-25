//! What the request service will let the household ask for, driven through the HTTP
//! port against a fake transport.
//!
//! Driven from here rather than in-crate for the reason `seerr.rs` next door is: the
//! client speaks an async trait built on another, and a path exercised only from an
//! in-crate module is counted from the wrong copy.
//!
//! **Every fixture answers by route rather than in turn.** Two of these calls read
//! before they write and a third reads a document to write it back whole, so a queue
//! would prove only that the right number of requests went out — and the defect worth
//! catching here is a *narrow* body, which a queue cannot see at all.
//!
//! The scripted service and the installs that reach it are shared with the file next
//! door, which drives the other half of the same exchange: what becomes of the reason
//! a refusal carried.

use std::sync::Arc;

use lemonfiber_core::ports::http::Http;
use lemonfiber_core::seerr::Seerr;
use lemonfiber_fixtures::http::Fake;

use crate::common;

fn seerr(fake: &Arc<Fake>) -> Seerr {
    let http: Arc<dyn Http> = fake.clone();
    Seerr::new(http, "http://127.0.0.1:5055", "seerr")
}

/// The account identifier the request service files a member under.
const MEMBER: &str = "4";

// ── Through the dispatcher, as every surface reaches it ──────────────────────

mod deciding;
mod limits;

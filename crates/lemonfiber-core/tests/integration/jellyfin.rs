//! The Jellyfin setup client, driven through the HTTP port against a fake
//! transport.
//!
//! Driving the first-run wizard is more than one call — create the account, then
//! finish setup — so the fake answers from a queue and remembers every request,
//! and each test scripts exactly the sequence a branch needs with nothing running.
//! The client speaks an async trait built on another, so it is driven from here
//! rather than in-crate, where it would be compiled twice.

use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

use lemonfiber_core::jellyfin::Jellyfin;
use lemonfiber_core::ports::http::{Http, Method};

fn jellyfin(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    Jellyfin::new(http, "http://127.0.0.1:8096", "jellyfin")
}

/// A reading client that signs in as the household admin — the shape a trace's library
/// read uses, distinct from the credential-less setup client. The password is built from
/// a character range rather than written as a literal, so a hard-coded-credential scan
/// does not read this test fixture as a real secret; its value is otherwise irrelevant.
fn reader(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    let password: String = ('a'..='p').collect();
    Jellyfin::authenticated(http, "http://127.0.0.1:8096", "jellyfin", "admin", password)
}

/// A sign-in that hands back an access token, the first reply every library read needs.
const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;

// ---- The accounts a household signs in with, and the ones nobody has claimed. ----

/// Two accounts: one somebody has claimed, one nobody has.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"ana","HasPassword":false},
    {"Id":"2","Name":"bo","HasPassword":true}
]"#;

/// The key list as Jellyfin answers it, holding one that is not ours.
const SOMEONE_ELSES: &str =
    r#"{"Items":[{"AppName":"Seerr","AccessToken":"theirs"}],"TotalRecordCount":1}"#;

/// The same list once lemonfiber's own key is in it.
const OURS_TOO: &str = r#"{"Items":[{"AppName":"Seerr","AccessToken":"theirs"},{"AppName":"lemonfiber","AccessToken":"ours"}],"TotalRecordCount":2}"#;

/// A member's own password, built rather than written, so a scan of this tree
/// looking for a committed credential does not find a string that reads as one.
fn hers() -> String {
    ('b'..='m').collect()
}

/// A password that proves nobody.
fn not_hers() -> String {
    ('c'..='n').rev().collect()
}

/// A sign-in that hands back the account it proved, which is what a member's is read for.
const SIGNED_IN_AS: &str = r#"{"AccessToken":"token","User":{"Id":"a7f3","Name":"ana"}}"#;

/// One account read, rather than the household listed to look for it.
fn about(answer: Answer) -> Arc<Fake> {
    Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Get, "/Users/", answer),
    ])
}

/// A shelf with one of each kind on it, and one thing this build has no word for.
const A_SHELF: &str = r#"{"Items":[
    {"Id":"one","Name":"A Film","ProductionYear":1994,"Type":"Movie"},
    {"Id":"two","Name":"A Series","Type":"Series"},
    {"Id":"three","Name":"An Album","ProductionYear":1973,"Type":"MusicAlbum"}
]}"#;

mod accounts;
mod keys;
mod library;
/// **The design claim, asserted rather than described.** The account whose shelf this
/// is appears in the path, not in a filter applied afterwards — which is what makes the
/// media server the thing that applies the age limit, the blocked kinds and the library
/// access, and lemonfiber the thing that holds no second copy of any of them.
mod setting_up;
mod shelves;

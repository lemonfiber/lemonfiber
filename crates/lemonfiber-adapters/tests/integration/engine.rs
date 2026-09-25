//! The engine adapter, driven against an engine written for the purpose.
//!
//! This is the one part of this crate a trait fake cannot exercise. The
//! adapter's whole job is to speak the Engine API over a socket, so a fake
//! implementing `Engine` would prove only that the fake works. What gets
//! replaced here is the daemon: a socket answering with whatever a test wants
//! to say, which drives the connection, the request, the decoding and the
//! mapping in one pass — and needs no Docker installed to do it.
//!
//! It lives beside the crate rather than inside it because it is scaffolding
//! rather than product, and because scaffolding that must itself reach full
//! line coverage grows tests about the scaffolding.

use crate::fake;

/// Two containers as a listing, one running and one that fell over.
#[cfg(unix)]
const LISTING: &str = concat!(
    r#"[{"Id":"id-sonarr","#,
    r#""Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"sonarr"},"#,
    r#""State":"running","Status":"Up 2 minutes (healthy)","#,
    r#""Health":{"Status":"healthy"}},"#,
    r#"{"Id":"id-gluetun","#,
    r#""Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"gluetun"},"#,
    r#""State":"exited","Status":"Exited (137) 2 hours ago"}]"#
);

mod listing;
mod reaching;
mod streaming;

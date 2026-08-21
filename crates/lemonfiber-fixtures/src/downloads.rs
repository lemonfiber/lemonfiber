//! Download clients, answering the way the real ones do.
//!
//! Two clients speaking two unrelated protocols, and a test that wants to know what
//! is coming down has to stand up both — qBittorrent behind a login, `SABnzbd`
//! behind a key it writes into its own config file. Here rather than beside any one
//! test because more than one asks the same question of them: the dashboard, to fill
//! its transfers panel, and a teardown, to find out what stopping would interrupt.
//!
//! The payloads are deliberately awkward in the ways the real clients are. `SABnzbd`
//! reports its speed as a string that will not always parse, and a reader that treats
//! an unparsable speed as zero turns "I do not know" into "it is stalled" — so the
//! fixture keeps the `nan` a real one sends rather than a tidy number nobody would
//! have to handle.

use std::sync::Arc;

use crate::http::{Answer, Fake};

/// One qBittorrent torrent, thirty per cent done at a known speed with an estimate.
pub const QBIT_TORRENTS: &str =
    r#"[{"name":"Ubuntu.iso","completed":300,"size":1000,"dlspeed":4096,"eta":120}]"#;

/// One `SABnzbd` download, whose queue speed will not parse — so a reader has to say
/// its speed is unknown rather than report a false zero.
pub const SAB_QUEUE: &str = r#"{"queue":{"kbpersec":"nan","slots":[{"filename":"Linux.nzb","percentage":"20","status":"Downloading","timeleft":"0:05:00"}]}}"#;

/// A `SABnzbd` queue with nothing in it.
pub const SAB_EMPTY: &str = r#"{"queue":{"kbpersec":"0.0","slots":[]}}"#;

/// A qBittorrent list holding one torrent that has finished.
///
/// A client keeps completed items in the same list it reports active ones from, so
/// anything asking "what would stopping interrupt" has to tell them apart.
pub const QBIT_FINISHED: &str =
    r#"[{"name":"Done.iso","completed":1000,"size":1000,"dlspeed":0,"eta":0}]"#;

/// A `sabnzbd.ini` carrying a usable key.
pub const SAB_KEY_INI: &str = "[misc]\napi_key = sabkey123\n";

/// A `sabnzbd.ini` from a client that has not written its key yet.
pub const SAB_NO_KEY_INI: &str = "[misc]\nhost = 0.0.0.0\n";

/// A transport that answers each download client on its own path.
///
/// qBittorrent's login and its torrent list are matched by path; anything else is
/// `SABnzbd`'s queue, which is sound because that is the only other call a read of
/// these two clients makes.
#[must_use]
pub fn downloads(torrents: &'static str, queue: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        ("/auth/login", Answer::reply(200, "Ok.")),
        ("/torrents/info", Answer::reply(200, torrents)),
        ("", Answer::reply(200, queue)),
    ])
}

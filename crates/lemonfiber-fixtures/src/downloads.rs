//! The bundled torrent client, answering the way qBittorrent does.
//!
//! Here rather than beside any one test because more than one asks the same question of
//! it: the dashboard, to fill its transfers panel, and a teardown, to find out what
//! stopping would interrupt. The Usenet client is asked over its contract, through the
//! adapter its plugin brings, so a test answers it with the contract's own types.

use crate::http::Answer;

/// One qBittorrent torrent, thirty per cent done at a known speed with an estimate.
pub const QBIT_TORRENTS: &str =
    r#"[{"name":"Ubuntu.iso","completed":300,"size":1000,"dlspeed":4096,"eta":120}]"#;

/// A qBittorrent list holding one torrent that has finished.
///
/// A client keeps completed items in the same list it reports active ones from, so
/// anything asking "what would stopping interrupt" has to tell them apart.
pub const QBIT_FINISHED: &str =
    r#"[{"name":"Done.iso","completed":1000,"size":1000,"dlspeed":0,"eta":0}]"#;

/// The routes a read of the torrent client takes, answering its list with `torrents`:
/// its login, and its torrent list.
#[must_use]
pub fn torrents(torrents: &'static str) -> Vec<(&'static str, Answer)> {
    vec![
        ("/auth/login", Answer::reply(200, "Ok.")),
        ("/torrents/info", Answer::reply(200, torrents)),
    ]
}

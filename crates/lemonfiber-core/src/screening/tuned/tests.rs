use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_fixtures::scratch::Scratch;

use super::{closed, kept, read, renewed, theirs, Open};
use crate::app::Ctx;
use crate::ports::service::{Channel, Failure, Tuned, Tuning};
use crate::test_support::a_context;

/// A media server that closes every live stream but the one named `stuck`, and says
/// which it was asked to close.
#[derive(Default)]
struct Closing {
    closed: Mutex<Vec<String>>,
}

#[async_trait]
impl Tuning for Closing {
    async fn channels(&self, _member: &str) -> Result<Vec<Channel>, Failure> {
        Ok(Vec::new())
    }

    async fn tune(&self, _member: &str, _channel: &str) -> Result<Option<Tuned>, Failure> {
        Ok(None)
    }

    async fn untune(&self, closes_by: &str) -> Result<(), Failure> {
        if let Ok(mut closed) = self.closed.lock() {
            closed.push(closes_by.to_owned());
        }
        if closes_by == "stuck" {
            return Err(Failure::Unavailable {
                service: "media".to_owned(),
            });
        }
        Ok(())
    }
}

/// A context keeping what it opens in a directory of its own.
fn ctx(tag: &str) -> Ctx {
    let dir = Scratch::named(&format!("tuned-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(dir.join(".env"));
    ctx
}

/// A live stream opened for `member`, closed by `closes_by`, lapsing at `lapses`.
fn open(member: &str, closes_by: &str, lapses: u64) -> Open {
    Open {
        member: member.to_owned(),
        channel: "news".to_owned(),
        closes_by: closes_by.to_owned(),
        direct: Some("/live/0123456789abcdef0123456789abcdef/stream.ts".to_owned()),
        transcoded: None,
        streams: vec!["0123456789abcdef0123456789abcdef".to_owned()],
        issued: lapses,
        lapses,
    }
}

#[test]
fn a_live_stream_is_found_only_by_the_member_it_was_opened_for() {
    let ctx = ctx("theirs");
    kept(&ctx, open("a7f3", "ls1", 100));
    assert_eq!(
        theirs(&ctx, "a7f3", "0123456789abcdef0123456789abcdef").map(|one| one.closes_by),
        Some("ls1".to_owned())
    );
    assert_eq!(
        theirs(&ctx, "b8e4", "0123456789abcdef0123456789abcdef"),
        None
    );
    assert_eq!(theirs(&ctx, "a7f3", "another"), None);
}

#[test]
fn a_renewal_moves_only_the_lapse_and_keeps_no_pass() {
    let ctx = ctx("renewed");
    kept(&ctx, open("a7f3", "ls1", 100));
    renewed(&ctx, "ls1", 400);
    let kept = read(&ctx);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept.first().map(|one| (one.issued, one.lapses)),
        Some((100, 400))
    );
    let written = crate::app::targets::beside_env(&ctx, super::KEPT)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    assert!(
        !written.contains("lfpass") && !written.contains("pass\""),
        "{written}"
    );
}

#[tokio::test]
async fn a_live_stream_the_server_would_not_close_is_kept_for_the_next_sweep() {
    let ctx = ctx("closed");
    kept(&ctx, open("a7f3", "ls1", 100));
    kept(&ctx, open("a7f3", "stuck", 100));
    kept(&ctx, open("b8e4", "ls2", 100));
    let server = Closing::default();
    closed(&ctx, &server, |one| one.member == "a7f3").await;
    let asked = server
        .closed
        .lock()
        .map(|asked| asked.clone())
        .unwrap_or_default();
    assert_eq!(asked, vec!["ls1".to_owned(), "stuck".to_owned()]);
    let left: Vec<String> = read(&ctx).into_iter().map(|one| one.closes_by).collect();
    assert_eq!(left, vec!["stuck".to_owned(), "ls2".to_owned()]);
}

#[test]
fn with_nowhere_to_keep_them_live_streams_are_neither_read_nor_written() {
    let ctx = a_context().build();
    kept(&ctx, open("a7f3", "ls1", 100));
    assert!(read(&ctx).is_empty());
}

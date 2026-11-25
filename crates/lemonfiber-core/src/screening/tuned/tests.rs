use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::upstream::media::STUCK;
use lemonfiber_fixtures::upstream::Upstream;

use super::{closed, kept, lapsed, read, renewed, theirs, Open};
use crate::app::Ctx;
use crate::test_support::a_context;

/// A context keeping what it opens in a directory of its own.
fn ctx(tag: &str) -> Ctx {
    let dir_scratch = Scratch::named(&format!("tuned-{tag}"));
    let dir = dir_scratch.to_path_buf();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().holding(dir_scratch).build();
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
    kept(&ctx, open("a7f3", STUCK, 100));
    kept(&ctx, open("b8e4", "ls2", 100));
    let server = Upstream::default();
    let adas = |one: &Open| one.member == "a7f3";
    assert_eq!(closed(&ctx, &server, adas).await, 1);
    let left = || -> Vec<String> { read(&ctx).into_iter().map(|one| one.closes_by).collect() };
    assert_eq!(left(), vec![STUCK.to_owned(), "ls2".to_owned()]);
    assert_eq!(closed(&ctx, &server, adas).await, 0);
    assert_eq!(
        server.told(),
        ["ls1", STUCK, STUCK].map(|closes_by| format!("untune {closes_by}"))
    );
    assert_eq!(left(), vec![STUCK.to_owned(), "ls2".to_owned()]);
}

#[tokio::test]
async fn the_sweep_closes_nothing_with_no_stack_to_read_or_no_media_server_to_ask() {
    let unread = ctx("lapsed-unread");
    let mut unread = unread;
    kept(&unread, open("a7f3", "ls1", 0));
    unread.stack = crate::test_support::nowhere();
    let serverless = ctx("lapsed-serverless");
    kept(&serverless, open("a7f3", "ls1", 0));
    for ctx in [unread, serverless] {
        assert_eq!(lapsed(&ctx).await, 0);
        assert_eq!(read(&ctx).len(), 1);
    }
}

#[test]
fn with_nowhere_to_keep_them_live_streams_are_neither_read_nor_written() {
    let ctx = a_context().build();
    kept(&ctx, open("a7f3", "ls1", 100));
    assert!(read(&ctx).is_empty());
}

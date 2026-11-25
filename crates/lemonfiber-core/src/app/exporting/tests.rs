use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::remembering::Remembering;

use super::{cleared, Exported};
use crate::app::surfacing::Answer;
use crate::app::{dispatch, Command, Ctx, Exporting, Outcome, Surfacing};
use crate::bundle::Terms;
use crate::error::codes::bundle::{BUNDLE_LEAK, BUNDLE_NO_MARKS, BUNDLE_UNWRITTEN};
use crate::test_support::a_context;

fn asked(text: &str) -> Command {
    Command::Surface(Surfacing::Export(Exporting {
        text: text.to_owned(),
        written: 3,
    }))
}

fn named(ctx: &Ctx) -> PathBuf {
    let stamp = ctx
        .seams
        .clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    PathBuf::from(format!("lemonfiber-logs-{stamp}-3.txt"))
}

fn remembering() -> (Ctx, Arc<Remembering>) {
    let files = Arc::new(Remembering::default());
    let ctx = a_context()
        .build()
        .with_random(Arc::new(Chance::repeating(vec![7])))
        .with_filesystem(Arc::clone(&files) as Arc<_>);
    (ctx, files)
}

#[tokio::test]
async fn an_export_is_redacted_as_a_bundle_is_and_written_beside_where_the_viewer_ran() {
    let (ctx, files) = remembering();

    let answered = dispatch(
        asked("sonarr | GET /api/v3/series?apikey=letmein done"),
        &ctx,
    )
    .await;

    let path = named(&ctx);
    assert_eq!(
        answered.ok(),
        Some(Outcome::Surfaced(Answer::Exported(Exported {
            path: path.display().to_string(),
            rehearsed: false,
        })))
    );
    assert_eq!(path.parent(), Some(Path::new("")), "{path:?}");
    let written = files.at(&path).unwrap_or_default();
    assert!(!written.contains("letmein"), "{written}");
    assert!(written.contains("/api/v3/series?"), "{written}");
    assert!(written.ends_with("done\n"), "{written:?}");
}

#[tokio::test]
async fn a_rehearsed_export_says_where_it_would_go_and_writes_nothing() {
    let (ctx, files) = remembering();
    let ctx = ctx.rehearsing();

    let answered = dispatch(asked("sonarr | fine"), &ctx).await;

    let path = named(&ctx);
    assert_eq!(
        answered.ok(),
        Some(Outcome::Surfaced(Answer::Exported(Exported {
            path: path.display().to_string(),
            rehearsed: true,
        })))
    );
    assert_eq!(files.at(&path), None);
}

#[tokio::test]
async fn an_export_that_cannot_be_read_back_is_not_said_to_be_there() {
    let ctx = a_context()
        .build()
        .with_random(Arc::new(Chance::repeating(vec![7])))
        .with_filesystem(Files::empty());

    let refused = dispatch(asked("sonarr | fine"), &ctx).await.err();

    assert_eq!(refused.map(|problem| problem.code), Some(BUNDLE_UNWRITTEN));
}

#[tokio::test]
async fn a_machine_without_randomness_writes_no_export() {
    let (ctx, files) = remembering();
    let ctx = ctx.with_random(Arc::new(Chance::exactly(None)));

    let refused = dispatch(asked("sonarr | fine"), &ctx).await.err();

    assert_eq!(refused.map(|problem| problem.code), Some(BUNDLE_NO_MARKS));
    assert_eq!(files.at(&named(&ctx)), None);
}

#[test]
fn an_export_that_still_reads_as_a_credential_is_refused_naming_the_line() {
    let terms = Terms::default();
    let key: String = ('a'..='f').chain('0'..='9').cycle().take(32).collect();

    let refused = cleared(format!("sonarr | fine\nsonarr | the key {key}\n"), &terms).err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(BUNDLE_LEAK)
    );
    assert!(
        refused.is_some_and(|problem| problem.summary.contains("line 2")),
        "the line is named"
    );
    assert_eq!(
        cleared("sonarr | fine\n".to_owned(), &terms)
            .ok()
            .as_deref(),
        Some("sonarr | fine\n")
    );
}

#[tokio::test]
async fn nothing_a_caller_sends_reaches_the_path_an_export_is_written_to() {
    let (ctx, files) = remembering();
    let asked = Command::Surface(Surfacing::Export(Exporting {
        text: "../../etc/passwd\n/tmp/elsewhere.txt\nC:\\Windows\\win.ini".to_owned(),
        written: usize::MAX,
    }));

    let answered = dispatch(asked, &ctx).await;

    let said = match answered {
        Ok(Outcome::Surfaced(Answer::Exported(exported))) => exported.path,
        _ => String::new(),
    };
    let path = PathBuf::from(&said);
    let stamp = named(&ctx)
        .display()
        .to_string()
        .replace("-3.txt", &format!("-{}.txt", usize::MAX));
    assert_eq!(said, stamp);
    assert_eq!(path.components().count(), 1, "{path:?}");
    assert!(files.at(&path).is_some(), "written where it says");
    assert_eq!(files.at(Path::new("../../etc/passwd")), None);
}

#[tokio::test]
async fn an_export_is_read_back_from_the_file_it_wrote_and_no_other() {
    let ctx = a_context()
        .build()
        .with_random(Arc::new(Chance::repeating(vec![7])))
        .with_filesystem(Files::at(vec![
            (PathBuf::from("lemonfiber-logs.txt"), "an older export"),
            (
                PathBuf::from("../lemonfiber-logs-0-3.txt"),
                "somebody else's file",
            ),
        ]));

    let refused = dispatch(asked("sonarr | fine"), &ctx).await.err();

    assert_eq!(refused.map(|problem| problem.code), Some(BUNDLE_UNWRITTEN));
}

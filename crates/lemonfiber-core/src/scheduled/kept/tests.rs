use super::{locked, owe, read, tail, write, Kept, ScheduledRun, LINES};
use crate::instant::Instant;
use lemonfiber_fixtures::scratch::Scratch;

#[test]
fn a_record_reads_back_as_written_and_as_nothing_where_none_was() {
    let scratch = Scratch::new("record");
    let directory = scratch.path().join("scheduled");
    assert_eq!(read(&directory, "configarr"), Kept::default());
    let kept = Kept {
        considered: Some(Instant::from_seconds(60)),
        owed: false,
        last: Some(ScheduledRun {
            started: Instant::from_seconds(60),
            ended: Instant::from_seconds(90),
            exit: Some(1),
            log: vec!["refused".to_owned()],
        }),
        skipped: 2,
        last_skipped: Some(Instant::from_seconds(120)),
    };
    write(&directory, "configarr", &kept).unwrap_or_else(|why| unreachable!("written: {why:?}"));
    assert_eq!(read(&directory, "configarr"), kept);
}

#[test]
fn owing_marks_each_service_once() {
    let scratch = Scratch::new("owing");
    owe(scratch.path(), &["a".to_owned(), "b".to_owned()]);
    owe(scratch.path(), &["a".to_owned()]);
    assert!(read(scratch.path(), "a").owed);
    assert!(read(scratch.path(), "b").owed);
    assert!(!read(scratch.path(), "c").owed);
}

#[test]
fn only_the_last_lines_are_kept_and_blank_ones_never() {
    let said = (0..LINES + 5)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let kept = tail(&said);
    assert_eq!(kept.len(), LINES);
    assert_eq!(kept.first().map(String::as_str), Some("line 5"));
    assert_eq!(kept.last(), Some(&format!("line {}", LINES + 4)));
    assert!(tail("").is_empty());
}

#[test]
fn a_line_carrying_a_credential_is_kept_with_it_withheld() {
    let secret = ["abcdef", "1234", "567890"].concat();
    let kept = tail(&format!("syncing\ntoken={secret}\n"));
    assert_eq!(kept.first().map(String::as_str), Some("syncing"));
    assert!(kept.iter().all(|line| !line.contains(&secret)), "{kept:?}");
    assert!(
        kept.last()
            .is_some_and(|line| line.contains(crate::config::store::REDACTED)),
        "{kept:?}"
    );
}

#[test]
fn a_run_holding_the_lock_keeps_a_second_from_taking_it() {
    let scratch = Scratch::new("lock");
    let first =
        locked(scratch.path(), "configarr").unwrap_or_else(|why| unreachable!("opened: {why:?}"));
    assert!(first.is_some());
    assert!(locked(scratch.path(), "configarr")
        .unwrap_or_else(|why| unreachable!("opened: {why:?}"))
        .is_none());
    drop(first);
    assert!(locked(scratch.path(), "configarr")
        .unwrap_or_else(|why| unreachable!("opened: {why:?}"))
        .is_some());
}

#[test]
fn a_run_succeeded_only_where_it_exited_zero() {
    let ran = |exit| ScheduledRun {
        started: Instant::EPOCH,
        ended: Instant::EPOCH,
        exit,
        log: Vec::new(),
    };
    assert!(ran(Some(0)).succeeded());
    assert!(!ran(Some(2)).succeeded());
    assert!(!ran(None).succeeded());
}

#[test]
fn a_record_or_a_lock_where_a_file_stands_for_its_directory_is_refused() {
    let scratch = Scratch::new("blocked");
    let directory = scratch.path().join("scheduled");
    let _ = std::fs::write(&directory, "");
    assert!(write(&directory, "configarr", &Kept::default()).is_err());
    assert!(locked(&directory, "configarr").is_err());
    let _ = std::fs::remove_file(&directory);
    let _ = std::fs::create_dir_all(directory.join("configarr.lock"));
    assert!(locked(&directory, "configarr").is_err());
}

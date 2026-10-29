use super::{Failure, Local, Progress, Runner};

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

/// Drain a stream into its lines and its final exit status.
async fn drain(runner: &Local, parts: &[&str]) -> (Vec<String>, Option<i32>) {
    let Ok(mut stream) = runner.stream(&argv(parts)).await else {
        return (Vec::new(), None);
    };
    let mut lines = Vec::new();
    let mut status = None;
    while let Some(event) = stream.recv().await {
        match event {
            Progress::Line(line) => lines.push(line),
            Progress::Ended(code) => status = code,
        }
    }
    (lines, status)
}

#[tokio::test]
async fn streams_a_programs_lines_then_its_exit() {
    // Both streams are read: a line to stdout and one to stderr, then a clean
    // exit. Order between the two is not asserted — only that both arrive.
    let (mut lines, status) = drain(
        &Local,
        &["sh", "-c", "printf 'out\\n'; printf 'err\\n' >&2"],
    )
    .await;
    lines.sort();
    assert_eq!(lines, vec!["err".to_owned(), "out".to_owned()]);
    assert_eq!(status, Some(0));
}

#[tokio::test]
async fn draining_a_program_that_cannot_start_yields_nothing() {
    let (lines, status) = drain(&Local, &["lemonfiber-no-such-program"]).await;
    assert!(lines.is_empty());
    assert_eq!(status, None);
}

#[tokio::test]
async fn a_non_zero_exit_arrives_on_the_stream_not_as_an_error() {
    let (_, status) = drain(&Local, &["sh", "-c", "exit 3"]).await;
    assert_eq!(
        status,
        Some(3),
        "the failed run's status still reaches the reader"
    );
}

#[tokio::test]
async fn streaming_a_missing_program_is_distinguished_from_a_broken_one() {
    let result = Local.stream(&argv(&["lemonfiber-no-such-program"])).await;
    assert!(matches!(result, Err(Failure::NotFound { .. })));
}

#[tokio::test]
async fn streaming_something_that_cannot_be_spawned_is_unusable_not_missing() {
    // A directory is present but not executable, so spawning it fails with a
    // reason other than "not found".
    let result = Local.stream(&argv(&["/"])).await;
    assert!(matches!(result, Err(Failure::Unusable { .. })));
}

#[tokio::test]
async fn streaming_nothing_is_refused_rather_than_spawned() {
    assert!(matches!(
        Local.stream(&[]).await,
        Err(Failure::Unusable { .. })
    ));
}

#[tokio::test]
async fn collects_what_a_program_wrote_and_how_it_exited() {
    let spoken = Local
        .run(&argv(&["echo", "hello"]))
        .await
        .map(|output| (output.succeeded(), output.stdout.trim().to_owned()))
        .ok();
    assert_eq!(spoken, Some((true, "hello".to_owned())));
}

#[tokio::test]
async fn a_non_zero_exit_is_a_result_rather_than_an_error() {
    let outcome = Local
        .run(&argv(&["false"]))
        .await
        .map(|output| output.succeeded())
        .ok();
    assert_eq!(outcome, Some(false), "a failed program still ran");
}

#[tokio::test]
async fn a_missing_program_is_distinguished_from_a_broken_one() {
    let result = Local
        .run(&argv(&["lemonfiber-no-such-program-exists"]))
        .await;
    assert!(matches!(result, Err(Failure::NotFound { .. })));
}

#[tokio::test]
async fn an_empty_argument_vector_is_refused_rather_than_spawned() {
    assert!(matches!(
        Local.run(&[]).await,
        Err(Failure::Unusable { .. })
    ));
}

/// A program that outlasts the bound is stopped, and the run ends without an exit
/// status rather than waiting for as long as the program chooses.
#[tokio::test(start_paused = true)]
async fn a_program_that_outlasts_the_bound_is_stopped() {
    let outcome = Local
        .run(&argv(&["sh", "-c", "sleep 30"]))
        .await
        .map(|output| output.status)
        .ok();
    assert_eq!(outcome, Some(None), "stopped, so there is no exit status");
}

/// A streamed program that outlasts the bound is stopped as a run is, and its stream
/// ends without an exit status.
#[tokio::test(start_paused = true)]
async fn a_streamed_program_that_outlasts_the_bound_is_stopped() {
    let (_, status) = drain(&Local, &["sh", "-c", "sleep 30"]).await;
    assert_eq!(status, None, "stopped, so there is no exit status");
}

/// Whether the process `pid` names is still there to be signalled.
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A stream whose reader goes away takes the program with it.
///
/// Work ended part-way drops what it was reading, and a start or a pull nobody is
/// waiting on any more must not go on changing the stack behind everybody's back.
#[tokio::test]
async fn a_stream_whose_reader_goes_away_takes_the_program_with_it() {
    let Ok(mut stream) = Local
        .stream(&argv(&["sh", "-c", "echo $$; exec sleep 30"]))
        .await
    else {
        unreachable!("sh is on every machine this runs on");
    };
    let pid = match stream.recv().await {
        Some(Progress::Line(pid)) => pid,
        other => unreachable!("the program says its pid first: {other:?}"),
    };
    assert!(alive(&pid), "the program is running while it is read");

    drop(stream);
    let mut gone = false;
    for _ in 0..100 {
        if !alive(&pid) {
            gone = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(gone, "the program was stopped once nobody was reading it");
}

/// A variable handed to the run is one the program reads.
#[tokio::test]
async fn a_run_with_variables_sets_each_of_them() {
    let ran = Local
        .run_with(
            &argv(&["sh", "-c", "printf '%s' \"$LEMONFIBER_PROBE\""]),
            &[("LEMONFIBER_PROBE".to_owned(), "set".to_owned())],
        )
        .await
        .map(|output| output.stdout);
    assert_eq!(ran.ok().as_deref(), Some("set"));
}

/// A run from a directory starts the program there.
#[tokio::test]
async fn a_run_from_a_directory_starts_there() {
    let dir = std::env::temp_dir().canonicalize().ok();
    let ran = match &dir {
        Some(dir) => Local
            .run_from(&argv(&["pwd", "-P"]), dir)
            .await
            .ok()
            .map(|output| std::path::PathBuf::from(output.stdout.trim_end())),
        None => None,
    };
    assert!(ran.is_some());
    assert_eq!(ran, dir);
}

/// A run given up on takes its child with it, rather than leaving it running.
#[tokio::test]
async fn a_run_given_up_on_ends_its_child() {
    let marker = std::env::temp_dir().join(format!("lemonfiber-kill-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    let script = format!("sleep 1; touch {}", marker.display());
    let given_up = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        Local.run(&argv(&["sh", "-c", &script])),
    )
    .await;
    assert!(given_up.is_err());
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert!(!marker.exists());
}

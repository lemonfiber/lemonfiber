use super::{after, complaint, definition, put, status, take, Host, Unhosted};
use lemonfiber_ports::hosting::{Failure, Hosted};
use lemonfiber_ports::process::Output;
use std::path::PathBuf;

fn spoke(status: Option<i32>, stdout: &str, stderr: &str) -> Output {
    Output {
        status,
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

fn a_command() -> Hosted {
    Hosted {
        name: "watch".to_owned(),
        program: PathBuf::from("/usr/local/bin/lemonfiber"),
        arguments: vec!["watch".to_owned()],
        output: PathBuf::from("/records/watch.log"),
        about: "guards the data location".to_owned(),
    }
}

#[tokio::test]
async fn a_platform_with_no_manager_refuses_every_operation_the_same_way() {
    assert_eq!(
        Unhosted.manager(),
        lemonfiber_ports::hosting::Manager::Unsupported
    );
    assert_eq!(Unhosted.place(&a_command()).await, Err(Failure::Unhostable));
    assert_eq!(Unhosted.standing("watch").await, Err(Failure::Unhostable));
    assert_eq!(Unhosted.withdraw("watch").await, Err(Failure::Unhostable));
}

#[test]
fn a_definition_is_written_read_back_and_taken_away_again() {
    let at_dir = super::scratch("round-trip");
    let at = at_dir.join("nested/one.service");
    assert_eq!(definition(&at), None);
    assert_eq!(put(&at, "[Service]\n"), Ok(()));
    assert_eq!(definition(&at).as_deref(), Some("[Service]\n"));
    assert_eq!(take(&at), Ok(()));
    assert_eq!(definition(&at), None);
}

#[test]
fn a_definition_that_will_not_be_written_carries_the_platforms_words() {
    let at_dir = super::scratch("unwritable");
    let at = at_dir.join("one.service");
    assert!(put(&at, "[Service]\n").is_ok());
    // A directory cannot be written over as a file, which is the shape of every
    // refusal here: the platform says why, and its words travel unchanged.
    let over = at.join("beneath.service");
    assert!(matches!(
        put(&over, "x"),
        Err(Failure::Unwritable { at: refused, .. }) if refused == over
    ));
    assert!(matches!(
        take(&at.join("absent")),
        Err(Failure::Unwritable { .. })
    ));
}

/// The two ways writing a definition fails are different failures.
///
/// One is the directory it goes in and the other is the file itself, and only
/// the second is reached where the directory was made without complaint — so a
/// test that only ever blocked the directory left the write's own refusal
/// carrying words nothing had read.
#[test]
fn a_definition_the_file_itself_refuses_carries_the_platforms_words() {
    let dir = super::scratch("write-refused");
    let at = dir.join("one.service");
    // A directory standing where the file goes: the directory above it is made
    // without complaint, and the write is what refuses.
    let _ = std::fs::create_dir_all(&at);
    assert!(matches!(
        put(&at, "[Service]\n"),
        Err(Failure::Unwritable { at: refused, .. }) if refused == at
    ));
}

#[test]
fn a_refusal_quotes_the_error_channel_first_and_the_rest_after() {
    assert_eq!(
        complaint(&spoke(Some(1), "out", "bad domain")),
        "bad domain"
    );
    assert_eq!(complaint(&spoke(Some(1), "out", "  ")), "out");
    assert_eq!(complaint(&spoke(Some(5), "", "")), "it exited 5");
    assert_eq!(complaint(&spoke(None, "", "")), "it exited on a signal");
}

#[test]
fn how_a_program_ended_is_a_number_or_the_absence_of_one() {
    assert_eq!(status(&spoke(Some(0), "", "")), "0");
    assert_eq!(status(&spoke(None, "", "")), "on a signal");
}

#[test]
fn the_value_after_a_key_is_the_first_one_and_nothing_where_there_is_none() {
    let text = "Description=one\nExecStart=/bin/lemonfiber watch\nExecStart=/second\n";
    assert_eq!(
        after(text, "ExecStart=").as_deref(),
        Some("/bin/lemonfiber watch")
    );
    assert_eq!(after(text, "Restart=").as_deref(), None);
}

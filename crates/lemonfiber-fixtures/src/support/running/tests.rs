use super::*;

/// A call no key names, where nothing was said for everything else, finds no program.
#[tokio::test]
async fn a_call_no_key_names_and_nothing_else_answers_finds_no_program() {
    let runner = Keyed::by_command(vec![("docker compose up", Ok(spoke("")))], None);

    let ran = runner.run(&["podman".to_owned(), "ps".to_owned()]).await;

    assert!(matches!(ran, Err(Failure::NotFound { program }) if program == "podman"));
    assert!(runner.ran("podman"));
}

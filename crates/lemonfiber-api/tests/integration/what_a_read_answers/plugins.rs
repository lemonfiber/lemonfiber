//! What is installed, and what the stack wires to what, read.

use lemonfiber_core::app::plugins::Asked as Installing;
use lemonfiber_core::app::Linking;

use super::reading::*;

/// A world whose record of what is installed holds `record`, beside its settings.
fn recording(named: &str, record: &str) -> Ctx {
    let ctx = configured(named, "LEMONFIBER_USENET=on\n");
    if let Some(env) = ctx.settings.env_file.as_deref() {
        let _ = std::fs::write(env.with_file_name("plugins.json"), record);
    }
    ctx
}

/// Whether an answer is the refusal a read gives for what it could not read: the
/// failure of the machine, in the error envelope, carrying `code`.
fn unread(seen: Option<&(StatusCode, String)>, code: &str) -> bool {
    seen.is_some_and(|(status, body)| {
        *status == StatusCode::INTERNAL_SERVER_ERROR
            && body.starts_with(r#"{"api_version":1,"kind":"error""#)
            && body.contains(&format!(r#""code":"{code}""#))
    })
}

#[tokio::test]
async fn what_is_installed_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &recording("plugins-none", ""),
        Command::Plugins(Installing::Installed),
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(recording("plugins-none", ""), table::PLUGINS).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_machine_with_nothing_installed_is_answered_with_an_empty_list() {
    // Written out rather than derived, so a second serialisation could not pass this
    // by agreeing with itself.
    let seen = asked(recording("plugins-empty", ""), table::PLUGINS).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"plugins","data":{"#)
            && body.contains(r#""installed":[]"#)),
        "nothing installed is an empty list under the plugins kind"
    );
}

#[tokio::test]
async fn a_record_that_will_not_read_is_refused_rather_than_answered_as_empty() {
    // The distinction the read exists to keep: an empty list says nothing is
    // installed, and a stranger's service may be running behind a damaged record.
    let seen = asked(recording("plugins-damaged", "{ not json"), table::PLUGINS).await;
    assert!(unread(seen.as_ref(), "PLUGIN-4"), "{seen:?}");
    assert!(
        !seen.is_some_and(|(_, body)| body.contains(r#""installed":[]"#)),
        "no empty list rides beside the refusal"
    );
}

#[tokio::test]
async fn what_the_stack_wires_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &recording("wiring-read", ""),
        Command::Wiring(Linking::Read),
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(recording("wiring-read", ""), table::WIRING).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_wiring_is_carried_in_its_own_envelope() {
    let seen = asked(recording("wiring-kind", ""), table::WIRING).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"wiring","data":{"#)
            && body.contains(r#""unfilled":"#)),
        "every link and what nothing fills, under the wiring kind"
    );
}

#[tokio::test]
async fn a_wiring_over_a_record_that_will_not_read_is_refused() {
    // A plugin's service may be a claimant, so a wiring that left the record out
    // would settle a contest nobody was told about.
    let seen = asked(recording("wiring-damaged", "{ not json"), table::WIRING).await;
    assert!(unread(seen.as_ref(), "PLUGIN-4"), "{seen:?}");
}

#[tokio::test]
async fn a_wiring_over_a_stack_that_will_not_read_is_refused() {
    let seen = asked(world(running(), nowhere()), table::WIRING).await;
    assert!(unread(seen.as_ref(), "STACK-1"), "{seen:?}");
}

#[tokio::test]
async fn neither_read_takes_a_parameter() {
    for read in [table::PLUGINS, table::WIRING] {
        let seen = asked(recording("plugins-asked", ""), &format!("{read}?plugin=x")).await;
        assert!(
            seen.as_ref()
                .is_some_and(|(status, _)| *status == StatusCode::BAD_REQUEST),
            "{read}: {seen:?}"
        );
    }
}

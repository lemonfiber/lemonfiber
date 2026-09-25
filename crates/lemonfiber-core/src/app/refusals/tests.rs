use super::{keep, load, NAME};
use crate::config::Settings;
use crate::test_support::{a_context, a_password, env_at};

/// A context whose settings point at a scratch install of its own.
fn ctx(name: &str) -> super::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env_at(name, &a_password())),
            ..Settings::default()
        })
        .build()
}

/// A reason written down is there for the next run to read.
#[test]
fn a_reason_written_down_is_read_back() {
    let ctx = ctx("kept");
    let mut held = load(&ctx);
    assert!(held.is_empty(), "a fresh install holds a refusal");

    held.keep(41, "we already have it dubbed", None);
    assert!(keep(&ctx, &held).is_ok());

    assert_eq!(
        load(&ctx).of(41).map(|kept| kept.reason.as_str()),
        Some("we already have it dubbed")
    );
}

/// The record sits beside the settings, which is what a backup carries.
#[test]
fn the_record_sits_with_the_settings() {
    let ctx = ctx("beside");
    assert!(keep(&ctx, &load(&ctx)).is_ok());

    assert!(
        ctx.settings
            .env_file
            .as_deref()
            .is_some_and(|env| env.with_file_name(NAME).exists()),
        "nothing was written where the next run would look"
    );
}

/// An install with nowhere to keep them holds none, and says so on the way out.
///
/// A machine with no settings has turned nothing down either, so reading is nothing
/// rather than a failure. Writing is the other way round: the words are the only copy
/// there is, and losing them in silence is the thing this record exists to stop.
#[test]
fn nowhere_to_keep_them_reads_as_none_and_refuses_to_be_written() {
    let nowhere = a_context().build();

    assert!(load(&nowhere).is_empty());
    assert!(keep(&nowhere, &load(&nowhere)).is_err());
}

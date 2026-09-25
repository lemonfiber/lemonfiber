use super::{keep, load, NAME};
use crate::asking::Expiry;
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

/// What was agreed to is there for the next run and the next reading.
#[test]
fn what_was_agreed_to_is_read_back() {
    let ctx = ctx("arranged");
    assert_eq!(load(&ctx).after(), None, "a fresh install closes something");

    assert!(keep(&ctx, &Expiry::agreed_to(30, None)).is_ok());

    assert_eq!(load(&ctx).after(), Some(30));
}

/// Withdrawing it puts the household back where it started.
#[test]
fn withdrawing_it_puts_the_household_back_where_it_started() {
    let ctx = ctx("withdrawn");
    assert!(keep(&ctx, &Expiry::agreed_to(30, None)).is_ok());

    assert!(keep(&ctx, &Expiry::default()).is_ok());

    assert_eq!(load(&ctx).after(), None);
}

/// It sits beside the settings, which is what a backup carries.
#[test]
fn it_sits_with_the_settings() {
    let ctx = ctx("beside-settings");
    assert!(keep(&ctx, &Expiry::default()).is_ok());

    assert!(
        ctx.settings
            .env_file
            .as_deref()
            .is_some_and(|env| env.with_file_name(NAME).exists()),
        "nothing was written where the next run would look"
    );
}

/// An install with nowhere to keep it has agreed to nothing, and refuses to agree.
///
/// A machine with no settings has arranged nothing either, so reading is nothing
/// rather than a failure. Writing is the other way round: an operator told their
/// household closes requests after thirty days by a run that recorded it nowhere
/// would be told the opposite by every reading afterwards.
#[test]
fn nowhere_to_keep_it_reads_as_none_and_refuses_to_be_written() {
    let nowhere = a_context().build();

    assert_eq!(load(&nowhere).after(), None);
    assert!(keep(&nowhere, &Expiry::agreed_to(30, None)).is_err());
}

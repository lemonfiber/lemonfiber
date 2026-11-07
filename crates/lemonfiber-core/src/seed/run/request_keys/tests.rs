use std::os::unix::fs::PermissionsExt as _;

use lemonfiber_contract::adapter::REQUEST_KEY_FILE;
use lemonfiber_fixtures::support::FixedRandom;

use super::Keys;
use crate::ports::digest::sha256;
use crate::test_support::{a_context, a_minting_curator, first_party, stack_fillers};
use crate::wiring::Filler;

/// The curator's adapter, written in a stack of its own named `name`, and that stack.
fn an_adapter(name: &str) -> (Filler, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let fillers = stack_fillers(
        Vec::new(),
        &[a_minting_curator()],
        Some(&at),
        &first_party("series"),
    );
    let adapter = fillers.service("series").cloned();
    assert!(adapter.is_some());
    let Some(adapter) = adapter else {
        unreachable!("asserted above");
    };
    let held = adapter.confined_to.clone().unwrap_or_default();
    let _ = std::fs::create_dir_all(&held);
    (adapter, held)
}

/// A context with randomness where `random`.
fn ctx(random: bool) -> crate::app::Ctx {
    a_context()
        .build()
        .with_random(std::sync::Arc::new(FixedRandom(
            random.then(|| vec![0xab; crate::secret::SECRET_BYTES]),
        )))
}

/// What is written where the gate's keys are, beneath `held`.
fn written(held: &std::path::Path) -> String {
    std::fs::read_to_string(held.join(REQUEST_KEY_FILE)).unwrap_or_default()
}

#[test]
fn keys_are_read_one_to_a_line_and_found_by_what_the_gate_presents() {
    let (adapter, held) = an_adapter("request-keys-read");
    let ctx = ctx(true);

    let none = Keys::of(&ctx, &adapter);
    assert!(none.is_ok_and(|keys| keys.held.is_empty()));

    let _ = std::fs::write(held.join(REQUEST_KEY_FILE), "one\n\n  two  \n");
    let keys = Keys::of(&ctx, &adapter);
    assert!(keys.is_ok());
    let Ok(keys) = keys else {
        return;
    };
    assert_eq!(keys.presented(&sha256("two")), Some("two"));
    assert_eq!(keys.presented(&sha256("three")), None);
}

#[test]
fn a_new_key_stands_in_place_of_those_held_or_beside_the_one_kept_until_it_alone_is_held() {
    let (adapter, held) = an_adapter("request-keys-written");
    let ctx = ctx(true);
    let new = crate::secret::render(&[0xab; crate::secret::SECRET_BYTES]);
    let _ = std::fs::write(held.join(REQUEST_KEY_FILE), "stale\nstaler\n");
    let keys = Keys::of(&ctx, &adapter);
    assert!(keys.is_ok());
    let Ok(mut keys) = keys else {
        return;
    };

    assert_eq!(keys.fresh(&ctx).as_ref(), Ok(&new));
    assert_eq!(written(&held), format!("{new}\n"));
    assert_eq!(
        std::fs::metadata(held.join(REQUEST_KEY_FILE))
            .map(|one| one.permissions().mode() & 0o777)
            .ok(),
        Some(0o600)
    );

    assert_eq!(keys.beside(&ctx, "kept").as_ref(), Ok(&new));
    assert_eq!(written(&held), format!("kept\n{new}\n"));
    assert_eq!(keys.presented(&sha256("kept")), Some("kept"));

    assert_eq!(keys.only(&ctx, "kept"), Ok(()));
    assert_eq!(written(&held), "kept\n");
    assert_eq!(keys.presented(&sha256(&new)), None);
}

#[test]
fn no_key_is_written_without_randomness() {
    let (adapter, held) = an_adapter("request-keys-unminted");
    let ctx = ctx(false);
    let keys = Keys::of(&ctx, &adapter);
    assert!(keys.is_ok());
    let Ok(mut keys) = keys else {
        return;
    };

    assert_eq!(
        keys.fresh(&ctx),
        Err(crate::secret::NO_RANDOMNESS_FOR_TOKEN.to_owned())
    );
    assert_eq!(
        keys.beside(&ctx, "kept"),
        Err(crate::secret::NO_RANDOMNESS_FOR_TOKEN.to_owned())
    );
    assert_eq!(written(&held), "");
}

#[test]
fn what_cannot_be_read_or_written_is_said_and_nothing_is_followed() {
    let (adapter, held) = an_adapter("request-keys-refused");
    let ctx = ctx(true);

    let mut homeless = adapter.clone();
    homeless.confined_to = None;
    assert!(Keys::of(&ctx, &homeless)
        .err()
        .is_some_and(|why| why.contains("no configuration on this machine")));

    let keys = Keys::of(&ctx, &adapter);
    assert!(keys.is_ok());
    let Ok(mut keys) = keys else {
        return;
    };
    let _ = std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o500));
    let refused = keys.fresh(&ctx);
    let _ = std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o700));
    assert!(
        refused
            .as_ref()
            .err()
            .is_some_and(|why| why.starts_with("the request gate's key could not be written")),
        "{refused:?}"
    );

    let elsewhere = held.join("elsewhere");
    let _ = std::fs::write(&elsewhere, "not-the-gates\n");
    let _ = std::os::unix::fs::symlink(&elsewhere, held.join(REQUEST_KEY_FILE));
    assert!(Keys::of(&ctx, &adapter)
        .err()
        .is_some_and(|why| why.contains("not a plain file of its own")));
}

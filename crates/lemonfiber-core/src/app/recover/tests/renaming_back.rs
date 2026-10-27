//! Putting a value moved to the key that says what it holds back under the key an
//! earlier release wrote.

use lemonfiber_adapters::Disk;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::undo;
use crate::config::store;
use crate::error::codes::setup::NOT_PUT_BACK;
use crate::journal::{Action, Undo};

const WAS: &str = "JELLYFIN_MODE";
const NOW: &str = "MEDIA_SERVER_MODE";

fn back() -> Undo {
    Undo {
        target: NOW.to_owned(),
        action: Action::RenameBack {
            from: WAS.to_owned(),
            to: NOW.to_owned(),
        },
    }
}

fn written(env: &std::path::Path, key: &str) -> Option<String> {
    store::read(env)
        .ok()
        .and_then(|file| file.written(key).map(str::to_owned))
}

#[test]
fn a_moved_value_goes_back_under_the_key_it_left() {
    let dir = Scratch::unmade("renaming-back-plain");
    let env = dir.join(".env");
    assert!(store::set(&env, NOW, "native").is_ok());

    assert!(undo(&Disk, &[back()], &env, Vec::new()).is_ok());

    assert_eq!(written(&env, WAS).as_deref(), Some("native"));
    assert_eq!(written(&env, NOW), None);
}

#[test]
fn a_value_moved_back_already_is_left_as_it_is() {
    let dir = Scratch::unmade("renaming-back-twice");
    let env = dir.join(".env");
    assert!(store::set(&env, "TZ", "Europe/Amsterdam").is_ok());

    assert!(undo(&Disk, &[back()], &env, Vec::new()).is_ok());

    assert_eq!(written(&env, WAS), None);
    assert_eq!(written(&env, NOW), None);
}

#[test]
fn a_key_written_again_since_is_named_and_left() {
    let dir = Scratch::unmade("renaming-back-theirs");
    let env = dir.join(".env");
    assert!(store::set(&env, WAS, "docker").is_ok());
    assert!(store::set(&env, NOW, "native").is_ok());

    let again = undo(&Disk, &[back()], &env, Vec::new());

    assert!(
        matches!(&again, Err(problem) if problem.code == NOT_PUT_BACK),
        "{again:?}"
    );
    assert_eq!(written(&env, WAS).as_deref(), Some("docker"));
    assert_eq!(written(&env, NOW).as_deref(), Some("native"));
}

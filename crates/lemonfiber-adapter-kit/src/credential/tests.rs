use std::path::PathBuf;

use lemonfiber_contract::adapter::{PASSWORD_MARKS, UPSTREAM_FILE};

use super::{spelled_as_a_key, Holding, Upstream, KEY_LIMIT};

/// The field these tests read the key from.
const API_KEY: &str = "api_key";

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "adapter-kit-credential-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn a_key() -> String {
    ('d'..='q').collect()
}

#[test]
fn the_file_lemonfiber_wrote_is_read_as_the_key() {
    let dir = directory("read");
    let key = a_key();
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        format!(r#"{{"{API_KEY}":"{key}"}}"#),
    );
    assert_eq!(Upstream::in_directory(&dir, API_KEY).key(), Ok(key));
}

#[test]
fn a_missing_file_is_refused_by_name() {
    let dir = directory("missing");
    let refused = Upstream::in_directory(&dir, API_KEY).key();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.starts_with("upstream.json could not be read")),
        "{refused:?}"
    );
}

#[test]
fn a_file_that_is_not_an_object_of_strings_is_refused_without_quoting_it() {
    let dir = directory("unreadable");
    let key = a_key();
    for written in [
        format!("api_key={key}"),
        format!(r#"{{"api_key":["{key}"]}}"#),
    ] {
        let _ = std::fs::write(dir.join(UPSTREAM_FILE), written);
        assert_eq!(
            Upstream::in_directory(&dir, API_KEY).key(),
            Err("upstream.json is not an object of named strings".to_owned())
        );
    }
}

#[test]
fn an_empty_or_missing_key_is_no_key_and_is_named() {
    let dir = directory("empty");
    for written in [r#"{"api_key":""}"#, r#"{"username":"admin"}"#] {
        let _ = std::fs::write(dir.join(UPSTREAM_FILE), written);
        assert_eq!(
            Upstream::in_directory(&dir, API_KEY).key(),
            Err("upstream.json names no api_key".to_owned())
        );
    }
}

#[test]
fn a_key_spelled_as_no_key_is_refused_without_quoting_it() {
    let dir = directory("misspelled");
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        r#"{"api_key":"abc&mode=shutdown"}"#,
    );
    let refused = Upstream::in_directory(&dir, API_KEY).key();
    assert_eq!(
        refused,
        Err("the api_key in upstream.json is not spelled as a key is".to_owned())
    );
}

#[test]
fn a_file_that_cannot_be_read_is_refused_by_name() {
    let dir = directory("directory");
    let _ = std::fs::create_dir_all(dir.join(UPSTREAM_FILE));
    let refused = Upstream::in_directory(&dir, API_KEY).key();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.starts_with("upstream.json could not be")),
        "{refused:?}"
    );
}

#[test]
fn a_key_is_letters_digits_and_the_marks_keys_are_written_in() {
    assert!(spelled_as_a_key("0123456789abcdef0123456789abcdef"));
    assert!(spelled_as_a_key("aB3-_.+/="));
    assert!(spelled_as_a_key(&"k".repeat(KEY_LIMIT)));
    for not_a_key in [
        String::new(),
        "k".repeat(KEY_LIMIT + 1),
        "a key".to_owned(),
        "a&b".to_owned(),
        "a\nb".to_owned(),
        "clé".to_owned(),
    ] {
        assert!(!spelled_as_a_key(&not_a_key), "{not_a_key:?}");
    }
}

#[test]
fn a_password_lemonfiber_mints_is_spelled_as_a_key_whichever_mark_it_holds() {
    for mark in PASSWORD_MARKS {
        let password = format!("Ab3{}Ab3Ab3Ab3Ab3Ab3Ab3Ab3Ab3Ab3A", char::from(*mark));
        assert!(spelled_as_a_key(&password), "{}", char::from(*mark));
    }
    assert!(
        !spelled_as_a_key("Ab3!Ab3"),
        "a mark no password holds is refused"
    );
}

#[test]
fn an_email_lemonfiber_minted_is_read_as_it_was_written() {
    let dir = directory("minted-email");
    let email = "0123456789abcdef0123456789abcdef@komga.invalid";
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        format!(r#"{{"username":"{email}"}}"#),
    );
    assert_eq!(
        Upstream::in_directory(&dir, "username").key(),
        Ok(email.to_owned())
    );
}

#[test]
fn an_address_lemonfiber_did_not_mint_is_refused_without_quoting_it() {
    let dir = directory("unminted-email");
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        r#"{"username":"admin@example.com"}"#,
    );
    assert_eq!(
        Upstream::in_directory(&dir, "username").key(),
        Err("the username in upstream.json is not spelled as a key is".to_owned())
    );
}

#[test]
fn a_key_held_in_hand_is_that_key() {
    assert_eq!(a_key().key(), Ok(a_key()));
}

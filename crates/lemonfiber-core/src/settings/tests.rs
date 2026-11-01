use std::path::{Path, PathBuf};
use std::time::Duration;

use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::scratch::Scratch;

use super::secrets::Keeping;
use super::undoing::Undone;
use super::{kept, named, Secrets, Settings, Source};
use crate::config::store::Failure;
use crate::credential::fingerprint;

mod changing;
mod choosing;
mod credentials;
mod files;
mod filing;
mod granting;
mod own;
mod reaching;
mod stacked;
mod waiting;

fn secrets_in(scratch: &Scratch) -> PathBuf {
    scratch.path().join("config").join("secrets.json")
}

fn keeping<'a>(name: &'a str, value: &'a str) -> Keeping<'a> {
    Keeping {
        name,
        value,
        from: Source::Recipe,
        at: 7,
    }
}

fn kept_now(path: &Path, name: &str, value: &str) -> Result<(), Failure> {
    kept(path, &[keeping(name, value)], &Chance::cycling(), &|_| {
        Ok(())
    })
}

/// The fingerprint the store at `path` keys `value` with.
fn keyed(path: &Path, value: &str) -> String {
    Secrets::at(path)
        .ok()
        .and_then(|secrets| secrets.keyed(value))
        .unwrap_or_default()
}

fn value(path: &Path, name: &str) -> Option<String> {
    Secrets::at(path)
        .ok()
        .and_then(|secrets| secrets.value(name).map(str::to_owned))
}

#[test]
fn a_secret_kept_reads_back_and_its_name_is_namespaced() {
    let scratch = Scratch::new("secrets-round-trip");
    let path = secrets_in(&scratch);
    let name = named("komga", "token");
    assert_eq!(name, "komga:token");
    assert!(kept_now(&path, &name, "t0k3n").is_ok());
    assert_eq!(value(&path, &name).as_deref(), Some("t0k3n"));
    assert_eq!(
        Secrets::at(&path)
            .ok()
            .and_then(|one| one.fingerprint(&name)),
        Some(keyed(&path, "t0k3n"))
    );
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let read: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let at = |pointer: &str| read.pointer(pointer).cloned();
    assert_eq!(at("/schema"), Some(serde_json::json!(1)));
    assert_eq!(
        at("/written_by"),
        Some(serde_json::json!(crate::config::store::RUNNING))
    );
    assert_eq!(
        at("/held/komga:token/from"),
        Some(serde_json::json!("recipe"))
    );
    assert_eq!(at("/held/komga:token/at"), Some(serde_json::json!(7)));
}

#[test]
fn what_is_journalled_is_read_before_the_store_is_written() {
    let scratch = Scratch::new("secrets-journal-first");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "first").is_ok());
    let told = std::cell::RefCell::new(Vec::new());
    let refused = kept(
        &path,
        &[keeping("komga:token", "second")],
        &Chance::cycling(),
        &|replaced| {
            told.borrow_mut().extend(
                replaced
                    .iter()
                    .map(|one| (one.name.clone(), one.previous.clone(), one.current.clone())),
            );
            Err(Failure::Nowhere)
        },
    );
    assert!(refused.is_err());
    let held = Secrets::at(&path).unwrap_or_default();
    assert_eq!(
        told.into_inner(),
        [(
            "komga:token".to_owned(),
            held.keyed("first"),
            held.keyed("second")
        )]
    );
    assert!(held
        .keyed("first")
        .is_some_and(|keyed| keyed != fingerprint("first")));
    assert_eq!(value(&path, "komga:token").as_deref(), Some("first"));
}

#[test]
fn a_secret_spanning_lines_is_refused_before_anything_is_written() {
    let scratch = Scratch::new("secrets-spanning");
    let path = secrets_in(&scratch);
    let refused = kept_now(&path, "komga:token", "one\ntwo");
    assert!(
        matches!(refused, Err(Failure::SpansLines { .. })),
        "{refused:?}"
    );
    assert!(!path.exists());
}

#[test]
fn a_store_a_newer_build_wrote_is_refused_and_left_as_it_was() {
    let scratch = Scratch::new("secrets-newer");
    let path = secrets_in(&scratch);
    let newer = r#"{"schema":1,"written_by":"99.0.0","held":{}}"#;
    assert!(std::fs::create_dir_all(scratch.path().join("config")).is_ok());
    assert!(std::fs::write(&path, newer).is_ok());
    let refused = kept_now(&path, "komga:token", "t");
    assert!(
        matches!(refused, Err(Failure::TooNew { .. })),
        "{refused:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).ok().as_deref(), Some(newer));
    let later = r#"{"schema":2,"written_by":"","held":{}}"#;
    assert!(std::fs::write(&path, later).is_ok());
    assert!(matches!(
        kept_now(&path, "komga:token", "t"),
        Err(Failure::TooNew { .. })
    ));
}

#[test]
fn a_write_stopped_part_way_leaves_the_store_whole() {
    let scratch = Scratch::new("secrets-interrupted");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "first").is_ok());
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        std::fs::create_dir_all(path.with_file_name("secrets.json.writing").join("held")).is_ok()
    );
    assert!(kept_now(&path, "komga:token", "second").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), before);
}

#[test]
fn a_change_waits_for_the_hold_another_process_has() {
    let scratch = Scratch::new("secrets-held");
    let path = secrets_in(&scratch);
    let hold = crate::config::held::Hold::taken(&path);
    assert!(hold.is_ok(), "{hold:?}");
    let again = path.clone();
    let waiting = std::thread::spawn(move || kept_now(&again, "komga:token", "t").is_ok());
    std::thread::sleep(Duration::from_millis(100));
    assert!(!waiting.is_finished());
    assert_eq!(value(&path, "komga:token"), None);
    drop(hold);
    assert!(waiting.join().unwrap_or_default());
    assert_eq!(value(&path, "komga:token").as_deref(), Some("t"));
}

#[cfg(unix)]
#[test]
fn the_store_is_its_owners_alone_in_a_directory_that_is_too() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new("secrets-owned");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "t").is_ok());
    let mode = |at: &Path| {
        std::fs::metadata(at)
            .map(|meta| meta.permissions().mode() & 0o777)
            .ok()
    };
    assert_eq!(mode(&path), Some(0o600));
    assert_eq!(mode(&scratch.path().join("config")), Some(0o700));
}

#[test]
fn nothing_printed_of_the_store_carries_a_value() {
    let scratch = Scratch::new("secrets-printed");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "t0k3n").is_ok());
    assert!(kept_now(&path, "komga:token", "l4ter").is_ok());
    let printed = format!("{:?}", Secrets::at(&path).unwrap_or_default());
    assert!(printed.contains("komga:token"), "{printed}");
    assert!(
        !printed.contains("t0k3n") && !printed.contains("l4ter"),
        "{printed}"
    );
}

#[test]
fn undoing_a_replacement_puts_the_one_earlier_value_back_while_it_matches() {
    let scratch = Scratch::new("secrets-undo");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "first").is_ok());
    assert!(kept_now(&path, "komga:token", "second").is_ok());
    let (first, second) = (keyed(&path, "first"), keyed(&path, "second"));
    let undone = super::secrets::reheld(&path, "komga:token", Some(&first), Some(&second), &|_| {
        Ok(())
    });
    assert!(matches!(undone, Ok(Undone::Done)));
    assert_eq!(value(&path, "komga:token").as_deref(), Some("first"));
    let again = super::secrets::reheld(&path, "komga:token", Some(&first), Some(&second), &|_| {
        Ok(())
    });
    assert!(matches!(again, Ok(Undone::Done)));
    assert_eq!(value(&path, "komga:token").as_deref(), Some("first"));
}

#[test]
fn a_replacement_with_no_earlier_copy_is_left_and_said_to_be_entered_again() {
    let scratch = Scratch::new("secrets-unkept");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "second").is_ok());
    let undone = super::secrets::reheld(
        &path,
        "komga:token",
        Some(&keyed(&path, "first")),
        Some(&keyed(&path, "second")),
        &|_| Ok(()),
    );
    assert!(matches!(undone, Ok(Undone::Unkept)));
    assert_eq!(value(&path, "komga:token").as_deref(), Some("second"));
    assert!(kept_now(&path, "komga:token", "third").is_ok());
    let other = super::secrets::reheld(
        &path,
        "komga:token",
        Some(&keyed(&path, "first")),
        Some(&keyed(&path, "third")),
        &|_| Ok(()),
    );
    assert!(
        matches!(other, Ok(Undone::Unkept)),
        "the earlier copy is another value"
    );
    assert_eq!(value(&path, "komga:token").as_deref(), Some("third"));
}

#[test]
fn a_secret_set_since_is_left_as_it_is() {
    let scratch = Scratch::new("secrets-theirs");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "theirs").is_ok());
    let undone = super::secrets::reheld(
        &path,
        "komga:token",
        None,
        Some(&keyed(&path, "ours")),
        &|_| Ok(()),
    );
    assert!(matches!(undone, Ok(Undone::TheirsNow)));
    assert_eq!(value(&path, "komga:token").as_deref(), Some("theirs"));
}

#[test]
fn undoing_a_filed_secret_puts_its_line_back_and_forgets_it() {
    let scratch = Scratch::new("secrets-unfiled");
    let config = scratch.path().join("config");
    let env = config.join(".env");
    assert!(kept_now(&config.join("secrets.json"), "komga:token", "t0k3n").is_ok());
    let undone = super::undoing::reheld(
        &env,
        "komga:token",
        None,
        Some(&keyed(&config.join("secrets.json"), "t0k3n")),
        Some("PLUGIN_KOMGA_TOKEN_SECRET"),
    );
    assert!(matches!(undone, Ok(Undone::Done)));
    assert_eq!(value(&config.join("secrets.json"), "komga:token"), None);
    assert_eq!(
        crate::config::store::read(&env)
            .ok()
            .and_then(|file| file.get("PLUGIN_KOMGA_TOKEN_SECRET").map(str::to_owned))
            .as_deref(),
        Some("t0k3n")
    );
}

#[test]
fn a_setting_goes_back_only_while_it_holds_what_was_recorded() {
    let scratch = Scratch::new("settings-undo");
    let config = scratch.path().join("config");
    let path = config.join("settings.json");
    let recorded = super::kept::changed(&path, &mut |settings: &mut serde_json::Value| {
        *settings = serde_json::json!({"values": {"komga:mode": "fast"}});
        Ok(())
    });
    assert!(recorded.is_ok(), "{recorded:?}");
    let read = |name: &str| {
        Settings::at(&path)
            .ok()
            .and_then(|settings| settings.value(name).map(str::to_owned))
    };
    assert_eq!(read("komga:mode").as_deref(), Some("fast"));
    let env = config.join(".env");
    let theirs = super::undoing::resettled(&env, "komga:mode", None, Some("slow"), None);
    assert!(matches!(theirs, Ok(Undone::TheirsNow)));
    let undone = super::undoing::resettled(&env, "komga:mode", Some("slow"), Some("fast"), None);
    assert!(matches!(undone, Ok(Undone::Done)));
    assert_eq!(read("komga:mode").as_deref(), Some("slow"));
    let cleared = super::undoing::resettled(&env, "komga:mode", None, Some("slow"), None);
    assert!(matches!(cleared, Ok(Undone::Done)));
    assert_eq!(read("komga:mode"), None);
}

#[test]
fn forgetting_a_namespace_takes_only_what_is_in_it() {
    let scratch = Scratch::new("secrets-forgotten");
    let path = secrets_in(&scratch);
    assert!(kept_now(&path, "komga:token", "a").is_ok());
    assert!(kept_now(&path, "komgatwo:token", "b").is_ok());
    assert!(super::secrets::forgotten(&path, "komga").is_ok());
    assert_eq!(value(&path, "komga:token"), None);
    assert_eq!(value(&path, "komgatwo:token").as_deref(), Some("b"));
}

#[test]
fn where_a_value_came_from_is_written_as_a_word() {
    assert_eq!(Source::Operator.as_str(), "operator");
    assert_eq!(Source::Recipe.as_str(), "recipe");
}

#[test]
fn a_store_that_does_not_read_is_refused_and_left_as_it_was() {
    let scratch = Scratch::new("secrets-unread");
    let path = secrets_in(&scratch);
    assert!(std::fs::create_dir_all(scratch.path().join("config")).is_ok());
    for text in [
        "{ not json",
        r#"{"schema":1,"written_by":"","held":{"a:b":{"value":987654321}}}"#,
    ] {
        assert!(std::fs::write(&path, text).is_ok());
        let refused = kept_now(&path, "komga:token", "t");
        assert!(
            matches!(&refused, Err(Failure::Unreadable { reason, .. })
                if reason.contains("line 1") && !reason.contains("987654321")),
            "{refused:?}"
        );
        assert!(matches!(
            Secrets::at(&path),
            Err(Failure::Unreadable { .. })
        ));
        assert_eq!(std::fs::read_to_string(&path).ok().as_deref(), Some(text));
    }
}

#[test]
fn a_store_that_is_a_directory_is_refused() {
    let scratch = Scratch::new("secrets-directory");
    let path = secrets_in(&scratch);
    assert!(std::fs::create_dir_all(&path).is_ok());
    assert!(matches!(
        Secrets::at(&path),
        Err(Failure::Unreadable { .. })
    ));
}

#[test]
fn a_hold_that_cannot_be_taken_writes_nothing() {
    let scratch = Scratch::new("secrets-unheld");
    let path = secrets_in(&scratch);
    assert!(std::fs::create_dir_all(crate::config::held::beside(&path)).is_ok());
    let refused = kept_now(&path, "komga:token", "t");
    assert!(
        matches!(refused, Err(Failure::NotWritten { .. })),
        "{refused:?}"
    );
    assert!(!path.exists());
}

#[test]
fn a_setting_already_back_is_done_and_left_untouched() {
    let scratch = Scratch::new("settings-already");
    let config = scratch.path().join("config");
    assert!(std::fs::create_dir_all(&config).is_ok());
    let text = r#"{"schema":1,"written_by":"","values":{"komga:mode":"slow"}}"#;
    assert!(std::fs::write(config.join("settings.json"), text).is_ok());
    let undone = super::undoing::resettled(
        &config.join(".env"),
        "komga:mode",
        Some("slow"),
        Some("fast"),
        None,
    );
    assert!(matches!(undone, Ok(Undone::Done)));
    assert_eq!(
        std::fs::read_to_string(config.join("settings.json"))
            .ok()
            .as_deref(),
        Some(text)
    );
}

#[test]
fn undoing_against_a_store_a_newer_build_wrote_is_refused() {
    let scratch = Scratch::new("undo-newer");
    let config = scratch.path().join("config");
    assert!(std::fs::create_dir_all(&config).is_ok());
    let newer = r#"{"schema":1,"written_by":"99.0.0"}"#;
    assert!(std::fs::write(config.join("settings.json"), newer).is_ok());
    assert!(std::fs::write(config.join("secrets.json"), newer).is_ok());
    let env = config.join(".env");
    assert!(matches!(
        super::undoing::resettled(&env, "komga:mode", None, Some("fast"), None),
        Err(Failure::TooNew { .. })
    ));
    assert!(matches!(
        super::undoing::reheld(&env, "komga:token", None, Some("~1"), None),
        Err(Failure::TooNew { .. })
    ));
}

#[test]
fn forgetting_with_nowhere_to_look_touches_nothing() {
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings::default())
        .build();
    let plugin = crate::test_support::an_installed(
        "comics",
        vec![crate::test_support::a_placed("komga", &[], None)],
    );
    super::forgotten(&ctx, &plugin);
    assert_eq!(super::secrets_at(&ctx), None);
}

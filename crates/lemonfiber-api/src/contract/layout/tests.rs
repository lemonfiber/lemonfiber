use super::{differing, read, rendered, replace, Files};

/// A directory of its own under the system's temporary one, empty.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-layout-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn files(pairs: &[(&str, &str)]) -> Files {
    pairs
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn what_is_written_is_what_is_read_back() {
    let dir = scratch("round");
    let written = files(&[
        ("index.json", "{}\n"),
        ("defs/Code.json", "{\"type\": \"string\"}\n"),
    ]);

    assert!(replace(&dir, &written).is_ok());
    assert_eq!(read(&dir).ok(), Some(written));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_the_new_set_no_longer_holds_is_gone() {
    let dir = scratch("gone");
    assert!(replace(&dir, &files(&[("a.json", "1\n"), ("kinds/b.json", "2\n")])).is_ok());
    assert!(replace(&dir, &files(&[("a.json", "3\n")])).is_ok());

    assert_eq!(read(&dir).ok(), Some(files(&[("a.json", "3\n")])));
    assert!(!dir.with_extension("next").exists());
    assert!(!dir.with_extension("old").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_a_run_that_failed_left_beside_it_is_cleared_first() {
    let dir = scratch("stale");
    for left in [dir.with_extension("next"), dir.with_extension("old")] {
        assert!(std::fs::create_dir_all(left.join("kinds")).is_ok());
        assert!(std::fs::write(left.join("kinds/stray.json"), "{}\n").is_ok());
    }
    let written = files(&[("index.json", "{}\n")]);

    assert!(replace(&dir, &written).is_ok());
    assert_eq!(read(&dir).ok(), Some(written));
    assert!(!dir.with_extension("next").exists());
    assert!(!dir.with_extension("old").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn agreeing_sets_say_nothing() {
    let both = files(&[("a.json", "1\n")]);

    assert!(differing(&both, &both).is_empty());
}

#[test]
fn each_way_two_sets_part_is_named_by_its_file() {
    let stored = files(&[
        ("same.json", "1\n"),
        ("moved.json", "1\n"),
        ("left.json", "1\n"),
    ]);
    let fresh = files(&[
        ("same.json", "1\n"),
        ("moved.json", "2\n"),
        ("new.json", "1\n"),
    ]);

    assert_eq!(
        differing(&stored, &fresh),
        vec![
            "moved.json is committed and differs from what the types generate — they part \
             company 0 characters in: the file has \"1\\n\" where the types make \"2\\n\""
                .to_owned(),
            "new.json is generated and not committed".to_owned(),
            "left.json is committed and nothing generates it".to_owned(),
        ]
    );
}

#[test]
fn a_file_is_rendered_indented_with_one_trailing_newline() {
    let value = serde_json::json!({ "b": 1, "a": [true] });

    assert_eq!(
        rendered(&value),
        "{\n  \"a\": [\n    true\n  ],\n  \"b\": 1\n}\n"
    );
}

#[test]
fn a_file_that_moved_says_where_it_went_wrong() {
    let stored = files(&[("a.json", "the same up to here")]);
    let fresh = files(&[("a.json", "the same up to there")]);
    let said = differing(&stored, &fresh).join("");

    assert!(said.contains("15 characters in"), "{said}");
    assert!(said.contains("\"here\""), "{said}");
    assert!(said.contains("\"there\""), "{said}");
}

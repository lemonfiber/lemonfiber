use std::path::Path;

use lemonfiber_fixtures::scratch::Scratch;

use super::{digest, keep, swept, Starting, Throwaway, MOST_BYTES, MOST_FILES};

const SERVICE: &str = "recyclarr";

fn put(root: &Path, relative: &str, text: &[u8]) {
    let at = root.join(relative);
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(at, text);
}

fn of(root: &Path) -> Option<Starting<'_>> {
    [Starting {
        root,
        service: SERVICE,
    }]
    .into_iter()
    .next()
}

#[test]
fn a_copy_holds_every_starting_file_and_goes_when_dropped() {
    let source = Scratch::new("throwaway-copied");
    put(&source, "config/recyclarr/recyclarr.yml", b"sonarr: {}\n");
    put(&source, "config/recyclarr/includes/a.yml", b"a\n");
    put(
        &source,
        "config/other/elsewhere.yml",
        b"not this service's\n",
    );
    let copy = Throwaway::of(of(&source));
    let made = copy.as_ref().ok().map(|copy| copy.path().to_path_buf());
    let read = |relative: &str| {
        made.as_ref()
            .and_then(|at| std::fs::read(at.join(relative)).ok())
    };
    assert_eq!(read("recyclarr.yml"), Some(b"sonarr: {}\n".to_vec()));
    assert_eq!(read("includes/a.yml"), Some(b"a\n".to_vec()));
    assert_eq!(read("elsewhere.yml"), None);
    drop(copy);
    assert!(made.is_some_and(|at| !at.exists()));
}

#[cfg(unix)]
#[test]
fn a_copy_is_open_to_whoever_the_run_is_and_its_parent_to_nobody_else() {
    use std::os::unix::fs::PermissionsExt as _;
    let source = Scratch::new("throwaway-modes");
    put(&source, "config/recyclarr/recyclarr.yml", b"x");
    let copy = Throwaway::of(of(&source));
    let mode = |at: &Path| {
        std::fs::metadata(at)
            .map(|meta| meta.permissions().mode() & 0o777)
            .unwrap_or_default()
    };
    let modes = copy.as_ref().ok().map(|copy| {
        (
            mode(copy.path()),
            mode(&copy.path().join("recyclarr.yml")),
            copy.path().parent().map(mode),
        )
    });
    assert_eq!(modes, Some((0o777, 0o666, Some(0o700))));
}

#[test]
fn a_service_shipping_nothing_is_run_with_an_empty_directory() {
    let source = Scratch::new("throwaway-none");
    let copy = Throwaway::of(of(&source));
    let empty = copy
        .as_ref()
        .ok()
        .and_then(|copy| std::fs::read_dir(copy.path()).ok())
        .map(Iterator::count);
    assert_eq!(empty, Some(0));
    assert_eq!(digest(of(&source)), digest(None));
}

#[cfg(unix)]
#[test]
fn a_link_anywhere_from_the_source_down_is_refused_rather_than_followed() {
    use std::os::unix::fs::symlink;
    let elsewhere = Scratch::new("throwaway-elsewhere");
    put(&elsewhere, "recyclarr/secret.txt", b"host file\n");
    put(&elsewhere, "secret.txt", b"host file\n");

    let linked_start = Scratch::new("throwaway-linked-start");
    let _ = std::fs::create_dir_all(linked_start.join("config"));
    let _ = symlink(
        elsewhere.join("recyclarr"),
        linked_start.join("config/recyclarr"),
    );

    let linked_ancestor = Scratch::new("throwaway-linked-ancestor");
    let _ = symlink(elsewhere.path(), linked_ancestor.join("config"));

    let linked_file = Scratch::new("throwaway-linked-file");
    put(&linked_file, "config/recyclarr/recyclarr.yml", b"x");
    let _ = symlink(
        elsewhere.join("secret.txt"),
        linked_file.join("config/recyclarr/secret.txt"),
    );

    let linked_below = Scratch::new("throwaway-linked-below");
    put(&linked_below, "config/recyclarr/recyclarr.yml", b"x");
    let _ = symlink(
        elsewhere.path(),
        linked_below.join("config/recyclarr/below"),
    );

    for source in [&linked_start, &linked_ancestor, &linked_file, &linked_below] {
        let refused = Throwaway::of(of(source));
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.contains("is a link or a device")),
            "{}: {refused:?}",
            source.display()
        );
        assert!(digest(of(source)).is_err(), "{}", source.display());
    }
}

#[cfg(unix)]
#[test]
fn a_file_is_read_through_no_link_at_its_name_and_only_from_beneath_its_directory() {
    let source = Scratch::new("throwaway-read");
    put(&source, "config/recyclarr/recyclarr.yml", b"x");
    let inside = source.join("config/recyclarr");
    let _ = std::os::unix::fs::symlink(inside.join("recyclarr.yml"), inside.join("named.yml"));
    let inside = std::fs::canonicalize(&inside).unwrap_or_default();
    assert_eq!(
        super::read(&inside.join("recyclarr.yml"), &inside, "recyclarr.yml", 8),
        Ok(b"x".to_vec())
    );
    assert!(super::read(&inside.join("named.yml"), &inside, "named.yml", 8).is_err());
    let elsewhere = Scratch::new("throwaway-read-elsewhere");
    let elsewhere = std::fs::canonicalize(elsewhere.path()).unwrap_or_default();
    assert!(super::read(
        &inside.join("recyclarr.yml"),
        &elsewhere,
        "recyclarr.yml",
        8
    )
    .is_err());
}

#[cfg(unix)]
#[test]
fn a_step_that_cannot_be_looked_at_is_refused_rather_than_taken_as_absent() {
    use std::os::unix::fs::PermissionsExt as _;
    let source = Scratch::new("throwaway-unlooked");
    put(&source, "config/recyclarr/recyclarr.yml", b"x");
    let config = source.join("config");
    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000));
    let looked = std::fs::symlink_metadata(config.join(SERVICE)).is_ok();
    let said = digest(of(&source));
    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o755));
    if !looked {
        assert!(
            said.as_ref()
                .is_err_and(|why| why.contains("could not be read")),
            "{said:?}"
        );
    }
}

#[test]
fn starting_files_past_the_bounds_are_refused() {
    let many = Scratch::new("throwaway-many");
    for index in 0..=MOST_FILES {
        put(&many, &format!("config/recyclarr/{index}.yml"), b"x");
    }
    let refused = Throwaway::of(of(&many));
    assert!(refused.is_err_and(|why| why.contains("pass 64 files")));

    let exactly = Scratch::new("throwaway-exactly");
    for index in 0..MOST_FILES {
        put(&exactly, &format!("config/recyclarr/{index}.yml"), b"x");
    }
    assert!(Throwaway::of(of(&exactly)).is_ok());

    let heavy = Scratch::new("throwaway-heavy");
    let half = usize::try_from(MOST_BYTES / 2).unwrap_or_default();
    put(&heavy, "config/recyclarr/a.bin", &vec![0; half]);
    put(&heavy, "config/recyclarr/b.bin", &vec![0; half + 1]);
    assert!(Throwaway::of(of(&heavy)).is_err_and(|why| why.contains("bytes")));

    let full = Scratch::new("throwaway-full");
    put(&full, "config/recyclarr/a.bin", &vec![0; half]);
    put(&full, "config/recyclarr/b.bin", &vec![0; half]);
    assert!(Throwaway::of(of(&full)).is_ok());
}

#[test]
fn the_digest_moves_with_any_starting_file_and_with_nothing_else() {
    let source = Scratch::new("throwaway-digest");
    put(&source, "config/recyclarr/recyclarr.yml", b"one\n");
    put(&source, "config/other/x.yml", b"x\n");
    let before = digest(of(&source));
    put(&source, "config/other/x.yml", b"y\n");
    assert_eq!(digest(of(&source)), before);
    put(&source, "config/recyclarr/recyclarr.yml", b"two\n");
    assert_ne!(digest(of(&source)), before);
    assert!(before.is_ok_and(|said| said.starts_with("sha256:") && said.len() == 71));
}

#[test]
fn a_copy_an_earlier_run_left_is_swept_and_nothing_else_is() {
    let temp = Scratch::new("throwaway-swept");
    for name in [
        "lemonfiber-proof-1-0",
        "lemonfiber-proof-2-0",
        "somebody-elses",
    ] {
        let _ = std::fs::create_dir_all(temp.join(name).join("configuration"));
    }
    let _ = std::fs::write(temp.join("lemonfiber-proof-file"), "x");
    let later = std::time::SystemTime::now()
        + crate::ports::program::LEFT_BEHIND
        + std::time::Duration::from_secs(1);
    swept(&temp, std::time::SystemTime::now());
    assert!(
        temp.join("lemonfiber-proof-1-0").exists(),
        "a copy a run may still be using stays"
    );
    swept(&temp, later);
    assert!(!temp.join("lemonfiber-proof-1-0").exists());
    assert!(!temp.join("lemonfiber-proof-2-0").exists());
    assert!(temp.join("somebody-elses").exists());
    assert!(temp.join("lemonfiber-proof-file").exists());
    swept(&temp.join("absent"), later);
}

#[test]
fn a_copy_that_cannot_be_made_where_it_goes_is_refused_naming_where() {
    let source = Scratch::new("throwaway-unmade");
    put(&source, "config/recyclarr/recyclarr.yml", b"x");
    let temp = Scratch::new("throwaway-unmade-temp");
    let _ = std::fs::write(temp.join("file"), "a file where a directory goes");
    let refused = Throwaway::beneath(&temp.join("file"), of(&source));
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("could not be made at")),
        "{refused:?}"
    );
    assert!(keep(
        Starting {
            root: &source,
            service: SERVICE,
        },
        &temp.join("file")
    )
    .is_err_and(|why| why.contains("could not be made at")));
}

#[cfg(unix)]
#[test]
fn starting_files_that_are_a_link_are_never_kept() {
    let elsewhere = Scratch::new("throwaway-keep-elsewhere");
    let source = Scratch::new("throwaway-keep-linked");
    let _ = std::fs::create_dir_all(source.join("config"));
    let _ = std::os::unix::fs::symlink(elsewhere.path(), source.join("config/recyclarr"));
    let into = Scratch::new("throwaway-keep-into");
    assert!(keep(
        Starting {
            root: &source,
            service: SERVICE,
        },
        &into
    )
    .is_err_and(|why| why.contains("is a link")));
}

#[cfg(unix)]
#[test]
fn a_directory_or_file_that_cannot_be_read_is_refused_rather_than_skipped() {
    use std::os::unix::fs::PermissionsExt as _;
    let closed = |at: &Path, mode: u32| {
        let _ = std::fs::set_permissions(at, std::fs::Permissions::from_mode(mode));
    };
    let unlistable = Scratch::new("throwaway-unlistable");
    put(&unlistable, "config/recyclarr/inner/a.yml", b"x");
    let inner = unlistable.join("config/recyclarr/inner");
    closed(&inner, 0o000);
    let listable = std::fs::read_dir(&inner).is_ok();
    let said = digest(of(&unlistable));
    closed(&inner, 0o755);
    if !listable {
        assert!(said.is_err_and(|why| why.contains("could not be read")));
    }

    let unreadable = Scratch::new("throwaway-unreadable");
    put(&unreadable, "config/recyclarr/a.yml", b"x");
    let file = unreadable.join("config/recyclarr/a.yml");
    closed(&file, 0o000);
    let readable = std::fs::read(&file).is_ok();
    let said = digest(of(&unreadable));
    closed(&file, 0o644);
    if !readable {
        assert!(said.is_err_and(|why| why.contains("could not be read")));
    }
}

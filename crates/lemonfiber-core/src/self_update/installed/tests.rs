use super::{Installed, Signs, EVERY_WAY};
use std::path::{Path, PathBuf};

/// The signs a machine gives for a binary at this path and nothing else.
fn at(path: &str) -> Signs<'_> {
    Signs {
        at: Some(Path::new(path)),
        ..Signs::default()
    }
}

#[test]
fn a_machine_that_will_not_say_where_the_binary_is_is_not_guessed_at() {
    assert_eq!(Installed::read(&Signs::default()), Installed::Untellable);
    assert_eq!(Installed::Untellable.owner(), None);
    assert!(!Installed::Untellable.defers());
}

#[test]
fn a_binary_in_a_cellar_belongs_to_homebrew_on_either_platform() {
    for path in [
        "/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
        "/usr/local/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
        "/home/linuxbrew/.linuxbrew/bin/lemonfiber",
    ] {
        assert_eq!(Installed::read(&at(path)), Installed::Homebrew, "{path}");
    }
}

#[test]
fn the_windows_managers_are_told_apart_by_the_tree_they_install_into() {
    assert_eq!(
        Installed::read(&at(
            r"C:\Users\sam\scoop\apps\lemonfiber\current\lemonfiber.exe"
        )),
        Installed::Scoop
    );
    for path in [
        r"C:\Users\sam\AppData\Local\Microsoft\WinGet\Packages\lemonfiber\lemonfiber.exe",
        r"C:\Program Files\WindowsApps\lemonfiber\lemonfiber.exe",
    ] {
        assert_eq!(Installed::read(&at(path)), Installed::Winget, "{path}");
    }
}

/// The one case a path cannot answer: the shell installer's default location is
/// cargo's own `bin`, so a binary there arrived either way and only a receipt
/// says which.
#[test]
fn the_two_that_share_a_directory_are_told_apart_by_their_receipts() {
    let path = PathBuf::from("/home/sam/.cargo/bin/lemonfiber");
    let bare = Signs {
        at: Some(&path),
        ..Signs::default()
    };
    assert_eq!(Installed::read(&bare), Installed::Elsewhere);
    assert_eq!(
        Installed::read(&Signs {
            recorded_by_cargo: true,
            ..bare
        }),
        Installed::Cargo
    );
    assert_eq!(
        Installed::read(&Signs {
            receipt: true,
            ..bare
        }),
        Installed::Installer
    );
}

/// Both receipts present is a machine where one tool wrote over the other's
/// work, and the installer is the one that leaves a receipt every time.
#[test]
fn a_receipt_from_each_is_read_as_the_one_that_writes_one_every_time() {
    let path = PathBuf::from("/home/sam/.cargo/bin/lemonfiber");
    assert_eq!(
        Installed::read(&Signs {
            at: Some(&path),
            receipt: true,
            recorded_by_cargo: true,
        }),
        Installed::Installer
    );
}

#[test]
fn a_package_managers_tree_is_read_before_any_receipt() {
    let path = PathBuf::from("/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber");
    assert_eq!(
        Installed::read(&Signs {
            at: Some(&path),
            receipt: true,
            recorded_by_cargo: true,
        }),
        Installed::Homebrew
    );
}

#[test]
fn the_systems_own_bin_is_a_distribution_and_the_one_beside_it_is_not() {
    assert_eq!(
        Installed::read(&at("/usr/bin/lemonfiber")),
        Installed::Distribution
    );
    assert_eq!(
        Installed::read(&at("/bin/lemonfiber")),
        Installed::Distribution
    );
    assert_eq!(
        Installed::read(&at("/usr/local/bin/lemonfiber")),
        Installed::Elsewhere
    );
}

#[test]
fn a_binary_nobody_claims_is_the_operators_own() {
    for path in [
        "/home/sam/Downloads/lemonfiber",
        "/Users/sam/code/lemonfiber/target/release/lemonfiber",
        "lemonfiber",
    ] {
        assert_eq!(Installed::read(&at(path)), Installed::Elsewhere, "{path}");
    }
}

#[test]
fn the_ones_a_tool_owns_are_the_ones_that_defer_and_they_name_the_tool() {
    let deferring: Vec<Installed> = EVERY_WAY
        .iter()
        .copied()
        .filter(|way| way.defers())
        .collect();
    assert_eq!(
        deferring,
        vec![
            Installed::Homebrew,
            Installed::Scoop,
            Installed::Winget,
            Installed::Cargo,
            Installed::Distribution,
        ]
    );
    for way in EVERY_WAY {
        assert_eq!(way.defers(), way.owner().is_some(), "{way:?}");
    }
}

/// The ones that keep an index are exactly the ones a version is never named to.
/// A tool told a version its index does not carry answers with nothing, which
/// leaves an operator holding a report and no way forward.
#[test]
fn the_ones_that_keep_an_index_are_the_ones_that_find_the_newest_themselves() {
    let finding: Vec<Installed> = EVERY_WAY
        .iter()
        .copied()
        .filter(|way| way.resolves_newest())
        .collect();
    assert_eq!(
        finding,
        vec![
            Installed::Homebrew,
            Installed::Scoop,
            Installed::Winget,
            Installed::Distribution,
        ]
    );
}

#[test]
fn every_way_is_named_once_and_by_something_a_reader_can_type() {
    let named: Vec<&str> = EVERY_WAY.iter().map(|way| way.as_str()).collect();
    assert_eq!(
        named,
        vec![
            "homebrew",
            "scoop",
            "winget",
            "cargo",
            "distribution",
            "installer",
            "elsewhere",
            "untellable"
        ]
    );
}

#[test]
fn a_tree_is_recognised_however_its_case_was_written() {
    assert_eq!(
        Installed::read(&at(r"C:\Users\sam\SCOOP\apps\lemonfiber\lemonfiber.exe")),
        Installed::Scoop
    );
}

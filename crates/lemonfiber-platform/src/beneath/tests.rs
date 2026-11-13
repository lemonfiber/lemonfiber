use std::ffi::OsStr;
use std::path::Path;

use super::{
    bytes, created, lies, made, make, moved, removed, replaced, staging, steps, unpacked_directory,
    unpacked_file, Kept, Lies, Refusal,
};
use crate::fs::{OWNER_DIRECTORY, OWNER_FILE, SHARED_DIRECTORY, SHARED_FILE};
use crate::scratch::Scratch;

#[test]
fn a_path_beneath_is_its_names_and_one_stepping_out_is_refused() {
    let within = Path::new("/stack");

    assert_eq!(
        steps(Path::new("/stack/config/service"), within),
        Ok(vec![OsStr::new("config"), OsStr::new("service")])
    );
    assert_eq!(steps(within, within), Ok(Vec::new()));
    for outside in ["/elsewhere/config", "/stack/config/../../etc"] {
        assert!(
            matches!(steps(Path::new(outside), within), Err(Refusal::Said(why)) if why.contains("does not lie beneath")),
            "{outside}"
        );
    }
}

#[test]
fn what_is_kept_names_its_modes_and_whose_the_way_must_be() {
    assert_eq!(
        (Kept::OWNER.file, Kept::OWNER.directory, Kept::OWNER.owned),
        (OWNER_FILE, OWNER_DIRECTORY, true)
    );
    assert_eq!(
        (
            Kept::PRIVATE.file,
            Kept::PRIVATE.directory,
            Kept::PRIVATE.owned
        ),
        (OWNER_FILE, OWNER_DIRECTORY, false)
    );
    assert_eq!(
        (
            Kept::SHARED.file,
            Kept::SHARED.directory,
            Kept::SHARED.owned
        ),
        (SHARED_FILE, SHARED_DIRECTORY, false)
    );
}

#[test]
fn a_file_is_written_under_a_hidden_name_beside_itself() {
    assert_eq!(staging(OsStr::new("config.xml")), ".config.xml.writing");
}

#[test]
fn a_directory_made_beneath_lies_there_and_one_not_made_is_absent() {
    let dir = Scratch::new("beneath");
    let within = dir.path();
    let deep = within.join("config").join("service");

    assert_eq!(lies(&deep, within), Lies::Absent);
    assert!(make(&deep, within).is_ok());

    assert!(deep.is_dir());
    assert_eq!(lies(&deep, within), Lies::Plain);
    assert_eq!(lies(Path::new("/elsewhere"), within), Lies::Escaped);
}

#[test]
fn a_file_created_beneath_is_made_with_the_directories_above_it() {
    let dir = Scratch::new("beneath");
    let within = dir.path();
    let at = within.join("config").join("settings.json");

    let file = created(&at, within, OWNER_FILE);

    assert!(file.is_ok(), "{file:?}");
    assert!(at.is_file());
}

#[test]
fn a_file_replaced_reads_back_whole_and_goes_when_taken_away() {
    let dir = Scratch::new("beneath");
    let within = dir.path();
    let at = within.join("config").join("settings.json");

    assert_eq!(bytes(&at, within, 64), Ok(None));
    assert!(replaced(&at, within, b"first", Kept::SHARED).is_ok());
    assert!(replaced(&at, within, b"second", Kept::SHARED).is_ok());

    assert_eq!(bytes(&at, within, 64), Ok(Some(b"second".to_vec())));
    assert!(
        bytes(&at, within, 3).is_err(),
        "it is larger than asked for"
    );
    assert!(removed(&at, within).is_ok());
    assert_eq!(bytes(&at, within, 64), Ok(None));
    assert!(
        removed(&at, within).is_ok(),
        "nothing there is nothing to take"
    );
}

#[test]
fn what_is_moved_lands_once_and_never_over_what_is_there() {
    let dir = Scratch::new("beneath");
    let within = dir.path();
    let (from, to) = (
        within.join("config").join("cache"),
        within.join("mounts").join("cache"),
    );
    assert!(made(&from, within, Kept::SHARED).is_ok());

    assert!(moved(&from, &to, within).is_ok());
    assert!(made(&from, within, Kept::SHARED).is_ok());

    assert!(to.is_dir());
    assert!(moved(&from, &to, within).is_err());
    assert!(
        matches!(moved(within, &to, within), Err(Refusal::Said(why)) if why.contains("itself"))
    );
}

#[test]
fn an_unpacked_file_is_made_new_beneath_and_never_over_one_there() {
    let dir = Scratch::new("beneath");
    let root = dir.path();
    let above = [OsStr::new("config")];

    assert!(unpacked_directory(root, &above, SHARED_DIRECTORY).is_ok());
    assert!(unpacked_file(root, &above, OsStr::new("kept"), SHARED_FILE, b"kept").is_ok());

    assert_eq!(
        std::fs::read(root.join("config").join("kept")).ok(),
        Some(b"kept".to_vec())
    );
    let again = unpacked_file(root, &above, OsStr::new("kept"), SHARED_FILE, b"again");
    assert!(matches!(again, Err(Refusal::Said(why)) if why.contains("was not written")));
}

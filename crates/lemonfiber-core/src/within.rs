//! Where a name somebody supplied lands beneath a directory.
//!
//! One rule, in one place, for every caller that turns text from outside into a
//! path underneath a directory lemonfiber chose. The app serves files by the path a
//! browser asked for, and a restore names one of the archives this machine kept;
//! both are text a request carried, and both would reach the operator's whole
//! filesystem if the text were handed to the platform as a path.
//!
//! Only ordinary names survive. A parent link is refused rather than resolved,
//! because the directory above the one a caller was given is not the one it was
//! given. Naming nothing is the empty path, which is the directory itself — what
//! that means is the caller's to say, since it is the app for one of them and
//! nothing at all for the other.

use std::path::PathBuf;

/// Where `asked` lands beneath a directory, or nothing where it leads outside one.
///
/// Split on the separator a request uses rather than handed to the platform's own
/// path parser: a backslash means one thing on Windows and nothing on Linux, and a
/// rule about what may be reached must not change with the machine.
#[must_use]
pub fn beneath(asked: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in asked.split('/') {
        match segment {
            "" | "." => {}
            ".." => return None,
            name if name.contains('\\') => return None,
            name => path.push(name),
        }
    }
    Some(path)
}

/// The one file `asked` names beneath a directory, or nothing where it names
/// anything else.
///
/// Stricter than [`beneath`] by exactly one rule: the answer is a single file in
/// the directory rather than anywhere under it. What it is for is a caller holding
/// a directory of its own files — the archives this machine kept — where a
/// subdirectory is not somewhere it ever wrote and so not somewhere to read from.
#[must_use]
pub fn one_file(asked: &str) -> Option<PathBuf> {
    let path = beneath(asked)?;
    let mut parts = path.components();
    let only = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(PathBuf::from(only.as_os_str()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{beneath, one_file};

    #[test]
    fn an_ordinary_name_is_the_name_that_was_given() {
        assert_eq!(beneath("app.css"), Some(PathBuf::from("app.css")));
        assert_eq!(
            beneath("assets/app.css"),
            Some(PathBuf::from("assets/app.css"))
        );
    }

    #[test]
    fn a_leading_separator_and_a_bare_dot_name_nothing_of_their_own() {
        assert_eq!(beneath("/./assets/./app.css"), beneath("assets/app.css"));
    }

    #[test]
    fn naming_nothing_is_the_directory_itself() {
        assert_eq!(beneath(""), Some(PathBuf::new()));
        assert_eq!(beneath("/"), Some(PathBuf::new()));
    }

    #[test]
    fn a_parent_link_is_refused_rather_than_resolved() {
        assert_eq!(beneath("../Cargo.toml"), None);
        assert_eq!(beneath("assets/../../Cargo.toml"), None);
    }

    #[test]
    fn a_name_written_with_the_other_separator_is_refused() {
        // A backslash is an ordinary character in a Linux filename and a separator
        // on Windows, so a name carrying one would mean two things.
        assert_eq!(beneath("assets\\..\\Cargo.toml"), None);
    }

    #[test]
    fn one_file_is_a_file_in_the_directory_and_not_under_it() {
        assert_eq!(
            one_file("lemonfiber-full-1.tar.gz"),
            Some(PathBuf::from("lemonfiber-full-1.tar.gz"))
        );
        assert_eq!(one_file("older/lemonfiber-full-1.tar.gz"), None);
        assert_eq!(one_file("../lemonfiber-full-1.tar.gz"), None);
        assert_eq!(one_file(""), None);
    }
}

//! The small JSON records kept beside the configuration.
//!
//! Five of them now — the conditions, the notification appetite, the answered
//! choices, what was last materialised, the adopted baseline — and every one had
//! written out the same three lines: read the file if there is one, parse it if it
//! parses, and fall back to the default. Four copies of a rule is four places for
//! it to drift, and the rule here is one worth stating once.
//!
//! **Reading is best effort and writing is not.** A record that cannot be read
//! leaves the default, which puts a settled question again or forgets how long a
//! fault has stood — tiresome, and always in the safe direction. A record that
//! cannot be *written* is a different thing: silence there would leave the
//! operator believing something was remembered that was not, so it is reported.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::store;
use crate::error::{Diagnose, Problem};

/// What the record holds, or the default where there is none to read.
///
/// Nothing configured means nowhere to keep it, which is not a fault: a machine
/// with no configuration has no history to remember either.
#[must_use]
pub(super) fn kept<T: Default + DeserializeOwned>(path: Option<&Path>) -> T {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the record where the next run will read it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(super) fn keep<T: Serialize>(path: Option<&Path>, value: &T) -> Result<(), Box<Problem>> {
    let path = path.ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    // A value that will not serialise writes as an empty record rather than
    // refusing: the types here are plain data with derived implementations, so it
    // cannot happen, and inventing an error path for it would be a branch no test
    // could ever reach.
    store::write(path, &serde_json::to_string(value).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

#[cfg(test)]
mod tests {
    use super::{keep, kept};
    use std::collections::BTreeSet;

    /// A scratch record path for one test.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-record-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("record.json")
    }

    #[test]
    fn what_was_written_is_what_comes_back() {
        let path = scratch("round-trip");
        let mut written = BTreeSet::new();
        written.insert("one".to_owned());
        assert!(keep(Some(path.as_path()), &written).is_ok());
        assert_eq!(kept::<BTreeSet<String>>(Some(path.as_path())), written);
    }

    #[test]
    fn a_record_that_is_not_there_reads_as_the_default() {
        // A machine with no history is not a fault; it is a machine with no
        // history.
        assert!(kept::<BTreeSet<String>>(Some(scratch("absent").as_path())).is_empty());
    }

    #[test]
    fn a_record_that_will_not_parse_reads_as_the_default_rather_than_a_failure() {
        // The safe direction: a settled question is put again, or a fault's age is
        // forgotten. The alternative is refusing to run over a file nobody needs.
        let path = scratch("corrupt");
        assert!(keep(Some(path.as_path()), &BTreeSet::<String>::new()).is_ok());
        assert!(crate::config::store::write(&path, "not json at all").is_ok());
        assert!(kept::<BTreeSet<String>>(Some(path.as_path())).is_empty());
    }

    #[test]
    fn nowhere_to_keep_it_reads_as_the_default_and_refuses_to_write() {
        // Reading and writing part company here, and deliberately: nothing to read
        // is ordinary, and nowhere to write is something the operator is owed.
        assert!(kept::<BTreeSet<String>>(None).is_empty());
        assert!(keep(None, &BTreeSet::<String>::new()).is_err());
    }

    #[test]
    fn a_record_that_cannot_be_written_is_reported_rather_than_swallowed() {
        // A directory where the file must go. Telling the operator something was
        // remembered when it was not is the failure worth avoiding here.
        let path = scratch("blocked");
        assert!(std::fs::create_dir_all(&path).is_ok());
        assert!(keep(Some(path.as_path()), &BTreeSet::<String>::new()).is_err());
    }
}

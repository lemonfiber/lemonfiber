//! Where the frontend comes from, and finding one file in it.
//!
//! The same shape as [`crate::stack`], for the same reason: the app ships inside
//! the binary so the common install has one thing to fetch and nothing to go
//! stale, and an operator building their own points at a directory instead.
//! Everything above this module stops being able to tell the difference.
//!
//! A path in, bytes out. There is no server here and there must not be one —
//! this crate cannot render and cannot listen, which is what keeps a surface a
//! rendering rather than a capability. What a browser is told a file *is*, and
//! which routes reach this at all, belong to whatever does the serving.
//!
//! Two decisions live here rather than in the surface, because both are about
//! the app rather than about HTTP. A path that climbs out of the directory is
//! refused instead of resolved. And a path naming no file at all is the app
//! itself, since its own router reads the path once the page is loaded.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use include_dir::Dir;

/// The one file every built app has, and the answer to a path that names none.
const INDEX: &str = "index.html";

/// Where the frontend lemonfiber serves is read from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// The app compiled into this binary.
    Embedded(&'static Dir<'static>),
    /// A directory on disk, named by whoever is running lemonfiber.
    External(&'static Path),
}

/// One file of the app: what it is called, and what is in it.
///
/// The path travels with the bytes because it is what says how to read them. A
/// caller handed bytes alone would have to be told their type separately, and
/// two arguments that must agree eventually do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// Where this file sits within the app.
    pub path: PathBuf,
    /// The file itself, borrowed from the binary or read from disk.
    pub bytes: Cow<'static, [u8]>,
}

impl Source {
    /// The file a path asks for, or the app itself where it asks for no file.
    ///
    /// A path with an extension names a file: if it is not here, nothing is, and
    /// answering with the page instead would hand a script that asked for a
    /// stylesheet a document it cannot use. A path without one is a route the
    /// app's own router will read, so the app is the honest answer to it.
    #[must_use]
    pub fn asset(self, asked: &str) -> Option<Asset> {
        let within = within(asked)?;
        if let Some(asset) = self.file(&within) {
            return Some(asset);
        }
        if within.extension().is_some() {
            return None;
        }
        self.file(Path::new(INDEX))
    }

    /// Whether there is an app here at all.
    ///
    /// Asked before anything is served rather than discovered one missing file at
    /// a time, so a build carrying no app can say so once instead of answering
    /// every request with the same absence.
    #[must_use]
    pub fn holds_an_app(self) -> bool {
        self.file(Path::new(INDEX)).is_some()
    }

    /// One file, exactly as named, however this app is stored.
    fn file(self, within: &Path) -> Option<Asset> {
        let bytes = match self {
            Self::Embedded(dir) => Cow::Borrowed(dir.get_file(within)?.contents()),
            // A file that cannot be read is a file that is not here. Whether this
            // is an app at all was settled before anything was served, and a
            // permission fault on one asset is not a different answer to a caller.
            Self::External(root) => Cow::Owned(std::fs::read(root.join(within)).ok()?),
        };
        Some(Asset {
            path: within.to_path_buf(),
            bytes,
        })
    }
}

/// Where a path lands within the app, or nothing where it leads outside it.
///
/// The rule about what a supplied name may reach is [`crate::within`]'s, shared
/// with the other caller that turns request text into a path beneath a directory
/// lemonfiber chose. What is decided here is only what naming nothing means, which
/// is about the app rather than about paths: a path with no file in it is a route
/// the app's own router reads once the page is loaded, so the app is the answer.
fn within(asked: &str) -> Option<PathBuf> {
    let path = crate::within::beneath(asked)?;
    if path.as_os_str().is_empty() {
        return Some(PathBuf::from(INDEX));
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use include_dir::{include_dir, Dir};

    use super::{within, Source};

    /// An app of three files, standing in for one a build would embed.
    ///
    /// The real directory is a submodule that does not exist yet, so what is held
    /// still here is the mechanism: a fixture reaches every branch the shipped
    /// app would, and the day the submodule arrives nothing below this changes.
    static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/frontend");

    /// A directory that is certainly not an app.
    static NOT_AN_APP: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/future-schema");

    /// The same fixture, read from disk instead of from the binary.
    fn on_disk() -> Source {
        Source::External(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/frontend"
        )))
    }

    /// Both readings of the same app, so every rule is proven against each.
    fn both() -> [Source; 2] {
        [Source::Embedded(&EMBEDDED), on_disk()]
    }

    /// What a path answers with, as text — empty where it answers with nothing.
    fn read(source: Source, asked: &str) -> String {
        source.asset(asked).map_or_else(String::new, |asset| {
            String::from_utf8_lossy(&asset.bytes).into_owned()
        })
    }

    #[test]
    fn a_named_file_is_the_file_that_was_named() {
        for source in both() {
            assert!(read(source, "/assets/app.js").contains("the app"));
            assert!(read(source, "/assets/app.css").contains("body"));
        }
    }

    #[test]
    fn the_root_is_the_app() {
        for source in both() {
            assert!(read(source, "/").contains("<!doctype html>"));
            assert!(read(source, "").contains("<!doctype html>"));
        }
    }

    #[test]
    fn a_route_the_app_reads_for_itself_is_the_app() {
        // The browser's own router reads this once the page is loaded, so the
        // page is what has to arrive for it to read anything at all.
        for source in both() {
            assert!(read(source, "/services/sonarr").contains("<!doctype html>"));
        }
    }

    #[test]
    fn a_named_file_that_is_not_here_is_not_the_app_instead() {
        // A script asked for a stylesheet; handing it a document would be a
        // wrong answer where an absent one was owed.
        for source in both() {
            assert_eq!(source.asset("/assets/missing.css"), None);
        }
    }

    #[test]
    fn a_path_climbing_out_of_the_app_is_refused() {
        for source in both() {
            assert_eq!(source.asset("/../../Cargo.toml"), None);
            assert_eq!(source.asset("../Cargo.toml"), None);
            assert_eq!(source.asset("/assets/../../Cargo.toml"), None);
        }
    }

    #[test]
    fn a_path_written_with_the_other_separator_is_refused() {
        for source in both() {
            assert_eq!(source.asset(r"..\Cargo.toml"), None);
        }
    }

    #[test]
    fn a_path_that_only_marks_where_it_is_reaches_the_same_file() {
        for source in both() {
            assert!(read(source, "/./assets/./app.js").contains("the app"));
        }
    }

    #[test]
    fn the_two_readings_of_one_app_agree() {
        for asked in ["/", "/assets/app.js", "/assets/app.css", "/services/sonarr"] {
            assert_eq!(
                read(Source::Embedded(&EMBEDDED), asked),
                read(on_disk(), asked),
                "the same app read two ways is the same app: {asked}"
            );
        }
    }

    #[test]
    fn a_directory_holding_an_index_holds_an_app() {
        for source in both() {
            assert!(source.holds_an_app());
        }
    }

    #[test]
    fn a_directory_holding_no_index_holds_no_app() {
        assert!(!Source::Embedded(&NOT_AN_APP).holds_an_app());
        assert!(!Source::External(Path::new("/lemonfiber/no/such/app")).holds_an_app());
    }

    #[test]
    fn a_build_carrying_no_app_answers_nothing_rather_than_something_else() {
        let empty = Source::Embedded(&NOT_AN_APP);
        assert_eq!(empty.asset("/"), None);
        assert_eq!(empty.asset("/assets/app.js"), None);
    }

    #[test]
    fn a_path_of_nothing_but_separators_is_still_the_app() {
        assert_eq!(within("///").as_deref(), Some(Path::new("index.html")));
    }
}

//! What a capture, a restore and a support bundle tell the operator they did.
//!
//! The three answers about an archive, together because they share the sentence
//! that names what one covers and because all three end by telling somebody what
//! is now on their disk and how careful to be with it. The listing of what has been
//! kept is here for the same reason: it is what a restore is asked with, and it
//! ends by saying how to ask.
//!
//! A restore's listing and what a restore did are one answer with the listing
//! always in it, and only one of the two is worth reading at a time: before, the
//! listing is the whole point and there is nothing else to say; after, repeating it
//! would put the same paragraph on the screen twice in one run.

use std::path::Path;

use lemonfiber_core::app::archives::Listing;
use lemonfiber_core::app::backup::Report as Capture;
use lemonfiber_core::app::restore::{Preview, Restoration};
use lemonfiber_core::app::support::Bundle;
use lemonfiber_core::backup::Scope;
use lemonfiber_core::bytes::humanize;

use super::Lines;

/// Where a backup went, and how private it is.
pub(crate) fn backup(report: &Capture) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Backed up {} to {}",
        scope_name(&report.scope),
        report.path.display()
    ));
    if report.sensitive {
        lines.put(
            "This backup contains credentials — the VPN key, provider passwords and API keys. \
             Keep it as private as the secrets inside it.",
        );
    }
    if !report.pruned.is_empty() {
        lines.put(format!("Pruned {} older backup(s).", report.pruned.len()));
    }
    lines
}

/// Which backups this machine has kept, and how to put one back.
///
/// The names and nothing else, because a name is what a restore is asked for and
/// what an archive holds is the answer to naming one. Newest first, since the
/// archive most often wanted is the one taken last.
pub(crate) fn kept(listing: &Listing) -> Lines {
    let mut lines = Lines::default();
    if listing.archives.is_empty() {
        lines.put("No backups have been taken on this machine yet.");
        lines.spaced("Take one with:  lemonfiber backup");
        return lines;
    }
    lines.put("Backups kept on this machine, newest first:");
    for name in &listing.archives {
        lines.put(format!("  {name}"));
    }
    lines.spaced("Put one back with:  lemonfiber restore <archive>");
    lines
}

/// What a restore would overwrite, or what it put back.
pub(crate) fn restoration(report: &Restoration) -> Lines {
    match &report.done {
        None => would(&report.would),
        Some(done) => {
            let mut lines = Lines::default();
            lines.put(format!(
                "Restored {} from a backup taken by lemonfiber {}.",
                scope_name(&done.scope),
                done.from_version
            ));
            if let Some(relocation) = &done.relocated {
                lines.put(format!(
                    "Re-pointed the data root from {} to {}.",
                    relocation.was, relocation.now
                ));
            }
            lines.extend(next_steps());
            lines
        }
    }
}

/// The archive's own account of itself, read before anything is overwritten.
fn would(preview: &Preview) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "This backup holds {}, taken by lemonfiber {} on {}.",
        scope_name(&preview.manifest.scope),
        preview.manifest.product_version,
        preview.manifest.created_at,
    ));
    for member in &preview.manifest.members {
        lines.put(format!("  - {}", member.label));
    }
    if preview.downgrade {
        lines.put(
            "It is from an older major version; restoring it is allowed but may need a further \
             reconcile.",
        );
    }
    if let Some(relocation) = &preview.relocation {
        lines.put(format!(
            "It was taken against a different data root ({} → {}); re-run with --repoint to \
             restore onto this machine's.",
            relocation.was, relocation.now
        ));
    }
    lines
}

/// What a bundle would hold, or where the one that exists went.
pub(crate) fn bundle(report: &Bundle) -> Lines {
    match &report.path {
        None => described(report),
        Some(path) => written(report, path),
    }
}

/// What a bundle would hold, while there is still nothing to attach.
///
/// The size is stated here rather than after writing, which is the whole reason a bare run
/// writes nothing: an operator decides whether to make this file at the one moment the
/// answer can still change what they do.
fn described(report: &Bundle) -> Lines {
    let contents = &report.contents;
    let mut lines = Lines::default();
    lines.put("A support bundle would hold:");
    for (name, _) in contents.files() {
        lines.put(format!("  {name}"));
    }
    lines.spaced(format!("{} in all.", humanize(report.bytes)));
    // Named rather than passed over, and set apart rather than mixed into the listing: a
    // gap nobody mentions reads as an absence of trouble instead of an absence of
    // information, and one buried among the filenames reads as neither.
    if !contents.missing.is_empty() {
        lines.spaced("Could not be read:");
        for gap in &contents.missing {
            lines.put(format!("  {gap}"));
        }
    }
    if !contents.terms.revealed.is_empty() {
        let (subject, verb) = if contents.terms.revealed.len() == 1 {
            ("it", "is")
        } else {
            ("they", "are")
        };
        lines.spaced(format!(
            "It will hold {} as {subject} {verb}, because you asked, and will say so on its first page.",
            contents.terms.revealed.join(", "),
        ));
    }
    lines.spaced("Nothing has been written. Run `lemonfiber support --write` to produce it.");
    lines
}

/// Where a bundle went, how large it is, and what a reader will find in it.
fn written(report: &Bundle, path: &Path) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Written to {} ({})",
        path.display(),
        humanize(report.bytes)
    ));
    for (name, _) in report.contents.files() {
        lines.put(format!("  {name}"));
    }
    lines
        .spaced("Nothing has left this machine. Read it before you send it, and send it yourself.");
    lines
}

/// How a scope reads in a line of output.
fn scope_name(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "the whole stack".to_owned(),
        Scope::Service { name } => format!("service {name}"),
    }
}

/// What a restore leaves the operator to do, once the files are back in place.
fn next_steps() -> Lines {
    let mut lines = Lines::default();
    lines.put(
        "Now bring the stack up and reconcile its wiring:  lemonfiber up <form> && lemonfiber seed",
    );
    lines.put(
        "Then check the restored credentials still work:  lemonfiber doctor --only credentials",
    );
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use lemonfiber_core::app::archives::Listing;
    use lemonfiber_core::app::backup::Report as Capture;
    use lemonfiber_core::app::restore::{Preview, Report as Restored, Restoration};
    use lemonfiber_core::app::support::Bundle;
    use lemonfiber_core::backup::{Manifest, Member, Relocation, Scope, SCHEMA};
    use lemonfiber_core::bundle::{Contents, Piece, Taken, Terms};

    use super::{backup, bundle, kept, restoration};

    fn manifest() -> Manifest {
        Manifest {
            schema: SCHEMA,
            product_version: "0.7.0".to_owned(),
            created_at: "2026-07-30".to_owned(),
            data_root: "/srv/media".to_owned(),
            scope: Scope::WholeStack,
            sensitive: true,
            members: vec![Member {
                archive_path: "config/sonarr".to_owned(),
                label: "Sonarr's configuration".to_owned(),
            }],
        }
    }

    fn moved() -> Relocation {
        Relocation {
            was: "/srv/media".to_owned(),
            now: "/mnt/library".to_owned(),
        }
    }

    #[test]
    fn a_capture_says_where_it_went_and_how_private_it_is() {
        let said = backup(&Capture {
            path: PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
            scope: Scope::WholeStack,
            sensitive: true,
            pruned: vec!["older.tar.gz".to_owned()],
        })
        .text();
        assert!(said.contains("Backed up the whole stack to"), "{said}");
        assert!(said.contains("credentials"), "{said}");
        assert!(said.contains("Pruned 1 older backup(s)."), "{said}");
    }

    #[test]
    fn a_capture_of_one_service_names_it_and_says_nothing_it_need_not() {
        let said = backup(&Capture {
            path: PathBuf::from("/data/lemonfiber/backups/sonarr.tar.gz"),
            scope: Scope::Service {
                name: "sonarr".to_owned(),
            },
            sensitive: false,
            pruned: Vec::new(),
        })
        .text();
        assert!(said.contains("service sonarr"), "{said}");
        assert!(!said.contains("credentials"), "{said}");
        assert!(!said.contains("Pruned"), "{said}");
    }

    #[test]
    fn a_restore_that_has_touched_nothing_lists_what_it_would_overwrite() {
        let said = restoration(&Restoration {
            would: Preview {
                manifest: manifest(),
                downgrade: true,
                relocation: Some(moved()),
            },
            done: None,
        })
        .text();
        assert!(said.contains("This backup holds the whole stack"), "{said}");
        assert!(said.contains("Sonarr's configuration"), "{said}");
        assert!(said.contains("older major version"), "{said}");
        assert!(said.contains("--repoint"), "{said}");
    }

    #[test]
    fn a_restore_that_put_something_back_says_so_and_what_is_left_to_do() {
        // The listing is not repeated: the operator was shown it before this
        // happened, and a run that printed the same paragraph twice would read as
        // though it had done the work twice.
        let said = restoration(&Restoration {
            would: Preview {
                manifest: manifest(),
                downgrade: false,
                relocation: Some(moved()),
            },
            done: Some(Restored {
                scope: Scope::WholeStack,
                from_version: "0.6.0".to_owned(),
                relocated: Some(moved()),
            }),
        })
        .text();
        assert!(said.contains("Restored the whole stack"), "{said}");
        assert!(said.contains("Re-pointed the data root"), "{said}");
        assert!(said.contains("lemonfiber seed"), "{said}");
        assert!(!said.contains("This backup holds"), "{said}");
    }

    #[test]
    fn a_restore_that_moved_nothing_says_nothing_about_moving() {
        let said = restoration(&Restoration {
            would: Preview {
                manifest: manifest(),
                downgrade: false,
                relocation: None,
            },
            done: Some(Restored {
                scope: Scope::WholeStack,
                from_version: "0.7.0".to_owned(),
                relocated: None,
            }),
        })
        .text();
        assert!(!said.contains("Re-pointed"), "{said}");
    }

    /// A bundle holding one file, with whatever terms and gaps a test needs.
    fn holding(missing: Vec<String>, revealed: Vec<String>) -> Contents {
        Contents {
            pieces: vec![Piece {
                name: "diagnosis.txt".to_owned(),
                body: "all well".to_owned(),
            }],
            missing,
            taken: Taken {
                lemonfiber: "0.7.0".to_owned(),
                stack: "1.0.0".to_owned(),
                at: "2026-07-30T00:00:00Z".to_owned(),
            },
            terms: Terms {
                window: "the last 200 lines of each service".to_owned(),
                filenames: lemonfiber_core::bundle::Filenames::Replaced,
                revealed,
            },
        }
    }

    #[test]
    fn a_bundle_that_does_not_exist_yet_says_what_it_would_hold() {
        let said = bundle(&Bundle {
            contents: holding(
                vec!["the diagnosis could not run".to_owned()],
                vec!["INDEXER_KEY".to_owned()],
            ),
            bytes: 2048,
            path: None,
        })
        .text();
        assert!(said.contains("A support bundle would hold:"), "{said}");
        assert!(said.contains("diagnosis.txt"), "{said}");
        assert!(said.contains("Could not be read:"), "{said}");
        assert!(said.contains("it is, because you asked"), "{said}");
        assert!(said.contains("Nothing has been written."), "{said}");
    }

    #[test]
    fn more_than_one_revealed_setting_reads_as_more_than_one() {
        let said = bundle(&Bundle {
            contents: holding(
                Vec::new(),
                vec!["INDEXER_KEY".to_owned(), "VPN_KEY".to_owned()],
            ),
            bytes: 1,
            path: None,
        })
        .text();
        assert!(said.contains("they are, because you asked"), "{said}");
        assert!(!said.contains("Could not be read:"), "{said}");
    }

    #[test]
    fn a_bundle_that_exists_says_where_it_is_and_that_it_has_gone_nowhere() {
        let said = bundle(&Bundle {
            contents: holding(Vec::new(), Vec::new()),
            bytes: 4096,
            path: Some(PathBuf::from("/tmp/lemonfiber-support.tar.gz")),
        })
        .text();
        assert!(
            said.contains("Written to /tmp/lemonfiber-support.tar.gz"),
            "{said}"
        );
        assert!(said.contains("diagnosis.txt"), "{said}");
        assert!(said.contains("Nothing has left this machine."), "{said}");
    }

    #[test]
    fn the_backups_kept_here_are_listed_with_how_to_put_one_back() {
        let said = kept(&Listing {
            archives: vec![
                "lemonfiber-full-2.tar.gz".to_owned(),
                "lemonfiber-full-1.tar.gz".to_owned(),
            ],
        })
        .text();
        assert!(said.contains("lemonfiber-full-2.tar.gz"), "{said}");
        assert!(said.contains("lemonfiber-full-1.tar.gz"), "{said}");
        assert!(
            said.contains("lemonfiber restore <archive>"),
            "a listing says how to use what it listed: {said}"
        );
    }

    #[test]
    fn a_machine_that_has_kept_nothing_is_told_how_to_keep_something() {
        // An empty list and a list of one read alike where the answer is a bare
        // heading with nothing under it, which is the shape that reads as broken.
        let said = kept(&Listing {
            archives: Vec::new(),
        })
        .text();
        assert!(said.contains("No backups have been taken"), "{said}");
        assert!(said.contains("lemonfiber backup"), "{said}");
        assert!(
            !said.contains("lemonfiber restore <archive>"),
            "nothing to put back is not an invitation to put one back: {said}"
        );
    }
}

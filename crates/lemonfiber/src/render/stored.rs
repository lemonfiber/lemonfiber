//! What this machine keeps of lemonfiber's, on a terminal.
//!
//! The two directories lead, because the answer to "where is all this" is two paths
//! and an operator who reads no further has the whole of it. Each thing under them
//! is named with where it is and why it is kept, and the ones holding a credential
//! say so — that is the sentence that decides how carefully somebody treats a copy.
//!
//! What is *not* lemonfiber's comes last and is the part a removal turns on: an
//! operator about to remove everything needs to have read that the library is not
//! in the list before they agree to it, not afterwards.

use lemonfiber_core::stored::{Removal, Stored};

use super::Lines;

/// What is kept on this machine, and what became of it.
pub(crate) fn kept(report: &Stored) -> Lines {
    let mut lines = Lines::default();
    match &report.removal {
        Removal::Done { gone, left } => return removed(gone, left),
        Removal::NotAsked => lines.put("What lemonfiber keeps on this machine:"),
        Removal::Unconfirmed => {
            lines.put("Removing everything lemonfiber keeps would take all of this:");
        }
    }
    for root in &report.roots {
        lines.spaced(format!("  {}", root.at));
        lines.put(format!("    {}", root.what));
    }
    for entry in &report.kept {
        lines.spaced(format!(
            "  {}{}",
            entry.what,
            if entry.secret {
                " — holds a credential"
            } else {
                ""
            }
        ));
        lines.put(format!("    at   {}", entry.at));
        lines.put(format!("    why  {}", entry.why));
    }
    lines.extend(untouched(report));
    if matches!(report.removal, Removal::Unconfirmed) {
        lines.spaced("Nothing was removed. Add --confirm to remove it.");
    }
    lines
}

/// What is on this machine that lemonfiber does not keep and will not remove.
fn untouched(report: &Stored) -> Lines {
    let mut lines = Lines::default();
    if report.beside.is_empty() {
        return lines;
    }
    lines.spaced("Not lemonfiber's, and never removed:");
    for beside in &report.beside {
        lines.put(format!("  {} — {}", beside.what, beside.why));
    }
    lines
}

/// What a confirmed removal took, and what it could not.
fn removed(gone: &[String], left: &[lemonfiber_core::stored::Left]) -> Lines {
    let mut lines = Lines::default();
    if gone.is_empty() {
        lines.put("Nothing was removed.");
    } else {
        lines.put("Removed:");
        for at in gone {
            lines.put(format!("  {at}"));
        }
    }
    if left.is_empty() {
        lines.spaced("Your library, your downloads and the containers are untouched.");
        return lines;
    }
    lines.spaced("Still here, and each will have to be removed by hand:");
    for still in left {
        lines.put(format!("  {} — {}", still.at, still.why));
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lemonfiber_core::config::paths::Paths;
    use lemonfiber_core::stored::{stored, Left, Removal, Stored};

    use super::kept;

    fn a_layout() -> Paths {
        Paths::rooted(
            Path::new("/home/op/.config"),
            Path::new("/home/op/.local/share"),
        )
    }

    fn shown(removal: Removal) -> String {
        kept(&stored(&a_layout(), removal)).text()
    }

    #[test]
    fn a_listing_names_the_two_directories_and_everything_under_them() {
        let said = shown(Removal::NotAsked);
        assert!(said.contains("/home/op/.config/lemonfiber"), "{said}");
        assert!(said.contains("/home/op/.local/share/lemonfiber"), "{said}");
        assert!(said.contains("the settings file"), "{said}");
        assert!(said.contains("why "), "{said}");
    }

    #[test]
    fn what_holds_a_credential_says_so_beside_its_name() {
        let said = shown(Removal::NotAsked);
        assert!(
            said.contains("the settings file — holds a credential"),
            "{said}"
        );
    }

    #[test]
    fn what_is_not_lemonfibers_is_said_under_the_list_rather_than_left_out() {
        let said = shown(Removal::NotAsked);
        assert!(
            said.contains("Not lemonfiber's, and never removed:"),
            "{said}"
        );
        assert!(said.contains("your library and your downloads"), "{said}");
    }

    /// The consequence before the question: what would go is on the screen, and the
    /// line saying nothing has gone is under it.
    #[test]
    fn an_unconfirmed_run_lists_what_would_go_and_says_nothing_went() {
        let said = shown(Removal::Unconfirmed);
        assert!(
            said.starts_with("Removing everything lemonfiber keeps"),
            "{said}"
        );
        assert!(said.contains("/home/op/.config/lemonfiber"), "{said}");
        assert!(
            said.ends_with("Nothing was removed. Add --confirm to remove it."),
            "{said}"
        );
    }

    /// Through the printer rather than by calling this module, because what a
    /// terminal draws is what the printer chose for the outcome — an arm nothing
    /// reaches renders nowhere, however good the renderer under it is.
    #[test]
    fn the_printer_reaches_this_renderer_for_this_outcome() {
        let report = stored(&a_layout(), Removal::NotAsked);
        let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Stored(report)).text();
        assert!(
            drawn.contains("What lemonfiber keeps on this machine"),
            "{drawn}"
        );
        assert!(drawn.contains("the settings file"), "{drawn}");
    }

    /// A report naming nothing beside it says nothing about it, rather than opening
    /// a heading with no rows under it.
    #[test]
    fn a_report_with_nothing_beside_it_says_nothing_about_that() {
        let bare = Stored {
            beside: Vec::new(),
            ..stored(&a_layout(), Removal::NotAsked)
        };
        let said = kept(&bare).text();
        assert!(!said.contains("Not lemonfiber's"), "{said}");
        assert!(said.contains("the settings file"), "{said}");
    }

    #[test]
    fn a_confirmed_run_names_what_went_and_what_was_left_alone() {
        let said = shown(Removal::Done {
            gone: vec!["/home/op/.config/lemonfiber".to_owned()],
            left: Vec::new(),
        });
        assert!(said.contains("Removed:"), "{said}");
        assert!(said.contains("/home/op/.config/lemonfiber"), "{said}");
        assert!(said.contains("Your library, your downloads"), "{said}");
    }

    /// A directory that would not go is named with what the machine said about it.
    /// Being told everything was removed and finding one still there is being told
    /// something false.
    #[test]
    fn a_directory_that_would_not_go_is_named_with_the_reason() {
        let said = shown(Removal::Done {
            gone: Vec::new(),
            left: vec![Left {
                at: "/home/op/.local/share/lemonfiber".to_owned(),
                why: "permission denied".to_owned(),
            }],
        });
        assert!(said.starts_with("Nothing was removed."), "{said}");
        assert!(said.contains("removed by hand"), "{said}");
        assert!(said.contains("permission denied"), "{said}");
    }
}

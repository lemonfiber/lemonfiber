use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::{Command, CommandFactory};

use super::{declared, pages, write, Page, REFERENCE_DIR, REFERENCE_PATH};
use crate::cli::Cli;

/// Where the artefacts are committed, read from the workspace root.
fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every command this command line declares, as it is typed.
///
/// A flat walk rather than the grouped one [`pages`] makes, so the two are not
/// the same reading agreeing with itself.
fn every_command() -> Vec<String> {
    let mut root = Cli::command();
    root.build();
    let mut found = Vec::new();
    walk(&root, "lemonfiber", &mut found);
    found
}

/// One command and every one beneath it, by the name each is typed under.
fn walk(cmd: &Command, trail: &str, found: &mut Vec<String>) {
    found.push(trail.to_owned());
    for sub in declared(cmd) {
        walk(sub, &format!("{trail} {}", sub.get_name()), found);
    }
}

/// The file names the reference writes into its directory.
fn written() -> BTreeSet<String> {
    pages()
        .iter()
        .filter(|page| page.path != REFERENCE_PATH)
        .filter_map(|page| page.path.rsplit('/').next().map(str::to_owned))
        .collect()
}

/// The file names that are actually there.
fn found() -> BTreeSet<String> {
    std::fs::read_dir(workspace().join(REFERENCE_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// Every committed page and the declarations must agree.
///
/// A command renamed, added or removed without regenerating fails here rather
/// than leaving a reader following a document to a command that is not there.
#[test]
fn every_committed_page_still_matches_the_command_line() {
    let pages = pages();
    let made = pages.len();
    assert!(
        made > 40,
        "the render made {made} pages, which is the wrong tree"
    );
    for Page { path, text } in pages {
        let stored = std::fs::read_to_string(workspace().join(&path)).unwrap_or_default();
        assert_eq!(
            stored, text,
            "{path} is out of date — regenerate it with `just reference`"
        );
    }
}

/// A page nothing writes any more is as wrong as one that is missing.
///
/// The half a per-page comparison cannot catch: every page still there would
/// still match, and a reader would still be pointed at a command that is gone.
#[test]
fn the_directory_holds_exactly_the_pages_the_reference_writes() {
    let written = written();
    let named = written.len();
    assert!(
        named > 40,
        "the render named {named} pages, which is the wrong tree"
    );
    assert_eq!(
        found(),
        written,
        "the directory and the reference disagree — regenerate it with `just reference`"
    );
}

/// Every command the command line declares is described exactly once.
///
/// Across the set of pages rather than within one. Rendering only the root would
/// match itself, and so would a set that quietly lost a page, so completeness has
/// to name what completeness is.
#[test]
fn every_command_is_described_on_exactly_one_page() {
    let pages = pages();
    let commands = every_command();
    let walked = commands.len();
    assert!(
        walked > 60,
        "the walk found {walked} commands, which is the wrong tree"
    );
    for trail in commands {
        let heading = format!("## `{trail}`");
        let on = pages
            .iter()
            .filter(|page| page.text.contains(&heading))
            .count();
        assert_eq!(on, 1, "{trail} is described on {on} pages");
    }
}

/// The names a reader would go looking for, on the pages they belong to.
///
/// Named rather than derived, beside the walk above: a derived check and the
/// thing it checks can be wrong together, and these are the shapes that matter —
/// a command with no subcommands, one with several, and one of those beneath it.
#[test]
fn a_command_and_the_ones_beneath_it_share_a_page() {
    for (name, described) in [
        ("setup", &["lemonfiber setup"][..]),
        (
            "config",
            &[
                "lemonfiber config",
                "lemonfiber config get",
                "lemonfiber config set",
                "lemonfiber config show",
            ][..],
        ),
        (
            "plugin",
            &[
                "lemonfiber plugin",
                "lemonfiber plugin provenance",
                "lemonfiber plugin install",
                "lemonfiber plugin installed",
            ][..],
        ),
    ] {
        let at = format!("{REFERENCE_DIR}/{name}.md");
        let page = pages()
            .into_iter()
            .find(|page| page.path == at)
            .map(|page| page.text)
            .unwrap_or_default();
        assert!(!page.is_empty(), "{at} is not a page");
        for trail in described {
            assert!(page.contains(&format!("## `{trail}`")), "{at}: {trail}");
        }
    }
}

/// The index points at every page, and at nothing else.
#[test]
fn the_index_links_to_every_page() {
    let index = pages()
        .into_iter()
        .find(|page| page.path == REFERENCE_PATH)
        .map(|page| page.text)
        .unwrap_or_default();
    let named = written();
    assert!(!named.is_empty(), "the render named no pages");
    for file in &named {
        assert!(index.contains(&format!("](commands/{file})")), "{file}");
    }
    assert_eq!(
        index.matches("](commands/").count(),
        named.len(),
        "the index links to something that is not a page"
    );
}

/// clap's own `help` subcommand is not one of the declared commands.
#[test]
fn it_leaves_out_the_subcommand_clap_adds_itself() {
    assert!(!written().contains("help.md"));
    for page in pages() {
        assert!(!page.text.contains("## `lemonfiber help`"), "{}", page.path);
    }
}

/// The help must arrive as plain text, on every page.
///
/// clap renders styled help too, and a page carrying escape sequences would
/// still compare equal to itself while being unreadable everywhere it is shown.
#[test]
fn no_page_carries_an_escape_sequence() {
    let pages = pages();
    let made = pages.len();
    assert!(
        made > 40,
        "the render made {made} pages, which is the wrong tree"
    );
    for page in pages {
        assert!(!page.text.contains('\u{1b}'), "{}", page.path);
    }
}

/// Writing the reference takes a page away with the command it described.
///
/// The one thing a redirect of stdout could never do, and the reason writing is
/// the renderer's rather than the recipe's.
#[test]
fn writing_the_reference_removes_a_page_it_no_longer_has() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("reference");
    let _ = std::fs::remove_dir_all(&root);
    let at = root.join(REFERENCE_DIR);
    let _ = std::fs::create_dir_all(&at);
    let abandoned = at.join("a-command-nobody-declares.md");
    let _ = std::fs::write(&abandoned, "left behind");
    // Beside it, a page the reference does have, so the loop that decides what
    // to remove is asked about both kinds rather than only the one it takes.
    let kept = at.join("setup.md");
    let _ = std::fs::write(&kept, "out of date");

    let wrote = write(&root);
    assert!(wrote.is_ok(), "{wrote:?}");
    assert!(
        !abandoned.exists(),
        "a page for a command nobody declares was left behind"
    );
    assert_ne!(
        std::fs::read_to_string(&kept).unwrap_or_default(),
        "out of date",
        "a page the reference does have was not rewritten"
    );

    let pages = pages();
    let made = pages.len();
    assert!(
        made > 40,
        "the render made {made} pages, which is the wrong tree"
    );
    for page in pages {
        assert!(
            root.join(&page.path).is_file(),
            "{} was not written",
            page.path
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

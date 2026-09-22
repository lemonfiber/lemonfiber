//! The command reference, rendered from the declarations the binary parses with.
//!
//! `just reference` writes it. The comparison lives in a test, so a stale artefact
//! fails the build rather than the program that emits it.
//!
//! **A page per top-level command**, with everything declared beneath it on the same
//! page, and an index linking to each. The grouping is clap's own tree rather than a
//! table somebody maintains, which is the only kind that cannot drift: a command
//! added gets a page, and one removed takes its page with it.
//!
//! **The set of pages is part of what is compared.** A page left behind for a
//! command that no longer exists is as wrong as a missing one, and it is the wrong
//! that nothing would otherwise notice — every page still there would still match.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use clap::{Command, CommandFactory};

use crate::cli::Cli;

/// Where the index is kept, relative to the workspace root.
pub const REFERENCE_PATH: &str = "reference/commands.md";

/// Where the per-command pages are kept, relative to the workspace root.
pub const REFERENCE_DIR: &str = "reference/commands";

/// What the index opens with, before the root command.
const PREAMBLE: &str = "\
# `lemonfiber` — command reference

Generated from the command line's own declarations. Run `just reference` to rewrite it.
";

/// One page of the reference, and where it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Where it goes, relative to the workspace root, with `/` between its parts.
    pub path: String,
    /// The whole of it, ready to write.
    pub text: String,
}

/// Every page of the reference: the index, then one per top-level command.
///
/// clap wraps help text only under its `wrap_help` feature, which this workspace
/// does not enable, so nothing here asks the terminal how wide it is and the same
/// commit renders the same pages on every machine.
#[must_use]
pub fn pages() -> Vec<Page> {
    let mut root = Cli::command();
    root.build();
    let commands: Vec<Command> = declared(&root).cloned().collect();

    let mut pages = vec![Page {
        path: REFERENCE_PATH.to_owned(),
        text: index(&mut root, &commands),
    }];
    for mut one in commands {
        pages.push(page(&mut one));
    }
    pages
}

/// Write every page, and remove any the reference no longer has.
///
/// The removal is the half a redirect of stdout could never do, and the half that
/// matters: a page for a command that was deleted would otherwise sit there
/// describing something nobody can type, matching itself for ever.
///
/// # Errors
///
/// Whatever the filesystem said, where a page could not be written or an abandoned
/// one could not be removed.
pub fn write(root: &Path) -> io::Result<()> {
    let pages = pages();
    let at = root.join(REFERENCE_DIR);
    std::fs::create_dir_all(&at)?;

    let written: BTreeSet<String> = pages
        .iter()
        .filter_map(|page| page.path.rsplit('/').next().map(str::to_owned))
        .collect();
    for entry in std::fs::read_dir(&at)? {
        let found = entry?.file_name();
        if !written.contains(found.to_string_lossy().as_ref()) {
            std::fs::remove_file(at.join(found))?;
        }
    }

    for page in pages {
        std::fs::write(root.join(&page.path), page.text)?;
    }
    Ok(())
}

/// The subcommands a command declares, less the one clap adds itself.
///
/// `help` is clap's own addition to any command that has subcommands, not part of
/// what this command line declares, so it is not described.
fn declared(cmd: &Command) -> impl Iterator<Item = &Command> {
    cmd.get_subcommands().filter(|sub| sub.get_name() != "help")
}

/// The root command's own help, and where to read about each of the rest.
fn index(root: &mut Command, commands: &[Command]) -> String {
    let mut out = String::from(PREAMBLE);
    section(root, "lemonfiber", &mut out);
    out.push_str("\n## Every command\n\n");
    let directory = REFERENCE_DIR.rsplit('/').next().unwrap_or(REFERENCE_DIR);
    for one in commands {
        let name = one.get_name();
        out.push_str("- [`lemonfiber ");
        out.push_str(name);
        out.push_str("`](");
        out.push_str(directory);
        out.push('/');
        out.push_str(name);
        out.push_str(".md)\n");
    }
    out
}

/// One top-level command and everything declared beneath it.
fn page(cmd: &mut Command) -> Page {
    let name = cmd.get_name().to_owned();
    let mut text = format!(
        "# `lemonfiber {name}`\n\nGenerated from the command line's own declarations. \
         Run `just reference` to rewrite it.\nPart of the [command reference](../commands.md).\n"
    );
    describe(cmd, "lemonfiber", &mut text);
    Page {
        path: format!("{REFERENCE_DIR}/{name}.md"),
        text,
    }
}

/// One command's help, then the ones it declares.
fn describe(cmd: &mut Command, path: &str, out: &mut String) {
    let trail = format!("{path} {}", cmd.get_name());
    section(cmd, &trail, out);
    for mut sub in declared(cmd).cloned().collect::<Vec<Command>>() {
        describe(&mut sub, &trail, out);
    }
}

/// One command's help, under the name it is typed by.
fn section(cmd: &mut Command, trail: &str, out: &mut String) {
    let help = cmd.render_long_help().to_string();
    out.push_str("\n## `");
    out.push_str(trail);
    out.push_str("`\n\n```text\n");
    for line in help.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str("```\n");
}

#[cfg(test)]
mod tests {
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
        let root =
            std::env::temp_dir().join(format!("lemonfiber-reference-{}", std::process::id()));
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
}

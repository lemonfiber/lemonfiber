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
mod tests;

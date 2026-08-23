//! The command reference, rendered from the declarations the binary parses with.
//!
//! `just reference` writes it to the committed artefact. The comparison lives in a
//! test, so a stale artefact fails the build rather than the program that emits it.

use clap::{Command, CommandFactory};

use crate::cli::Cli;

/// Where the generated artefact is kept, relative to the workspace root.
pub const REFERENCE_PATH: &str = "reference/commands.md";

/// What the artefact opens with, before the first command.
const PREAMBLE: &str = "\
# `lemonfiber` — command reference

Generated from the command line's own declarations. Run `just reference` to rewrite it.
";

/// The whole reference: the root command, then every subcommand it declares.
///
/// clap wraps help text only under its `wrap_help` feature, which this workspace
/// does not enable, so nothing here asks the terminal how wide it is and the same
/// commit renders the same artefact on every machine.
#[must_use]
pub fn render() -> String {
    let mut out = String::from(PREAMBLE);
    let mut root = Cli::command();
    root.build();
    describe(&mut root, &[], &mut out);
    out
}

/// Appends one command's help, then recurses into the ones it declares.
///
/// `help` is clap's own addition to any command that has subcommands, not part of
/// what this command line declares, so it is not described.
fn describe(cmd: &mut Command, path: &[String], out: &mut String) {
    let mut trail: Vec<String> = path.to_vec();
    trail.push(cmd.get_name().to_owned());
    let help = cmd.render_long_help().to_string();

    out.push_str("\n## `");
    out.push_str(&trail.join(" "));
    out.push_str("`\n\n```text\n");
    for line in help.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str("```\n");

    let subs: Vec<Command> = cmd
        .get_subcommands()
        .filter(|sub| sub.get_name() != "help")
        .cloned()
        .collect();
    for mut sub in subs {
        describe(&mut sub, &trail, out);
    }
}

#[cfg(test)]
mod tests {
    use super::{render, REFERENCE_PATH};

    /// What is committed, read from the workspace root.
    fn committed() -> Option<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(REFERENCE_PATH)).ok()
    }

    /// The committed artefact and the declarations must agree.
    ///
    /// A command renamed, added or removed without regenerating fails here rather
    /// than leaving a reader following a document to a command that is not there.
    #[test]
    fn the_committed_reference_still_matches_the_command_line() {
        let fresh = render();
        let stored = committed().unwrap_or_default();

        assert_eq!(
            stored, fresh,
            "the command reference is out of date — regenerate it with `just reference`"
        );
    }

    /// Every subcommand the command line declares gets a section of its own.
    ///
    /// Rendering only the root would still produce an artefact that matches itself,
    /// so the check that it is complete has to name what completeness is.
    #[test]
    fn it_describes_every_subcommand_and_the_ones_beneath_them() {
        let text = render();

        for name in [
            "lemonfiber setup",
            "lemonfiber version",
            "lemonfiber up",
            "lemonfiber support",
            "lemonfiber walkthrough",
            "lemonfiber config get",
            "lemonfiber quality upgrade",
        ] {
            assert!(text.contains(&format!("## `{name}`")), "{name} missing");
        }
    }

    /// clap's own `help` subcommand is not one of the declared commands.
    #[test]
    fn it_leaves_out_the_subcommand_clap_adds_itself() {
        assert!(!render().contains("## `lemonfiber help`"));
    }

    /// The help must arrive as plain text.
    ///
    /// clap renders styled help too, and an artefact carrying escape sequences would
    /// still compare equal to itself while being unreadable everywhere it is shown.
    #[test]
    fn it_carries_no_escape_sequence() {
        assert!(!render().contains('\u{1b}'));
    }
}

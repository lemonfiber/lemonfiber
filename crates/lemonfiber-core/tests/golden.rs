//! Every form's Compose invocation, pinned to a file.
//!
//! This is the highest-consequence surface in the crate — *does lemonfiber
//! generate the right `docker compose` command?* — and command construction is
//! pure, so all of it is covered here with no daemon, no containers and no
//! network.
//!
//! Each form is asserted on **every** environment against the same file. That
//! the four agree is a fact about today rather than a rule: when an environment
//! first changes the command, its assertion fails, and that form's file splits
//! per environment. Writing four identical files now would hide that moment
//! instead of catching it.
//!
//! Regenerate after an intended change:
//!
//! ```sh
//! LEMONFIBER_BLESS_GOLDEN=1 cargo test -p lemonfiber-core --test golden
//! ```
//!
//! Then read the diff. A golden file updated without being read is a test that
//! asserts whatever the code happens to do.

use std::fs;
use std::path::{Path, PathBuf};

use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::closure::resolve;
use lemonfiber_core::stack::compose::{build, Action};
use lemonfiber_manifest::Manifest;

/// The stack this repository carries.
const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

/// A fixed directory, so a golden file does not depend on where it was written.
const STACK_DIR: &str = "/opt/lemonfiber/stack";

/// All four environments, so each file is asserted against every one of them.
const ENVIRONMENTS: [Environment; 4] = [
    Environment::MacOs,
    Environment::LinuxNative,
    Environment::LinuxDesktop,
    Environment::Windows,
];

fn golden(form: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("up_{form}.txt"))
}

#[test]
fn every_form_matches_its_golden_command_on_every_environment() {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately");
    };

    let bless = std::env::var_os("LEMONFIBER_BLESS_GOLDEN").is_some();
    let settings = Settings {
        protocols: Protocols::both(),
        ..Settings::default()
    };

    assert!(!manifest.forms.is_empty(), "the stack declares forms");

    for form in &manifest.forms {
        let named = vec![form.id.clone()];
        let Ok(plan) = resolve(&manifest, &named, Protocols::both()) else {
            unreachable!("every declared form resolves with both protocols configured");
        };

        let mut rendered = None;
        for environment in ENVIRONMENTS {
            let argv = build(
                &plan,
                &settings,
                Path::new(STACK_DIR),
                &Action::Up,
                environment,
            );
            let line = argv.join(" ");
            match &rendered {
                None => rendered = Some(line),
                Some(first) => assert_eq!(
                    &line, first,
                    "`{}` builds a different command on {environment:?}; \
                     if that is intended, split its golden file per environment",
                    form.id
                ),
            }
        }

        let Some(line) = rendered else {
            unreachable!("there is at least one environment");
        };
        let path = golden(&form.id);

        if bless {
            let Some(parent) = path.parent() else {
                unreachable!("the golden path has a directory");
            };
            let _ = fs::create_dir_all(parent);
            let _ = fs::write(&path, format!("{line}\n"));
            continue;
        }

        let recorded = fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            recorded.trim_end(),
            line,
            "`{}` no longer builds its recorded command ({})",
            form.id,
            path.display()
        );
    }
}

#[test]
fn there_is_a_golden_file_for_every_form_and_no_others() {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses");
    };

    let mut expected: Vec<String> = manifest
        .forms
        .iter()
        .map(|form| format!("up_{}.txt", form.id))
        .collect();
    expected.sort();

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut found: Vec<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();

    assert_eq!(
        found, expected,
        "a form without a golden file is untested, and a golden file without a \
         form is asserting something the stack no longer declares"
    );
}

//! The boundaries, made executable.
//!
//! Every rule here is one a reviewer would otherwise have to hold in their head
//! on every pull request. Reviewers are inconsistent about that and a test is
//! not, which is the whole argument for writing them down as code.
//!
//! These tests read source text rather than the compiled crate. That is coarse,
//! and it is enough: each rule is about where a name is allowed to *appear*.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file in the workspace, keyed by its path relative to the root.
fn sources() -> BTreeMap<PathBuf, String> {
    let root = workspace_root();
    let mut found = BTreeMap::new();
    collect(&root.join("crates"), &root, &mut found);
    assert!(
        found.len() > 10,
        "the crawler found {} files, which means it is looking in the wrong place",
        found.len()
    );
    found
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest.parent().and_then(Path::parent) else {
        unreachable!("this crate lives two directories below the workspace root");
    };
    root.to_path_buf()
}

fn collect(dir: &Path, root: &Path, found: &mut BTreeMap<PathBuf, String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let Ok(text) = fs::read_to_string(&path) else {
                unreachable!("a source file that was just listed should be readable");
            };
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            found.insert(relative, text);
        }
    }
}

/// Files whose path contains any of `segments`.
fn outside(path: &Path, segments: &[&str]) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    !segments.iter().any(|segment| text.contains(segment))
}

/// The core cannot render.
///
/// This is the boundary everything else rests on: a surface cannot acquire
/// behaviour of its own if the behaviour lives somewhere that has no way to show
/// anything. Checked against the dependency declarations rather than the source,
/// because the crate graph is where it is actually enforced.
#[test]
fn the_core_has_no_user_interface_dependency() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-core/Cargo.toml")) else {
        unreachable!("the core crate has a manifest");
    };

    for forbidden in [
        "ratatui",
        "crossterm",
        "clap",
        "axum",
        "hyper",
        "color-eyre",
        "console",
        "indicatif",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "lemonfiber-core must not depend on `{forbidden}` — it cannot render"
        );
    }
}

/// Each external dependency has exactly one legitimate home.
///
/// Without this, "compose invocation and engine access live in separate modules"
/// is a sentence in a document. With it, a subsystem that grows its own way out
/// to the network fails the build.
#[test]
fn talking_to_the_outside_world_only_happens_in_adapters() {
    let confined: [(&str, &[&str]); 8] = [
        ("tokio::process", &["adapters/process.rs"]),
        ("std::process::Command", &["adapters/process.rs"]),
        (
            "bollard",
            &["adapters/docker.rs", "adapters/docker/translate.rs"],
        ),
        ("reqwest", &["adapters/http.rs"]),
        ("sysinfo", &["adapters/filesystem.rs"]),
        // The HTTP adapter names it too: it is reqwest's TLS backend, and the
        // reason a static Linux build carries no system TLS. Both are its homes.
        ("rustls", &["adapters/nntp.rs", "adapters/http.rs"]),
        ("webpki_roots", &["adapters/nntp.rs"]),
        // Not an adapter — YAML is a format, and reading one is pure. Confined
        // for the same reason the rest are: the day something else wants to read
        // a compose file, it asks the module that already knows how.
        ("serde_yaml_ng", &["stack/mounts.rs"]),
    ];

    for (crate_name, permitted) in confined {
        for (path, text) in sources() {
            if !path.starts_with("crates/lemonfiber-core") {
                continue;
            }
            assert!(
                !(text.contains(crate_name) && outside(&path, permitted)),
                "`{crate_name}` appears in {} — it belongs only in {permitted:?}",
                path.display()
            );
        }
    }
}

/// Deciding what platform this is happens in one place.
///
/// Conditional compilation scattered through call sites is how a codebase
/// becomes impossible to exercise on any single machine; everything else asks
/// the one component that knows.
#[test]
fn only_the_platform_module_asks_which_operating_system_this_is() {
    for (path, text) in sources() {
        if !path.starts_with("crates/lemonfiber-core") {
            continue;
        }
        assert!(
            !(text.contains("target_os") && outside(&path, &["platform.rs"])),
            "{} tests the operating system directly — ask the platform module instead",
            path.display()
        );
    }
}

/// Suppressions are not how a rule gets satisfied.
///
/// An allow attribute turns a standard the whole codebase is held to into one
/// that applies wherever nobody objected. Change the code, or change the rule
/// for everyone in the workspace manifest.
#[test]
fn no_lint_is_suppressed_in_source() {
    for (path, text) in sources() {
        if path.starts_with("crates") && path.to_string_lossy().contains("tests") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            assert!(
                !(trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow(")),
                "{}:{} suppresses a lint — change the code, or change the rule for everyone",
                path.display(),
                number + 1
            );
        }
    }
}

/// A test file covers one seam, and stays small enough to read.
///
/// The production cap does not apply here: a test file legitimately carries
/// fixtures, fakes and every case of one thing, and holding it to 550 would push
/// the shared scaffolding into ever more modules rather than making anything
/// clearer.
///
/// What does apply is the reason behind that cap. One file, one seam. Past this
/// length a file has stopped being the tests for one thing and become the tests
/// for a subsystem — which is how `seed.rs` reached two and a half thousand lines
/// covering five different drivers, each with a fake nobody else could see.
///
/// The number is a ratchet, not a target: it should come down as files are split,
/// never up to admit one that grew.
#[test]
fn no_test_file_covers_more_than_one_seam() {
    /// Lines a single test file may hold.
    const CAP: usize = 1_200;

    let oversized: Vec<(PathBuf, usize)> = sources()
        .into_iter()
        .filter(|(path, _)| path.to_string_lossy().contains("tests"))
        .map(|(path, text)| (path, text.lines().count()))
        .filter(|(_, lines)| *lines > CAP)
        .collect();
    assert!(
        oversized.is_empty(),
        "past {CAP} lines a test file covers more than one seam — split it: {oversized:?}"
    );
}

/// No two problems answer to the same code.
///
/// The error model deliberately declares each code as a `const` beside the error
/// that raises it rather than in one shared enum, so that adding an error is not
/// editing a list everyone edits. What that decision costs is the one property a
/// central list would have given for free: nothing stops two errors picking the
/// same string.
///
/// It costs something real. An operator who searches for a code should find the
/// same answer a year later, and for four releases `VPN-5` was both a port
/// mismatch and a killswitch leak — so whoever looked one up found the other.
#[test]
fn no_two_problems_answer_to_the_same_code() {
    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut collisions: Vec<String> = Vec::new();
    for (path, text) in sources() {
        // Tests reuse real codes as fixtures, which is not a second declaration
        // of one — the production half of each file is what declares.
        let production = text.split("#[cfg(test)]").next().unwrap_or_default();
        for code in declared(production) {
            if let Some(first) = seen.insert(code.clone(), path.clone()) {
                collisions.push(format!(
                    "{code} in {} and {}",
                    first.display(),
                    path.display()
                ));
            }
        }
    }
    assert!(collisions.is_empty(), "{collisions:?}");
}

/// Every code declared in a piece of source.
fn declared(text: &str) -> Vec<String> {
    text.match_indices("Code::new(\"")
        .filter_map(|(at, opening)| {
            let from = at + opening.len();
            let rest = text.get(from..)?;
            rest.find('"')
                .and_then(|end| rest.get(..end))
                .map(str::to_owned)
        })
        .collect()
}

/// Requirement identifiers do not belong in code.
///
/// Provenance in a comment is worthless to the next reader and rots the moment
/// the requirement is superseded. Identifiers go in commits and pull requests;
/// code links to `.docs/`, and those pages cite the specification.
#[test]
fn no_requirement_identifier_appears_in_a_comment() {
    let prefixes = ["ARCH-R", "REPO-R", "GOV-R", "OPS-R", "Q-R", "DES-R", "ADR-"];

    for (path, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*")) {
                continue;
            }
            for prefix in prefixes {
                assert!(
                    !line.contains(prefix),
                    "{}:{} cites `{prefix}…` in a comment — cite it in the commit instead",
                    path.display(),
                    number + 1
                );
            }
        }
    }
}

/// Feature-area identifiers are caught too.
///
/// Separate from the prefix list above because these have no fixed prefix — the
/// shape is an uppercase letter, a digit, a dash, an R, then a digit. Written as
/// a character test rather than as an example, since an example would be a
/// requirement identifier in a comment and this test would find it.
#[test]
fn no_feature_requirement_identifier_appears_in_a_comment() {
    for (path, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*")) {
                continue;
            }
            let characters: Vec<char> = line.chars().collect();
            for window in characters.windows(5) {
                let [area, feature, dash, marker, index] = window else {
                    continue;
                };
                let looks_like_an_identifier = area.is_ascii_uppercase()
                    && feature.is_ascii_digit()
                    && *dash == '-'
                    && *marker == 'R'
                    && index.is_ascii_digit();
                assert!(
                    !looks_like_an_identifier,
                    "{}:{} cites a requirement in a comment — cite it in the commit instead",
                    path.display(),
                    number + 1
                );
            }
        }
    }
}

/// No file grows past what one sitting can hold.
///
/// A file that keeps accreting is how a codebase stops being navigable: the third
/// concern arrives, nobody notices, and by the fifth there is nowhere obvious to
/// put the sixth. The cap is deliberately mechanical — it makes no judgement about
/// whether a file is *cohesive*, only that it is finite — and it is a floor under
/// review rather than a substitute for it.
///
/// Counted before the test module, because tests legitimately double a file and
/// there is no reason to ration them.
///
/// When this fails, the answer is a module: split the file into a directory of the
/// same name, one concern per file, tests beside the code they exercise and shared
/// fixtures in a `fixtures.rs` of their own. Raising the number is not the answer.
#[test]
fn no_source_file_outgrows_reading_in_one_sitting() {
    /// Production lines a single file may hold.
    const CAP: usize = 550;

    let mut oversized: Vec<(PathBuf, usize)> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let production = text
            .lines()
            .position(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("mod tests") || trimmed.starts_with("pub(crate) mod tests")
            })
            .map_or_else(|| text.lines().count(), |at| at.saturating_sub(1));
        if production > CAP {
            oversized.push((path, production));
        }
    }
    oversized.sort_by_key(|(_, lines)| std::cmp::Reverse(*lines));

    let named: Vec<String> = oversized
        .iter()
        .map(|(path, lines)| format!("{} ({lines})", path.display()))
        .collect();
    assert!(
        oversized.is_empty(),
        "past {CAP} production lines, split into a module rather than raising the cap: {}",
        named.join(", ")
    );
}

/// A failure reaches the operator on stderr, so a script can read the answer on
/// stdout and a person can read the problem beside it.
///
/// Guarded rather than merely true: `complain` is one function, and the day
/// somebody adds a line to it the difference between `println!` and `eprintln!`
/// is one character and no test. A machine-readable run that mixed its diagnosis
/// into its output would be broken in a way nothing else here would catch.
#[test]
fn a_failure_is_reported_on_stderr_and_never_on_stdout() {
    let exit = std::fs::read_to_string("src/exit.rs").unwrap_or_default();
    let complain = exit
        .split_once("pub(crate) fn complain")
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .map(|(body, _)| body)
        .unwrap_or_default();

    assert!(!complain.is_empty(), "the reporter was found");
    assert!(
        complain.contains("eprintln!"),
        "it reports something at all"
    );
    assert!(
        !complain.contains("println!(") || !complain.contains(" println!"),
        "a diagnosis on stdout would corrupt a machine-readable run"
    );
    for line in complain.lines() {
        let statement = line.trim_start();
        assert!(
            !statement.starts_with("println!"),
            "reports on stdout: {statement}"
        );
        // Nothing in a failure path may wait for a person: a non-interactive run
        // has nobody to answer, and a prompt there hangs a script for ever.
        assert!(
            !statement.contains("read_line") && !statement.contains("stdin"),
            "prompts while reporting: {statement}"
        );
    }
}

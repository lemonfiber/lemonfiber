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

/// A manifest with its comments stripped.
///
/// Both rules below are about what a crate *declares*, and both manifests explain
/// themselves in comments that name the very crate being forbidden. Reading the raw
/// text would fail on the documentation for the rule it is enforcing.
fn declarations(manifest: &str) -> String {
    manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// The boundary sits below the logic, and the build is what says so.
///
/// A port that could reach up into `lemonfiber-core` would stop being a boundary:
/// the seam and the thing it is a seam for would depend on each other, and the
/// claim that the boundary is the stable part would be a sentence in a document
/// rather than a property. Cargo refuses that particular edge outright, since it
/// is a cycle — this states the rule anyway, because the manifest is where a
/// future dependency would be added and this is where it should be argued with.
#[test]
fn the_ports_crate_depends_on_nothing_of_ours_but_the_manifest() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-ports/Cargo.toml")) else {
        unreachable!("the ports crate has a manifest");
    };

    let declared = declarations(&manifest);
    for forbidden in ["lemonfiber-core", "lemonfiber-fixtures", "lemonfiber ="] {
        assert!(
            !declared.contains(forbidden),
            "lemonfiber-ports must not depend on `{forbidden}` — a boundary that reaches \
             up into the logic is not a boundary"
        );
    }
}

/// The fakes have one home, and it stays reachable from both kinds of test.
///
/// A crate's in-source test modules and its `tests/` directory are separate
/// compilation units, so a fake defined in either is invisible to the other. That
/// is how one port came to be faked twice and the filesystem four times, and why
/// the fixtures live in a crate rather than a module.
///
/// The rule below is the load-bearing half. Cargo *permits* a development-
/// dependency cycle: were the fixtures to depend on `lemonfiber-core`, it would
/// build the core twice and hand the fake a trait belonging to neither copy the
/// test is using. Nothing fails — it compiles, and the fake simply never matches.
/// A silent failure is worth a test that cannot be argued out of.
#[test]
fn the_fixtures_crate_does_not_depend_on_the_core() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-fixtures/Cargo.toml"))
    else {
        unreachable!("the fixtures crate has a manifest");
    };

    assert!(
        !declarations(&manifest).contains("lemonfiber-core"),
        "lemonfiber-fixtures must not depend on lemonfiber-core — Cargo allows the \
         development-dependency cycle and then builds the core twice, leaving every fake \
         implementing a trait belonging to neither copy under test"
    );
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

/// Each requirement is claimed by exactly one row of the status file.
///
/// The release gate reads done-ness from that file: a requirement named in a ✅
/// row is finished, and one named in a ☐ row is not. A requirement named in both
/// is a question the gate answers by whichever row it reads first — and it has
/// been wrong three times, once for four releases.
///
/// Only the identifier column counts. The prose beside it cites requirements
/// freely, which is how a row explains itself, and citing one is not claiming it.
#[test]
fn each_requirement_is_claimed_by_one_row() {
    let file = fs::read_to_string("../../IMPLEMENTATION-STATUS.md").unwrap_or_default();
    let mut claimed: BTreeMap<String, usize> = BTreeMap::new();
    for row in file.lines().filter(|line| line.starts_with('|')) {
        for id in requirements(column(row, 1)) {
            *claimed.entry(id).or_default() += 1;
        }
    }
    let twice: Vec<&String> = claimed
        .iter()
        .filter(|(_, rows)| **rows > 1)
        .map(|(id, _)| id)
        .collect();
    assert!(
        twice.is_empty(),
        "claimed by more than one row, so the gate reads whichever it finds first: {twice:?}"
    );
}

/// One cell of a table row, or nothing where the row is too short.
fn column(row: &str, at: usize) -> &str {
    row.split('|').nth(at + 1).unwrap_or_default()
}

/// Every requirement a cell claims, with `X-R1..R4` counted as the four it means.
///
/// Anything that is not a requirement identifier — a command name, a feature with
/// no requirement number, an error code — is passed over rather than guessed at.
fn requirements(cell: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in cell.split('`').skip(1).step_by(2) {
        let Some((feature, numbers)) = token.split_once("-R") else {
            continue;
        };
        if !feature
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            continue;
        }
        let (first, last) = numbers.split_once("..").unwrap_or((numbers, numbers));
        let last = last.trim_start_matches('R');
        let (Ok(first), Ok(last)) = (first.parse::<u32>(), last.parse::<u32>()) else {
            continue;
        };
        found.extend((first..=last).map(|number| format!("{feature}-R{number}")));
    }
    found
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

//! The boundaries, made executable.
//!
//! Every rule here is one a reviewer would otherwise have to hold in their head
//! on every pull request. Reviewers are inconsistent about that and a test is
//! not, which is the whole argument for writing them down as code.
//!
//! These tests read source text rather than the compiled crate. That is coarse,
//! and it is enough: each rule is about where a name is allowed to *appear*.
//!
//! What those words say is guarded next door, in `plain_language.rs`, over the same
//! crawl of the tree.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

mod source_tree;

use source_tree::{production, sources, workspace_root};

/// Reading a latch never settles it.
///
/// Four values here are settled once at startup and read everywhere after: what a
/// terminal can draw, who the output is for, whether words are explained, and which
/// have already been. A read that reached for `get_or_init` would settle the value
/// itself — and the `settle` that came afterwards would then be **silently ignored**,
/// in whatever order some future caller happened to introduce. Nothing reports that;
/// the feature simply stops working for that run.
///
/// One of them was written that way and this is why it is pinned. Only a function
/// that says it settles may settle.
#[test]
fn reading_a_latch_never_settles_it() {
    let mut settling: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains("/src/") {
            continue;
        }
        let mut whose = String::new();
        for (number, line) in production(&text).lines().enumerate() {
            if let Some(name) = declaring(line) {
                whose = name;
            }
            if line.contains("get_or_init") && !whose.starts_with("settle") {
                settling.push(format!("{where_it_lives}:{}: {whose}", number + 1));
            }
        }
    }
    assert!(
        settling.is_empty(),
        "a read that settles the value it reads, so a later settle is ignored: \
         {settling:?}"
    );
}

/// The name of the function a line declares, where it declares one.
fn declaring(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The body of a named function in a piece of source, or nothing where it is absent.
///
/// Read from the source rather than by running it, because what this pins is which
/// stream a failure reaches, and a test that called the reporter would have to
/// capture this process's own stderr to find out — a harness rather than a test.
fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
    source
        .split_once(signature)
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .map(|(body, _)| body)
        .unwrap_or_default()
}

/// The text of a call, from its opening bracket to the one that closes it.
///
/// Read by counting brackets rather than by taking a fixed number of lines, because
/// a fixed window is wrong in both directions: it misses a call that formatting has
/// spread wider than the window, and it blames a call for a line that merely follows
/// it. Neither is hypothetical here — `engine.rs` already has an ordinary `say!`
/// sitting seven lines above an unrelated `to_json()`, which a six-line window
/// cleared by one line.
fn invocation(text: &str, opens: usize) -> &str {
    let Some(rest) = text.get(opens..) else {
        return "";
    };
    let mut depth = 0_usize;
    for (at, character) in rest.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return rest.get(..=at).unwrap_or(rest);
                }
            }
            _ => {}
        }
    }
    rest
}

/// What a parser reads is never rendered for a person.
///
/// Folding decides what a person's terminal can draw, and `--json` has no person on
/// the other end of it. There it is not merely unnecessary but damaging: the fold
/// writes a curly quote as `"`, and inside a JSON string that is not a character but
/// the end of it — so a release name carrying one arrives as something that will not
/// parse at all. `--json` is for scripts, and a script is exactly where `LC_ALL=C`
/// is set, which is what turns folding on. The two meet more often than not.
///
/// Guarded by shape rather than by output, because the fault appears only on a
/// terminal that folds and only for text carrying one of a handful of characters. A
/// test of what was printed would pass on the machine of whoever broke it.
#[test]
fn nothing_a_parser_reads_is_rendered_for_a_person() {
    let mut rendered: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains("/src/") {
            continue;
        }
        let shipped = production(&text);
        for (at, _) in shipped.match_indices("say!(") {
            if !invocation(shipped, at + "say!".len()).contains("to_json()") {
                continue;
            }
            let number = shipped
                .get(..at)
                .map_or(0, |before| before.matches('\n').count() + 1);
            rendered.push(format!("{where_it_lives}:{number}"));
        }
    }
    assert!(
        rendered.is_empty(),
        "serialised output put out through the door that folds — use `emit!`: \
         {rendered:?}"
    );

    // And the doors themselves: what a parser reads goes out exactly as it was
    // built, on either stream. A failure is output too, and a script that asked for
    // something it could parse asked about those most of all.
    let say = std::fs::read_to_string("src/say.rs").unwrap_or_default();
    for door in ["pub(crate) fn emitted", "pub(crate) fn refused"] {
        let body = body_of(&say, door);
        assert!(!body.is_empty(), "{door} was found");
        assert!(
            !body.contains("rendered") && !body.contains("folded"),
            "{door} does not fold: {body}"
        );
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
    let confined: [(&str, &[&str]); 9] = [
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
        // Nor is this one. Hashing a password is pure, and it is confined for a
        // sharper reason than the rest: a second place that hashed one its own way
        // would be a second set of parameters, and the weaker of the two would be
        // invisible in everything it produced.
        ("argon2", &["admission/credential.rs"]),
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
///
/// Read through the same reader that writes the committed inventory, so that what
/// counts as a declaration is decided once. A second reader would answer this
/// question about a slightly different set of declarations than the one the
/// artefact is built from, and the two would drift without either being wrong.
#[test]
fn no_two_problems_answer_to_the_same_code() {
    let Ok(declared) = lemonfiber::codes::declared(&workspace_root()) else {
        unreachable!("the workspace these tests run in is readable");
    };

    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut collisions: Vec<String> = Vec::new();
    for (path, code) in declared {
        if let Some(first) = seen.insert(code.clone(), path.clone()) {
            collisions.push(format!(
                "{code} in {} and {}",
                first.display(),
                path.display()
            ));
        }
    }
    assert!(collisions.is_empty(), "{collisions:?}");
}

/// Nothing here shapes the machine's traffic, and this is what keeps it that way.
///
/// A bandwidth feature has one obvious cheat in it. The tools that shape a host's
/// traffic are a shell command away, they work on every application at once, and
/// they would make every figure in this product's own report come true without any
/// of the reading back that makes it honest. What they need is a privilege this
/// program should not hold and a reach over software that is none of its business —
/// the browser, the work laptop's backup, somebody's call.
///
/// The rule is that limits are set inside lemonfiber's own download clients through
/// those clients' own interfaces, and nowhere else. That rule is invisible in a
/// diff: a shaper reached from one line of one adapter reads like plumbing and
/// changes what this program *is*. So the names are held against the shipped half of
/// the tree, where anything reaching one would have to appear.
///
/// A word here is refused as a word rather than as a command, so the guard cannot
/// be walked around by building the same call out of pieces — and each is spelled
/// where a shell would have to spell it, with the separators a path or an argument
/// puts around it.
#[test]
fn nothing_shipped_reaches_for_a_traffic_shaper() {
    /// What a host's traffic is shaped with, on the platforms this ships to.
    const SHAPERS: [&str; 6] = [
        "wondershaper",
        "trickle",
        "/sbin/tc",
        "pfctl",
        "dnctl",
        "iptables",
    ];

    let mut reaching: Vec<String> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let shipped = production(&text);
        for shaper in SHAPERS {
            if shipped.contains(shaper) {
                reaching.push(format!("{} names {shaper}", path.display()));
            }
        }
    }
    assert!(
        reaching.is_empty(),
        "limits belong inside lemonfiber's own download clients and nowhere else: {reaching:?}"
    );
}

/// Output leaves through one place, and this is what keeps it that way.
///
/// The funnel was worth building because a question about *how* something is shown
/// had a hundred and six answers, or in practice none. Nothing about its shape stops
/// the hundred and seventh: a bare print added later still compiles, still reads
/// correctly in review, and silently opts that one line out of every rendering
/// decision the funnel makes. The failure it produces is the cruel kind — the line
/// that skipped the fold would be the one still carrying a tick on the terminal that
/// cannot draw one, which is precisely the terminal the fold exists for.
///
/// Two files are allowed to reach a stream directly, for opposite reasons.
/// `src/say.rs` **is** the funnel. `build.rs` is not talking to a person at all: its
/// output is a protocol Cargo parses, and rendering a directive for a human terminal
/// would corrupt it. Everything under a `src/` directory that is not the funnel goes
/// through the funnel.
#[test]
fn nothing_reaches_a_terminal_except_through_the_one_way_out() {
    let mut leaks: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        // `build.rs` sits beside `src/` rather than inside it, so naming the funnel
        // is the whole of the exception list.
        if !where_it_lives.contains("/src/") || where_it_lives.ends_with("src/say.rs") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            // One entry per line rather than one per way out: `eprintln!(` contains
            // `println!(`, and a line reported twice reads as two faults.
            //
            // `prompt_password` is here because it is not a macro and was the second
            // leak found: the password crate writes the prompt itself, so the text
            // has to arrive already folded rather than being handed over raw.
            if [
                "println!(",
                "eprintln!(",
                "print!(",
                "eprint!(",
                "prompt_password(format!",
            ]
            .iter()
            .any(|reaching| trimmed.contains(reaching))
            {
                leaks.push(format!("{where_it_lives}:{}: {trimmed}", number + 1));
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "these reach a terminal without passing the funnel, so nothing decides how \
         they are rendered: {leaks:?}"
    );
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

/// Every diagnostic check is given something to ask.
///
/// A check must establish what it reports rather than infer it from the operator's
/// configuration. The difference matters most exactly where an operator is least able
/// to tell: a check that reads a port number out of the settings and calls it "bound"
/// will pass on a machine where nothing is listening, and it will pass loudest on the
/// machine that is broken.
///
/// Asserted structurally, because it is a structural property. A check that only reads
/// configuration needs nothing but data; one that goes and looks needs a seam to look
/// through, and in this crate that seam is always a trait object — the engine, a
/// runner, an HTTP client, a filesystem, or a narrower port like `Validator` or
/// `UsenetAccounts`.
///
/// Read from what the check is **handed**, not only from what it stores: several keep
/// their ports inside a helper of their own rather than as a field, which is a detail
/// of how they are built rather than of whether they can see.
///
/// This does not prove any particular finding was observed rather than assumed. It
/// proves the check was built able to observe, and it fails the moment somebody adds
/// one that was not — which is the change worth catching, since the finding itself
/// reads the same either way.
#[test]
fn every_check_is_given_something_to_ask() {
    let mut assuming: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, text) in sources() {
        if !path.to_string_lossy().contains("doctor") {
            continue;
        }
        for name in text.lines().filter_map(|line| {
            line.strip_prefix("pub struct ")
                .and_then(|rest| rest.strip_suffix(" {"))
                .filter(|name| name.ends_with("Check"))
        }) {
            seen += 1;
            // What it holds, and what it is handed. Either is a way to go and look;
            // a check with neither can only be repeating the configuration back.
            let fields = text
                .split(&format!("pub struct {name} {{"))
                .nth(1)
                .and_then(|rest| rest.split("\n}").next())
                .unwrap_or_default()
                .to_owned();
            let built = text
                .split(&format!("impl {name} {{"))
                .nth(1)
                .and_then(|rest| rest.split(") -> Self").next())
                .unwrap_or_default()
                .to_owned();

            if !fields.contains("Arc<dyn ") && !built.contains("Arc<dyn ") {
                assuming.push(format!("{} ({name})", path.display()));
            }
        }
    }

    assert!(
        seen > 5,
        "the scan found {seen} checks, which means it is looking in the wrong place"
    );
    assert!(
        assuming.is_empty(),
        "a check with nothing to ask can only be repeating the configuration back: {}",
        assuming.join(", ")
    );
}

/// A check that disturbs the running system says how long it disturbs it for.
///
/// Opting in is a decision an operator makes about a cost, and half the cost is the
/// length of it: a tunnel down for one probe and a tunnel down for ten minutes are
/// different answers to "shall I run this now". The killswitch stated what it disturbed
/// for four releases and never how long, and nothing in the build noticed.
///
/// Read structurally, from the gate rather than from the words: a check that refuses to
/// act unless it was asked for is a check that disturbs something, and it must reach for
/// the one place a length is put into words. A sentence with the number typed into it
/// would pass a reading of the prose and still go stale the day the budget changed.
#[test]
fn every_disturbing_check_says_how_long_it_disturbs_for() {
    let mut silent: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, text) in sources() {
        if !path.to_string_lossy().contains("doctor") {
            continue;
        }
        let shipped = production(&text);
        if !shipped.contains("!self.disruptive") {
            continue;
        }
        seen += 1;
        if !shipped.contains("disturbing_for") {
            silent.push(path.display().to_string());
        }
    }

    assert!(
        seen > 1,
        "the scan found {seen} checks gated on being asked for, which means it is looking \
         in the wrong place"
    );
    assert!(
        silent.is_empty(),
        "a check that disturbs the stack must say how long for: {}",
        silent.join(", ")
    );
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
        let shipped = production(&text).lines().count();
        if shipped > CAP {
            oversized.push((path, shipped));
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

/// The name a `mod NAME {` line declares, where it opens one here.
///
/// Only an inline module counts. A `mod name;` declaration puts the code in another
/// file, which the cap measures on its own, so it is not a point this file stops
/// shipping at — `prompt.rs` and the core's `lib.rs` both declare a test-only module
/// that way and are production all the way down.
fn module_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("pub(crate) ").unwrap_or(trimmed);
    let opened = rest.strip_prefix("mod ")?.trim_end().strip_suffix('{')?;
    let name = opened.trim_end();
    (!name.is_empty()).then_some(name)
}

/// The modules a file declares behind `#[cfg(test)]`, in the order they appear.
fn test_modules(text: &str) -> Vec<&str> {
    text.lines()
        .zip(text.lines().skip(1))
        .filter(|(attribute, _)| attribute.trim() == "#[cfg(test)]")
        .filter_map(|(_, declaration)| module_name(declaration))
        .collect()
}

/// A file that has tests declares them somewhere the line cap can find them.
///
/// The cap tells production from test by looking for a `mod tests`, so a lone test
/// module under any other name leaves the *whole* file counted as shipped. That is
/// not theoretical: `services.rs` measured 538 of 550 while shipping 372 lines,
/// because its tests were `mod telling_tests`. The next person to add a dozen lines
/// of test to it would have been told to split a file that had 178 lines spare —
/// by a cap whose own reason for existing says tests are not rationed.
///
/// Conservative in direction, which is why it went unnoticed: over-counting only
/// ever produces a false red. It still makes the cap arbitrary from file to file,
/// and a guard nobody can predict is one people learn to raise rather than obey.
///
/// A *second* test module beside the first may be named for what it covers —
/// `exit.rs` has `mod reporting` after its `mod tests`, and the cap has already cut
/// by then. Only the declaration it cuts at has to be findable.
#[test]
fn a_file_with_tests_declares_them_where_the_line_cap_looks() {
    let mut unfindable: Vec<String> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let declared = test_modules(&text);
        if declared.is_empty() || declared.contains(&"tests") {
            continue;
        }
        unfindable.push(format!(
            "{} (mod {})",
            path.display(),
            declared.join(", mod ")
        ));
    }
    assert!(
        unfindable.is_empty(),
        "the line cap finds where a file stops shipping by its `mod tests`, so these \
         have their tests counted as production: {}",
        unfindable.join(", ")
    );
}

/// Every door of the funnel treats the text before it puts it out.
///
/// The funnel exists so that a question about how something is shown has one
/// answer. It has two doors, and for a long time only one of them treated what it
/// was given: the person's half made the text plain, and the parser's half printed
/// it exactly as it arrived — on the stated grounds that serialising had already
/// escaped every control character. It escapes the ones below a space, and carries
/// the C1 controls, the line separators, the bidirectional overrides and the
/// zero-widths raw. So `--json` was the one way out a release title could still
/// reach a terminal through intact, and the sentence saying otherwise was the whole
/// reason nobody looked.
///
/// Pinned by shape rather than by output, for the same reason the funnel's other
/// rules are: what this catches is a **third** door added later, printing directly
/// because that is what the two beside it appear to do. Reading the streams back
/// would be a harness, and would say nothing about the door nobody has written yet.
#[test]
fn nothing_leaves_this_binary_without_passing_through_a_treatment() {
    /// What puts a line where somebody can read it.
    const DOORS: [&str; 3] = ["println!", "eprintln!", "print!"];
    /// The two answers to what happens to it first: for a person, for a parser.
    const TREATMENTS: [&str; 2] = ["rendered(", "written("];

    let say = fs::read_to_string("src/say.rs").unwrap_or_default();
    let shipped = production(&say);
    let mut untreated: Vec<&str> = Vec::new();
    let mut doors = 0_usize;
    for line in shipped.lines() {
        let statement = line.trim_start();
        if !DOORS.iter().any(|door| statement.starts_with(door)) {
            continue;
        }
        doors += 1;
        if !TREATMENTS.iter().any(|how| statement.contains(how)) {
            untreated.push(statement);
        }
    }

    // What this is reading, asserted before what it found. A funnel that has been
    // renamed or moved leaves the loop above matching nothing, and a guard that
    // found no doors would pass while watching an empty file — which is the shape
    // this repository has been caught by twice.
    assert!(
        doors >= DOORS.len(),
        "fewer doors than there are ways to print, so this is reading the wrong \
         file: {doors} found"
    );
    assert!(
        untreated.is_empty(),
        "these put text out with neither treatment, so somebody else's control \
         characters reach a terminal through them: {untreated:?}"
    );

    // And the treatments themselves, since a door naming one that does nothing
    // would satisfy the loop above without disarming anything.
    let core = fs::read_to_string("../lemonfiber-core/src/text.rs").unwrap_or_default();
    assert!(
        core.contains("pub fn plain") && core.contains("pub fn escaped"),
        "the two treatments are where the doors reach for them"
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
    let complain = body_of(&exit, "pub(crate) fn complain");
    // The lines themselves are built next door now, so the per-line checks below
    // have to follow them there. A guard that kept reading only the two-line caller
    // would pass on anything.
    let built = body_of(&exit, "\npub(crate) fn reported");
    let reporter = format!("{complain}\n{built}");

    assert!(!complain.is_empty(), "the reporter was found");
    assert!(!built.is_empty(), "and what it builds");
    assert!(
        complain.contains(".eprint()"),
        "it reports something at all"
    );
    assert!(
        !complain.contains(".print()"),
        "a diagnosis on stdout would corrupt a machine-readable run"
    );

    // Output leaves through one place now, so this is pinned where it is decided
    // rather than at each of a hundred call sites: `complain!` is the stderr half
    // of that funnel, and `say!` the stdout half. A reporter reaching for the wrong
    // one is the failure this guards, and it is still one word.
    let say = std::fs::read_to_string("src/say.rs").unwrap_or_default();
    assert!(
        say.contains("fn complained") && say.contains("eprintln!"),
        "the funnel's error half writes to stderr"
    );
    assert!(
        say.contains("fn said") && say.contains("println!"),
        "and its ordinary half writes to stdout"
    );

    for line in reporter.lines() {
        let statement = line.trim_start();
        assert!(
            !statement.starts_with("println!") && !statement.starts_with("say!"),
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

/// Every API a service can declare is one seeding acts on, or one deferred by name.
///
/// A service's `api.kind` is what decides whether its credential is ever read: adding
/// a kind to the manifest without a client to answer it leaves that service declaring
/// an API nothing speaks, and the only symptom is a key that never gets published —
/// which reads downstream as a service with nothing to say.
///
/// The stack keeps its own list of kinds it will accept, so this is the second half of
/// that pair: the stack refuses a kind nothing here implements, and this refuses a kind
/// nothing here acts on. Two lists agreeing is what makes either of them mean anything.
///
/// One kind is deferred rather than missing: the book indexer's wiring waits on a live
/// instance to pin its endpoints against, which is written down in the tracker rather
/// than here. It is named so that the exception is a decision rather than a gap.
#[test]
fn every_api_a_service_can_declare_is_acted_on() {
    /// Declared, and deliberately not wired — see the tracker row for the book indexer.
    const DEFERRED: [&str; 1] = ["Bindery"];

    let schema =
        fs::read_to_string(workspace_root().join("crates/lemonfiber-manifest/src/schema.rs"))
            .unwrap_or_default();
    let Some(block) = schema
        .split_once("pub enum ApiKind {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
    else {
        unreachable!("the manifest declares the API kinds a service may name");
    };
    let declared: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("///") && line.ends_with(','))
        .map(|line| line.trim_end_matches(','))
        .filter(|name| !name.is_empty())
        .collect();
    assert!(
        declared.len() > 3,
        "the API kinds were not read: {declared:?}"
    );

    let acted_on: String = sources()
        .into_iter()
        .filter(|(path, _)| {
            path.to_string_lossy()
                .replace('\\', "/")
                .contains("/src/app")
        })
        .map(|(_, text)| text)
        .collect();

    let ignored: Vec<&str> = declared
        .iter()
        .filter(|kind| !DEFERRED.contains(kind))
        .filter(|kind| !acted_on.contains(&format!("ApiKind::{kind}")))
        .copied()
        .collect();
    assert!(
        ignored.is_empty(),
        "a service declaring one of these names an API nothing acts on, so its \
         credential is never read: {ignored:?}"
    );
}

/// The request service is given an owner before anything is registered into it.
///
/// It refuses every call until it has one, and the step that gives it one is the
/// identity wiring. Registering the \*arrs into it first is not merely early — the
/// service answers "refused the credential", which a seed run reports as two failed
/// connections. It then, further down the same run, sets up the service that had just
/// refused it, so a fresh stack reported a fault it had already fixed by the time
/// anybody read it.
///
/// Pinned by the order the calls appear in, because that is the whole of the
/// dependency: neither step takes anything from the other, so nothing else in the
/// types or the data would notice them being swapped back.
#[test]
fn the_request_service_is_set_up_before_anything_is_registered_into_it() {
    let seed = fs::read_to_string(workspace_root().join("crates/lemonfiber-core/src/app/seed.rs"))
        .unwrap_or_default();
    let shipped = production(&seed);

    let (Some(identity), Some(targets)) = (
        shipped.find("seed_jellyfin_identity("),
        shipped.find("seed_fulfilment_targets("),
    ) else {
        unreachable!("seeding sets up the request service and registers the *arrs into it");
    };
    assert!(
        identity < targets,
        "the *arrs are registered into the request service before it is given an \
         owner, so a fresh stack reports two failures and then fixes them in the \
         same run"
    );
}

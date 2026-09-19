//! Why no Windows binary ships, held where a gate can read it.
//!
//! The reason was a sentence beside the target list in the workspace manifest, and
//! a sentence there is a place nothing checks. Two things could happen quietly: a
//! reason could be fixed and the target stay off, or a second reason could arrive
//! and the sentence go on naming one.
//!
//! So the reasons are a register, and the target list is held to it. A call this
//! names and the source no longer makes is a blocker that has gone — which is news,
//! because it may be the last one. A target added while anything is still named is
//! a build that would not compile.
//!
//! **Neither half is a claim about what the compiler accepts.** Only the compiler
//! knows that, and asking it means a Windows toolchain and a target that does not
//! build yet, which is a gate that is red by design. What this holds is the
//! narrower thing worth holding: that the account of why and the decision it
//! justifies cannot drift apart.

use std::fs;

mod source_tree;

use source_tree::workspace_root;

/// One thing a Windows build would fail on.
struct Blocked {
    /// The file that makes the call.
    at: &'static str,
    /// The call itself.
    call: &'static str,
    /// Why it stops a build, in the words somebody clearing it would need.
    why: &'static str,
}

/// The target a Windows binary would be built for.
const WINDOWS: &str = "x86_64-pc-windows-msvc";

/// Everything a Windows build is waiting on.
///
/// Two kinds, and the difference decides who can clear each. The first two are
/// stable-compiler gaps: the calls exist, and reaching them means a compiler
/// feature nobody outside the toolchain can turn on. The third is this workspace's
/// own: the Windows transport is a named pipe rather than a socket, and nothing
/// here has written that arm yet.
const WAITING: &[Blocked] = &[
    Blocked {
        at: "crates/lemonfiber-adapters/src/filesystem.rs",
        call: "file_index",
        why: "behind an unstable compiler feature, so no stable toolchain has it",
    },
    Blocked {
        at: "crates/lemonfiber-adapters/src/filesystem.rs",
        call: "number_of_links",
        why: "behind the same unstable feature as the index beside it",
    },
    Blocked {
        at: "crates/lemonfiber-adapters/src/docker.rs",
        call: "connect_with_unix",
        why: "the engine is reached over a named pipe on Windows, and that arm is unwritten",
    },
];

/// Every call the register names is one the source still makes.
///
/// The direction that goes unnoticed. A call that has gone is a blocker cleared,
/// and the last one going is the moment the target could be turned on — which
/// nobody finds out about by reading a comment that still says why it is off.
#[test]
fn every_call_windows_waits_on_is_one_this_workspace_still_makes() {
    let root = workspace_root();
    assert!(
        !WAITING.is_empty(),
        "nothing is waiting, so the target below is off for a reason nobody has written down"
    );

    for Blocked { at, call, why } in WAITING {
        let Ok(source) = fs::read_to_string(root.join(at)) else {
            unreachable!("the register names {at}, which this workspace carries");
        };
        assert!(
            source.contains(call),
            "{at} no longer calls `{call}` — it was waiting on it because {why}, so take \
             it off the register and say whether that was the last one"
        );
    }
}

/// The target list and the register agree about whether Windows can be built.
///
/// One decision written in two places, which is the shape that drifts. A target
/// added while anything is still named would produce a build that does not compile;
/// a register emptied while the target is still off is a platform left unshipped
/// for no recorded reason.
#[test]
fn windows_is_a_target_exactly_when_nothing_is_waiting_on_it() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("Cargo.toml")) else {
        unreachable!("the workspace this test is compiled from carries a manifest");
    };
    let Ok(read) = manifest.parse::<toml::Table>() else {
        unreachable!("the workspace manifest parses, or nothing here builds");
    };

    let targets: Vec<String> = read
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("metadata"))
        .and_then(toml::Value::as_table)
        .and_then(|metadata| metadata.get("dist"))
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("targets"))
        .and_then(toml::Value::as_array)
        .map(|named| {
            named
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    assert!(
        targets.len() > 2,
        "the manifest declares {} targets, which means this is reading the wrong table",
        targets.len()
    );

    let built = targets.iter().any(|target| target == WINDOWS);
    let waiting: Vec<String> = WAITING
        .iter()
        .map(|one| format!("  {} calls `{}` — {}", one.at, one.call, one.why))
        .collect();
    let said = waiting.join("\n");

    let declares = if built { "does" } else { "does not" };
    let holds = if waiting.is_empty() {
        "has nothing"
    } else {
        "still has these"
    };
    assert_eq!(
        built,
        waiting.is_empty(),
        "the manifest {declares} build {WINDOWS} and the register {holds} waiting on:\n{said}"
    );
}

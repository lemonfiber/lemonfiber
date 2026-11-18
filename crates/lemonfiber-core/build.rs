//! Lists the release record's files for the crate to compile in, and stamps in the
//! commit the build is made from.
//!
//! The record is a directory with a file per release, and a release adds a file. The
//! list is written here rather than kept in the source, so a file added to the
//! directory is carried by the next build without anyone naming it, and none is
//! carried that the directory does not hold.
//!
//! The commit is the one the workspace's own checkout is at, or the one the CI run
//! building it names where no checkout answers; a build where neither does is made from
//! no commit it can name.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the record is kept, from this crate.
const RECORD: &str = "../../reference/changelog";

/// The workspace root, from this crate.
const WORKSPACE: &str = "../..";

/// The variable a GitHub Actions run names the commit it builds in.
const CI_COMMIT: &str = "GITHUB_SHA";

/// The file beneath this build's output the commit is written to, as Rust.
const STAMPED: &str = "commit.rs";

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    recorded(&manifest, &out);
    stamped(&manifest.join(WORKSPACE), &out);
}

/// List the release record's files into `out`.
fn recorded(manifest: &Path, out: &Path) {
    let record = manifest.join(RECORD);
    println!("cargo::rerun-if-changed={}", record.display());

    let mut names: Vec<String> = std::fs::read_dir(&record)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();

    let mut listed = String::from("&[\n");
    for name in &names {
        let path = record.join(name);
        let _ = writeln!(
            listed,
            "    ({name:?}, include_str!({:?})),",
            path.display().to_string()
        );
    }
    listed.push(']');

    if let Err(error) = std::fs::write(out.join("carried.rs"), listed) {
        println!("cargo::error=could not list the release record: {error}");
    }
}

/// Write the commit the workspace at `workspace` is built from into `out`, built again
/// where the checkout moves or the CI run's commit changes.
fn stamped(workspace: &Path, out: &Path) {
    println!("cargo::rerun-if-env-changed={CI_COMMIT}");
    let commit = checked_out(workspace)
        .or_else(|| std::env::var(CI_COMMIT).ok())
        .filter(|commit| is_commit(commit));
    if let Err(error) = std::fs::write(out.join(STAMPED), format!("{commit:?}")) {
        println!("cargo::error=could not stamp the commit this build is made from: {error}");
    }
}

/// The commit the git checkout at `workspace` is at, where `workspace` is that
/// checkout's root and git answers; each file naming it is watched.
fn checked_out(workspace: &Path) -> Option<String> {
    let answer = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
            "HEAD",
        ])
        .output()
        .ok()
        .filter(|answer| answer.status.success())?;
    let text = String::from_utf8(answer.stdout).ok()?;
    let [top, own, common, commit] = text.lines().collect::<Vec<_>>()[..] else {
        return None;
    };
    if Path::new(top).canonicalize().ok()? != workspace.canonicalize().ok()? {
        return None;
    }
    let own = Path::new(own);
    let common = Path::new(common);
    let head = std::fs::read_to_string(own.join("HEAD")).unwrap_or_default();
    let mut watched = vec![own.join("HEAD"), common.join("packed-refs")];
    if let Some(branch) = head.trim().strip_prefix("ref: ") {
        watched.push(common.join(branch));
    }
    for file in watched.iter().filter(|file| file.exists()) {
        println!("cargo::rerun-if-changed={}", file.display());
    }
    Some(commit.to_owned())
}

/// Whether `commit` is a commit as git names one: forty or sixty-four lowercase hex digits.
fn is_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64)
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

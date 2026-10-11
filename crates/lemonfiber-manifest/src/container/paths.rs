//! Where a mount may come from and where it may land.

use super::DATA;

/// What is wrong with `path` as a target inside a container, if anything.
pub(super) fn target(path: &str, takes_data: bool) -> Option<&'static str> {
    if !path.starts_with('/') || path == "/" || path.contains("..") || !is_plain(path) {
        return Some(
            "is not one plain absolute path: not the root, with no \"..\", in letters, digits \
             and \"._/-\" alone",
        );
    }
    let in_data = path == DATA || path.starts_with(&format!("{DATA}/"));
    (takes_data && in_data).then_some("is inside the data mount")
}

/// Whether `path` is a target inside a container a service may mount at: one plain
/// absolute path, not the root, and outside the data mount where it takes data.
#[must_use]
pub fn is_target(path: &str, takes_data: bool) -> bool {
    target(path, takes_data).is_none()
}

/// Whether the source `path` lies inside the source `other`, and is not it.
#[must_use]
pub fn lies_inside(path: &str, other: &str) -> bool {
    path.len() > other.len()
        && path.starts_with(other)
        && path.as_bytes().get(other.len()) == Some(&b'/')
}

/// Whether `path` is a mount source: a plain relative path inside the service's own mount
/// tree, with no empty, `.` or `..` step.
#[must_use]
pub fn is_source(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && is_plain(path)
        && path
            .split('/')
            .all(|step| !step.is_empty() && step != "." && step != "..")
}

/// Whether `text` is written only in letters, digits, `.`, `_`, `/` and `-`.
#[must_use]
pub fn is_plain(text: &str) -> bool {
    text.chars()
        .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '.' | '_' | '/' | '-'))
}

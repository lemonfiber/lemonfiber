//! The upstream image a service says its image is built on.

use crate::schema::Service;
use crate::Violation;

/// Whether a service naming the upstream image it is built on names a registry path with
/// no tag or digest on it, and a release with no space in it.
pub(super) fn built(service: &Service, found: &mut Vec<Violation>) {
    let Some(built_on) = &service.built_on else {
        return;
    };
    let refused = |field: &str, message: String| Violation {
        location: format!("service {}.built_on.{field}", service.id),
        message,
    };
    let image = built_on.image.as_str();
    if image.trim().is_empty() {
        found.push(refused(
            "image",
            "is blank: name the upstream image the build starts from".to_owned(),
        ));
    } else if image.contains('@') || image.rsplit('/').next().is_some_and(super::has_tag) {
        found.push(refused(
            "image",
            format!("{image} carries its own tag or digest; the release names which one"),
        ));
    }
    let release = built_on.release.as_str();
    if release.is_empty() || release.contains(char::is_whitespace) {
        found.push(refused(
            "release",
            format!("{release:?} is not one tag: name the upstream release the build starts from"),
        ));
    }
}

#[cfg(test)]
mod tests;

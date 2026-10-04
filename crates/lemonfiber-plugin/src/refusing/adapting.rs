//! What a plugin's service has to say beside the adapter it names.

use crate::schema::Service;
use crate::Violation;

/// Whether an adapter the service names can reach it.
///
/// The adapter is lemonfiber's and the set is closed, which the schema holds. What the
/// schema cannot hold is the rest of the declaration it needs: where inside the stack's
/// network the service answers, and which version of the shared \*arr shape it speaks.
pub(super) fn adapted(service: &Service, found: &mut Vec<Violation>) {
    let Some(api) = &service.api else {
        return;
    };
    if service.listens.is_none() {
        found.push(Violation {
            location: format!("service {}.listens", service.id),
            message: "is absent, and a service naming an adapter has to say the port it answers \
                      on inside the stack's network, or nothing could reach it there"
                .to_owned(),
        });
    }
    if let Some(path) = api.path.as_deref().filter(|path| !beneath(path, service)) {
        found.push(Violation {
            location: format!("service {}.api.path", service.id),
            message: format!(
                "{path} is not one file beneath {}; what is permitted is an absolute path \
                 inside the service's configuration directory, with no `..`, no empty or `.` \
                 segment, written in letters, digits and `._/-` alone",
                service.configuration()
            ),
        });
    }
    if api.kind == lemonfiber_manifest::ApiKind::Servarr && api.version.is_none() {
        found.push(Violation {
            location: format!("service {}.api.version", service.id),
            message: "is absent, and the servarr shape spans two versions of its API, so which \
                      one this service speaks has to be said"
                .to_owned(),
        });
    }
}

/// Whether `path` names something strictly beneath the service's configuration directory.
///
/// lemonfiber reads the credential the adapter names from that directory on the host, and
/// hands it to the service as its own. A path that climbed out of it, or named it rather
/// than a file in it, would have lemonfiber read whatever else the host keeps and send
/// it to a stranger's container.
fn beneath(path: &str, service: &Service) -> bool {
    let directory = service.configuration().trim_end_matches('/');
    let Some(inside) = path
        .strip_prefix(directory)
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    super::carried::is_directory(path)
        && inside
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

#[cfg(test)]
mod tests;

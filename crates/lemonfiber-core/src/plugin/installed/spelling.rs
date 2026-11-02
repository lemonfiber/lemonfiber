//! Two plugins' services that would be one container under one name.

use super::Installed;

/// A service of `would` named, once spelled as an environment name, as a service of
/// another installed plugin already is: the one it would bring, the one already there,
/// and whose that is.
///
/// lemonfiber writes a plugin's container under its service's id and keeps what the
/// service holds in settings named after it, so two such services would be one container
/// and one credential. The same plugin's own record is not another plugin's, so an update
/// replacing a version is not held to the names of the version it replaces.
#[must_use]
pub fn spelled_alike(
    would: &Installed,
    installed: &[Installed],
) -> Option<(String, String, String)> {
    for other in installed
        .iter()
        .filter(|other| other.plugin != would.plugin)
    {
        for theirs in &other.services {
            let spelled = lemonfiber_manifest::environment_name(&theirs.service);
            let ours = would
                .services
                .iter()
                .find(|ours| lemonfiber_manifest::environment_name(&ours.service) == spelled);
            if let Some(ours) = ours {
                return Some((
                    ours.service.clone(),
                    theirs.service.clone(),
                    other.plugin.clone(),
                ));
            }
        }
    }
    None
}

use super::published_as;

/// The name is the service's own, upper-cased.
#[test]
fn a_key_is_published_under_the_service_it_came_from() {
    assert_eq!(published_as("sonarr"), "SONARR_API_KEY");
    assert_eq!(published_as("sabnzbd"), "SABNZBD_API_KEY");
}

/// A hyphen is not something an environment name may carry.
///
/// Service ids are Compose names and several of them are hyphenated. A shell
/// reads a hyphen as an operator, so a name built straight from the id would be
/// one nothing could reference.
#[test]
fn a_hyphenated_service_is_published_under_a_name_a_shell_can_read() {
    assert_eq!(
        published_as("calibre-web-automated"),
        "CALIBRE_WEB_AUTOMATED_API_KEY"
    );
}

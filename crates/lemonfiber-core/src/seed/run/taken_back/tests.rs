use super::not_given;
use crate::credential::Propagation;

/// Only a consumer the replacement could not reach is said, with why.
#[test]
fn only_a_consumer_it_could_not_reach_is_said() {
    let said = not_given(&[
        Propagation::updated("Seerr"),
        Propagation::pending("Bazarr", "at its next restart"),
        Propagation::failed("Sonarr", "it refused"),
    ]);
    assert_eq!(said, ["Sonarr could not be given it: it refused."]);
}

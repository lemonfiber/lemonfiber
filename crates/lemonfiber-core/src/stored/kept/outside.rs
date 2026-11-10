//! What is on this machine that lemonfiber neither keeps nor removes.

use crate::stored::Beside;

/// What is on this machine that lemonfiber neither keeps nor removes.
///
/// An operator reading a list of what is stored is owed the reason a thing is absent
/// from it as much as they are owed the entries — and the library is the absence
/// that would otherwise read as an oversight.
#[must_use]
pub fn beside() -> Vec<Beside> {
    vec![
        Beside {
            what: "your library and your downloads".to_owned(),
            why: "Written by the services under the path you chose, and yours. lemonfiber never \
                  reads what is in them and never removes them."
                .to_owned(),
        },
        Beside {
            what: "the containers, images and volumes".to_owned(),
            why: "The container engine's. lemonfiber starts and stops them and owns none of \
                  them; docker is what lists and removes them."
                .to_owned(),
        },
        Beside {
            what: "anything you installed yourself".to_owned(),
            why: "Docker, a media server running natively, a tunnel client. lemonfiber did not \
                  put them there and does not take them away."
                .to_owned(),
        },
    ]
}

//! The ports services publish on the host, and which of them two services publish.

use super::Publish;

/// Each port a service publishes on the host: its `port`, then each further one.
pub fn published(port: Option<u16>, publish: &[Publish]) -> impl Iterator<Item = u16> + '_ {
    port.into_iter()
        .chain(publish.iter().map(|further| further.port))
}

/// Every port two of `services` publish, each as the port, the service that publishes it
/// first and the service that publishes it again.
#[must_use]
pub fn doubled<'a, P: IntoIterator<Item = u16>>(
    services: impl IntoIterator<Item = (&'a str, P)>,
) -> Vec<(u16, &'a str, &'a str)> {
    let mut held: Vec<(u16, &'a str)> = Vec::new();
    let mut found = Vec::new();
    for (service, ports) in services {
        for port in ports {
            match held.iter().find(|(taken, _)| *taken == port) {
                Some((_, first)) if *first != service => found.push((port, *first, service)),
                Some(_) => {}
                None => held.push((port, service)),
            }
        }
    }
    found
}

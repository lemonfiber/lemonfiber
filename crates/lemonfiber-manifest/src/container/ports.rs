//! The ports services publish on the host, and which of them two services publish.

use super::Publish;

/// How something other than lemonfiber on this machine's loopback reaches a port inside a
/// service's container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The port its `port` publishes on its tier.
    Port,
    /// The port a `[[service.publish]]` entry publishes on its tier.
    Publish,
    /// The proxy's route to it, at the number its `port` declares.
    Route,
    /// Every service asking for what it fills, at where it `listens`.
    Ask,
}

impl Reach {
    /// The field that makes it reach, as a refusal names it.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Port | Self::Route => "port",
            Self::Publish => "publish",
            Self::Ask => "listens",
        }
    }

    /// What reaches the port, as a refusal says it.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::Port => "the port it publishes",
            Self::Publish => "a further port it publishes",
            Self::Route => "the proxy's route to it",
            Self::Ask => "every service that asks for what it fills",
        }
    }
}

/// Where inside its container a published port lands: where it says it listens, or the
/// same number.
#[must_use]
pub const fn inside(port: u16, listens: Option<u16>) -> u16 {
    match listens {
        Some(listening) => listening,
        None => port,
    }
}

/// Every port inside a service's container something but lemonfiber on loopback reaches,
/// with how: what its `port` publishes, what each further port publishes, the proxy's
/// route at its `port`, and every asking service at where it `listens`. Every one is TCP,
/// and every one a single port: a manifest declares no other protocol and no range.
#[must_use]
pub fn reached(port: Option<u16>, listens: Option<u16>, publish: &[Publish]) -> Vec<(Reach, u16)> {
    port.map(|port| (Reach::Port, inside(port, listens)))
        .into_iter()
        .chain(
            publish
                .iter()
                .map(|further| (Reach::Publish, inside(further.port, further.listens))),
        )
        .chain(port.map(|port| (Reach::Route, port)))
        .chain(listens.map(|listening| (Reach::Ask, listening)))
        .collect()
}

/// Whether what `how` reaches at `at` inside the container is the port a service speaks
/// its contracts on, `contract`: an ask at where it listens is not, where its contracts
/// are spoken there too, because the services that ask reach it there by design.
#[must_use]
pub fn exposes(how: Reach, at: u16, contract: Option<u16>, listens: Option<u16>) -> bool {
    contract.is_some_and(|spoken| spoken == at && !(how == Reach::Ask && listens == Some(spoken)))
}

/// The first way something but lemonfiber on loopback reaches `contract`, the port a
/// service speaks its contracts on, where anything does.
#[must_use]
pub fn exposed(
    contract: u16,
    port: Option<u16>,
    listens: Option<u16>,
    publish: &[Publish],
) -> Option<Reach> {
    reached(port, listens, publish)
        .into_iter()
        .find(|(how, at)| exposes(*how, *at, Some(contract), listens))
        .map(|(how, _)| how)
}

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

#[cfg(test)]
mod tests;

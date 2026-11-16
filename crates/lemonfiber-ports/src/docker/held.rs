//! What the engine holds beyond the containers it runs: the images it pulled, the
//! networks and volumes a project left it.

use async_trait::async_trait;

use super::Failure;

/// One image this machine has pulled, and what is standing on it.
///
/// The projects travel with it because whether an image may be removed is not a
/// property of the image: one that another Compose project's container is built on
/// belongs to that project as much as to this one, and nothing downstream can
/// establish that from a name and a size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Every name it answers to: its tags, its digests, and each name a container
    /// was started from it by.
    ///
    /// All three, because an image pulled by digest carries no tag — the version it
    /// was pinned at survives only in the name its container was started from.
    /// Empty for an image nothing names any more, which is still an image taking up
    /// room and so still worth reporting.
    pub tags: Vec<String>,
    /// What it occupies, as the engine reports its size.
    pub bytes: u64,
    /// The Compose projects whose containers are built on it.
    ///
    /// A container the engine reports under no Compose project contributes an empty
    /// entry, because for the purposes of removal "something outside Compose is
    /// standing on this" is the same answer as "another project is".
    pub projects: Vec<String>,
}

/// Listing the images an engine has pulled, and who is standing on each.
///
/// A trait of its own rather than another method on [`Engine`](super::Engine), for the reason the
/// volume watch and the eraser are apart from the filesystem: the one command that
/// asks needs nothing else of an engine, and every other implementation of the wider
/// trait — five of them are test fakes — would gain a method it never calls.
#[async_trait]
pub trait Images: Send + Sync {
    /// Every image on this machine, with the projects holding containers on it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached, which the
    /// caller reports as an unknown rather than as no images.
    async fn images(&self) -> Result<Vec<Image>, Failure>;

    /// The engine's id for the image `reference` names on this machine, or none where it
    /// names none here.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn identified(&self, reference: &str) -> Result<Option<String>, Failure>;
}

/// What a network is made of: the addresses it holds and whether anything leaves it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shape {
    /// Its subnet, as an address and a prefix length joined by `/`, where it names one.
    pub subnet: Option<String>,
    /// The part of the subnet the engine hands out, written the same way, where it is
    /// narrower than the subnet.
    pub ip_range: Option<String>,
    /// Whether it has no route off the host.
    pub internal: bool,
    /// Whether it translates addresses off the host.
    pub masquerade: bool,
}

/// One network of a project, as the engine holds it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Network {
    /// The engine's name for it.
    pub name: String,
    /// The name the project declares it under.
    pub declared: String,
    /// The engine's id for it, which changes each time it is created again.
    pub id: String,
    /// What it is made of.
    pub shape: Shape,
    /// The address the engine routes it through, where it names one.
    pub gateway: Option<String>,
}

/// What a network is created as: its name, the project and the name the project declares
/// it under, which it is labelled with, and its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted<'a> {
    /// The engine's name for it.
    pub name: &'a str,
    /// The project it is labelled with.
    pub project: &'a str,
    /// The name the project declares it under, which it is labelled with.
    pub declared: &'a str,
    /// What it is made of.
    pub shape: &'a Shape,
}

/// A container joining a network: the names it answers to there beyond its own service's,
/// and the address it holds there, where fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Joining<'a> {
    /// The engine's name for the network.
    pub network: &'a str,
    /// The engine's id for the container.
    pub container: &'a str,
    /// The names it answers to there.
    pub aliases: &'a [String],
    /// Its fixed address there.
    pub address: Option<&'a str>,
}

/// How a container is on one network: the address it was given there, where fixed, and the
/// names it was given there apart from its own name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Endpoint {
    /// The engine's name for the network.
    pub network: String,
    /// Its fixed address there.
    pub address: Option<String>,
    /// The names it answers to there, apart from its own name.
    pub aliases: Vec<String>,
}

/// Creating, listing and taking away the networks of a project, and connecting a container to
/// one or disconnecting it.
///
/// A trait of its own for the reason [`Images`] is: the steps before a start and after a
/// teardown ask it, and every other implementation of the wider trait would gain methods
/// it never calls.
#[async_trait]
pub trait Networks: Send + Sync {
    /// Every network the engine holds labelled with `project`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn networks(&self, project: &str) -> Result<Vec<Network>, Failure>;

    /// Create the network `wanted` names, labelled with its project and declared name.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn created(&self, wanted: Wanted<'_>) -> Result<(), Failure>;

    /// Disconnect every container still on the network named `name`, then remove it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn removed(&self, name: &str) -> Result<(), Failure>;

    /// Connect the container `joining` names to its network, answering to its names and
    /// holding its address there.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn connected(&self, joining: Joining<'_>) -> Result<(), Failure>;

    /// Disconnect the container `container` from the network named `network`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn disconnected(&self, network: &str, container: &str) -> Result<(), Failure>;

    /// How the container `container` is on each network it is on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn endpoints(&self, container: &str) -> Result<Vec<Endpoint>, Failure>;
}

/// Reading the volumes the engine keeps on a service's behalf, and copying one out.
///
/// A trait of its own for the reason [`Networks`] is: one step asks it, before a start,
/// and nothing else does.
#[async_trait]
pub trait EngineVolumes: Send + Sync {
    /// Whether the engine keeps a volume named `name`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn kept(&self, name: &str) -> Result<bool, Failure>;

    /// Copy every plain file and directory the volume named `name` holds into `into`, a
    /// directory beneath `within` reached through no link, through a container of `image`
    /// that is created to reach the volume and never started. The volume is left as it
    /// was.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses, and
    /// where what it hands over cannot be written beneath `within`.
    async fn copied(
        &self,
        name: &str,
        image: &str,
        into: &std::path::Path,
        within: &std::path::Path,
    ) -> Result<(), Failure>;
}

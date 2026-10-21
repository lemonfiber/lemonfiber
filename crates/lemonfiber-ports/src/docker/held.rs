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
}

/// One network a Compose project made, as the engine holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    /// The engine's name for it.
    pub name: String,
    /// The name the project's Compose file declares it under.
    pub declared: String,
}

/// Listing the networks a Compose project made, and taking one away.
///
/// A trait of its own for the reason [`Images`] is: one step asks it, before a start,
/// and every other implementation of the wider trait would gain methods it never calls.
#[async_trait]
pub trait Networks: Send + Sync {
    /// Every network the engine holds for `project`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn networks(&self, project: &str) -> Result<Vec<Network>, Failure>;

    /// Disconnect every container still on the network named `name`, then remove it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached or refuses.
    async fn removed(&self, name: &str) -> Result<(), Failure>;
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

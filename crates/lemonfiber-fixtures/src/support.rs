//! The seams a test stands in for, beyond the transport and the filesystem.
//!
//! A runner that answers from a script, an engine that reports what a test put in it, a
//! filesystem that hands back the configuration a service would have written. Lifted out of
//! `lemonfiber-core`, where being private to the crate meant its integration tests could
//! not see them.

mod reporting;
mod running;

pub use reporting::{Question, Reporting, Tunnel};
pub use running::{refused, spoke, Keyed, Recording, Scripted, Sequenced};

use std::sync::Arc;

use crate::http::{Answer, Fake};
use async_trait::async_trait;

/// A randomness source answering with exactly the bytes a test scripts.
pub struct FixedRandom(pub Option<Vec<u8>>);

impl lemonfiber_ports::random::Random for FixedRandom {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

/// The routes a whole seed run reads where a test answers none of them itself: every call
/// taken, so a run's many calls need no exact ordering.
#[must_use]
pub fn seeding_routes() -> Vec<(&'static str, Answer)> {
    vec![("", Answer::reply(200, "Ok."))]
}

/// A transport answering a whole seed run, every call taken.
#[must_use]
pub fn seeding() -> Arc<Fake> {
    Fake::by_path(seeding_routes())
}

/// A seed run answering `extra` first and falling through to the ordinary routes.
///
/// For the tests that need one service to say something of its own without restating
/// the rest of the run around it.
#[must_use]
pub fn seeding_with(extra: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend(seeding_routes());
    Fake::by_path(routes)
}

/// A filesystem that hands back a Servarr configuration for a Servarr path, or nothing.
/// Only `read` is meaningful to seeding; the rest are unused.
pub struct SeedFs {
    servarr: Option<&'static str>,
    /// Path fragments a canonicalize should fail for, standing in for host
    /// directories that are not there — so a root folder's existence check can be
    /// driven to missing. Every path resolves when empty.
    missing: Vec<&'static str>,
    /// Path fragments a canonicalize resolves somewhere else entirely, standing in for
    /// a link a container put where its file was expected.
    leading_away: Vec<&'static str>,
    /// What a volume describe reports — a zero total by default, which the
    /// dashboard reads as free space unknown.
    facts: lemonfiber_ports::filesystem::StorageFacts,
    elsewhere: Vec<(&'static str, lemonfiber_ports::filesystem::StorageFacts)>,
    wrote: std::sync::Mutex<Vec<(std::path::PathBuf, String)>>,
}

impl SeedFs {
    /// A filesystem holding `servarr` as every Servarr service's configuration.
    #[must_use]
    pub fn keyed(servarr: Option<&'static str>) -> Self {
        Self {
            servarr,
            missing: Vec::new(),
            leading_away: Vec::new(),
            elsewhere: Vec::new(),
            wrote: std::sync::Mutex::new(Vec::new()),
            facts: lemonfiber_ports::filesystem::StorageFacts {
                point: std::path::PathBuf::new(),
                kind: lemonfiber_ports::filesystem::FsKind::Linking("test".to_owned()),
                removable: false,
                available: 0,
                total: 0,
            },
        }
    }

    /// The same, reporting the given volume facts to a describe.
    #[must_use]
    pub fn with_facts(mut self, facts: lemonfiber_ports::filesystem::StorageFacts) -> Self {
        self.facts = facts;
        self
    }

    /// Every file written through this filesystem, in the order it was written.
    ///
    /// Recorded rather than discarded, because a claim about what a command *wrote*
    /// cannot be made from what it returned: a report saying a file was written and a
    /// file having been written are two facts, and the gap between them is exactly the
    /// bug worth catching.
    #[must_use]
    pub fn wrote(&self) -> Vec<(std::path::PathBuf, String)> {
        self.wrote
            .lock()
            .map(|wrote| wrote.clone())
            .unwrap_or_default()
    }

    /// The same, answering differently for paths under a named prefix.
    ///
    /// A fake that answered the same for every path could not be asked whether two
    /// paths sit on one filesystem, which is the whole of the hardlink question — so a
    /// test about a layout that breaks hardlinks would pass against a fake that has
    /// only ever had one.
    #[must_use]
    pub fn with_facts_under(
        mut self,
        prefix: &'static str,
        facts: lemonfiber_ports::filesystem::StorageFacts,
    ) -> Self {
        self.elsewhere.push((prefix, facts));
        self
    }

    /// The same, but failing to canonicalize any path carrying one of these
    /// fragments — a host directory that is not there, for a root-folder check.
    #[must_use]
    pub fn missing(mut self, fragments: Vec<&'static str>) -> Self {
        self.missing = fragments;
        self
    }

    /// The same, resolving every path holding one of `fragments` to somewhere outside
    /// any directory a test names — the shape of a link put where a file was expected.
    #[must_use]
    pub fn leading_away(mut self, fragments: Vec<&'static str>) -> Self {
        self.leading_away = fragments;
        self
    }
}

#[async_trait]
impl lemonfiber_ports::filesystem::FileSystem for SeedFs {
    async fn canonicalize(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, lemonfiber_ports::filesystem::Fault> {
        resolved(self, path)
    }
    async fn touch(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn link(
        &self,
        _from: &std::path::Path,
        _to: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn identify(
        &self,
        _path: &std::path::Path,
    ) -> Result<lemonfiber_ports::filesystem::Identity, lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn remove(&self, _path: &std::path::Path) {}
    async fn read(&self, path: &std::path::Path) -> Option<String> {
        keyed(self, path)
    }
    async fn write(&self, path: &std::path::Path, contents: &str) {
        crate::noted(&self.wrote, (path.to_path_buf(), contents.to_owned()));
    }
    async fn ownership(
        &self,
        _path: &std::path::Path,
    ) -> Option<lemonfiber_ports::filesystem::Ownership> {
        None
    }
}

#[async_trait]
impl lemonfiber_ports::filesystem::Storage for SeedFs {
    async fn describe(&self, path: &std::path::Path) -> lemonfiber_ports::filesystem::StorageFacts {
        let named = path.to_string_lossy().into_owned();
        self.elsewhere
            .iter()
            .find(|(prefix, _)| named.starts_with(prefix))
            .map_or_else(|| self.facts.clone(), |(_, facts)| facts.clone())
    }
}

pub use crate::passwords::a_password;

/// What this fixture holds at every path.
fn keyed(seed: &SeedFs, _path: &std::path::Path) -> Option<String> {
    seed.servarr.map(str::to_owned)
}

fn resolved(
    seed: &SeedFs,
    path: &std::path::Path,
) -> Result<std::path::PathBuf, lemonfiber_ports::filesystem::Fault> {
    let text = path.to_string_lossy();
    if seed.missing.iter().any(|fragment| text.contains(fragment)) {
        return Err(lemonfiber_ports::filesystem::Fault::new("no such path"));
    }
    if seed
        .leading_away
        .iter()
        .any(|fragment| text.contains(fragment))
    {
        return Ok(std::path::PathBuf::from("/etc/elsewhere"));
    }
    Ok(path.to_path_buf())
}

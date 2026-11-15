//! A confined disk holding a key for each of its services, over another disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_ports::filesystem::{Beneath, Confined, Fault, Lying, Readable};

/// A disk holding one key for each of its services, at the same file under each
/// service's directory wherever that is, and reading every other file as `inner` does:
/// for a stack whose directory a test cannot write.
pub struct Keyed {
    inner: Arc<dyn Confined>,
    held: Vec<PathBuf>,
    key: String,
}

impl Keyed {
    /// `inner`, holding `key` at `file` under the directory of each of `services`.
    #[must_use]
    pub fn holding(
        inner: Arc<dyn Confined>,
        services: &[&str],
        file: &str,
        key: &str,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner,
            held: services
                .iter()
                .map(|service| Path::new(service).join(file))
                .collect(),
            key: key.to_owned(),
        })
    }
}

impl Confined for Keyed {
    fn read(&self, path: &Path, within: &Path) -> Beneath {
        if self.held.iter().any(|held| path.ends_with(held)) {
            return Beneath::Read(self.key.clone());
        }
        self.inner.read(path, within)
    }

    fn overwrite(&self, path: &Path, within: &Path, contents: &[u8]) -> Result<(), Fault> {
        self.inner.overwrite(path, within, contents)
    }

    fn written(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        self.inner.written(path, within, contents, readable)
    }

    fn lies(&self, path: &Path, within: &Path) -> Lying {
        self.inner.lies(path, within)
    }

    fn moved(&self, from: &Path, to: &Path, within: &Path) -> Result<(), Fault> {
        self.inner.moved(from, to, within)
    }

    fn made(&self, path: &Path, within: &Path, readable: Readable) -> Result<(), Fault> {
        self.inner.made(path, within, readable)
    }

    fn replaced(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        self.inner.replaced(path, within, contents, readable)
    }

    fn bytes(&self, path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault> {
        self.inner.bytes(path, within)
    }

    fn removed(&self, path: &Path, within: &Path) -> Result<(), Fault> {
        self.inner.removed(path, within)
    }
}

//! The volumes a test says the engine keeps, and which of them a run copied out.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::docker::{EngineVolumes, Failure};

/// One volume: its name, and each file it holds with what the file holds.
type Held = (String, Vec<(String, String)>);

/// An engine keeping exactly these volumes, recording each one copied out.
pub struct Volumed {
    held: Vec<Held>,
    refusing: bool,
    silent: bool,
    copied: Mutex<Vec<(String, String)>>,
}

impl Volumed {
    /// An engine keeping no volume.
    #[must_use]
    pub fn none() -> Arc<Self> {
        Self::holding(&[])
    }

    /// An engine keeping each named volume, holding these files.
    #[must_use]
    pub fn holding(held: &[(&str, &[(&str, &str)])]) -> Arc<Self> {
        Arc::new(Self {
            held: held
                .iter()
                .map(|(name, files)| {
                    (
                        (*name).to_owned(),
                        files
                            .iter()
                            .map(|(file, text)| ((*file).to_owned(), (*text).to_owned()))
                            .collect(),
                    )
                })
                .collect(),
            refusing: false,
            silent: false,
            copied: Mutex::new(Vec::new()),
        })
    }

    /// An engine keeping each named volume and refusing to hand any of them over.
    #[must_use]
    pub fn refusing(names: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            held: names
                .iter()
                .map(|name| ((*name).to_owned(), Vec::new()))
                .collect(),
            refusing: true,
            silent: false,
            copied: Mutex::new(Vec::new()),
        })
    }

    /// An engine that cannot be reached to say what it keeps.
    #[must_use]
    pub fn silent() -> Arc<Self> {
        Arc::new(Self {
            held: Vec::new(),
            refusing: false,
            silent: true,
            copied: Mutex::new(Vec::new()),
        })
    }

    /// Every volume copied out, with the image it was reached through, in order.
    #[must_use]
    pub fn handed_over(&self) -> Vec<(String, String)> {
        self.copied
            .lock()
            .map(|copied| copied.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl EngineVolumes for Volumed {
    async fn kept(&self, name: &str) -> Result<bool, Failure> {
        self.keeps(name)
    }

    async fn copied(
        &self,
        name: &str,
        image: &str,
        into: &Path,
        _within: &Path,
    ) -> Result<(), Failure> {
        self.handing_over(name, image, into)
    }
}

impl Volumed {
    /// Whether the volume `name` is kept, unless this engine cannot be reached.
    fn keeps(&self, name: &str) -> Result<bool, Failure> {
        if self.silent {
            return Err(Failure::Unreachable {
                reason: "the engine did not answer".to_owned(),
            });
        }
        Ok(self.held.iter().any(|(held, _)| held == name))
    }

    /// Write each file the volume `name` holds into `into`, unless this engine refuses.
    fn handing_over(&self, name: &str, image: &str, into: &Path) -> Result<(), Failure> {
        if self.refusing {
            return Err(failed(format!("{name} was not handed over")));
        }
        for (file, text) in self
            .held
            .iter()
            .filter(|(held, _)| held == name)
            .flat_map(|(_, files)| files)
        {
            let at = into.join(file);
            at.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&at, text))
                .map_err(|why| failed(why.to_string()))?;
        }
        let _ = self
            .copied
            .lock()
            .map(|mut copied| copied.push((name.to_owned(), image.to_owned())));
        Ok(())
    }
}

/// What an engine that would not hand a volume over answers, saying `why`.
fn failed(why: String) -> Failure {
    Failure::Unreachable { reason: why }
}

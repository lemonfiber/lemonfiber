use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::undoing::Undone;
use crate::config::env::is_one_line;
use crate::config::store::Failure;
use crate::ports::random::Random;

pub(crate) mod keyed;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Secrets {
    #[serde(default)]
    held: BTreeMap<String, Kept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    value: String,
    from: Source,
    at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Source {
    Operator,
    Recipe,
    Minted,
}

impl Source {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Recipe => "recipe",
            Self::Minted => "minted",
        }
    }
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_set().entries(self.held.keys()).finish()
    }
}

impl Secrets {
    pub(crate) fn at(path: &Path) -> Result<Self, Failure> {
        super::kept::read(path)
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.held.get(name).map(|kept| kept.value.as_str())
    }

    /// The name of every secret held.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.held.keys().map(String::as_str)
    }

    /// Every value held, and every value each replaced.
    pub(crate) fn values(&self) -> impl Iterator<Item = &str> {
        self.held
            .values()
            .flat_map(|kept| std::iter::once(kept.value.as_str()).chain(kept.previous.as_deref()))
    }

    pub(crate) fn source(&self, name: &str) -> Option<Source> {
        self.held.get(name).map(|kept| kept.from)
    }

    /// The keyed fingerprint of what the store holds under `name`.
    pub(crate) fn fingerprint(&self, name: &str) -> Option<String> {
        self.value(name).and_then(|value| self.keyed(value))
    }

    /// The keyed fingerprint of `value` under this store's key, or nothing where the store
    /// has no key yet.
    pub(crate) fn keyed(&self, value: &str) -> Option<String> {
        self.key
            .as_deref()
            .map(|key| keyed::fingerprint(key, value))
    }

    /// `recorded`, a fingerprint an earlier release wrote unkeyed for `name`, keyed from
    /// the value or the earlier value the store holds that it fingerprints, or the mark no
    /// value matches where neither does.
    pub(crate) fn rekeyed(&self, name: &str, recorded: &str) -> String {
        if keyed::is_keyed(recorded) {
            return recorded.to_owned();
        }
        self.held
            .get(name)
            .and_then(|kept| {
                std::iter::once(kept.value.as_str())
                    .chain(kept.previous.as_deref())
                    .find(|held| crate::credential::fingerprint(held) == recorded)
            })
            .and_then(|held| self.keyed(held))
            .unwrap_or_else(|| keyed::UNMATCHED.to_owned())
    }

    pub(crate) fn entered_at(&self, name: &str) -> Option<u64> {
        self.held.get(name).map(|kept| kept.at)
    }

    fn keep(&mut self, name: &str, value: &str, from: Source, at: u64) {
        let previous = self.held.remove(name).map(|kept| kept.value);
        self.held.insert(
            name.to_owned(),
            Kept {
                value: value.to_owned(),
                from,
                at,
                previous,
            },
        );
    }

    fn reheld(&mut self, name: &str, previous: Option<&str>, wrote: Option<&str>) -> Undone {
        let holds = self.fingerprint(name);
        if holds.as_deref() == previous {
            return Undone::Done;
        }
        if holds.as_deref() != wrote {
            return Undone::TheirsNow;
        }
        let Some(previous) = previous else {
            self.held.remove(name);
            return Undone::Done;
        };
        let earlier = self
            .held
            .get(name)
            .and_then(|kept| kept.previous.as_deref())
            .and_then(|held| self.keyed(held));
        match self.held.get_mut(name) {
            Some(kept) if earlier.as_deref() == Some(previous) => {
                let earlier = kept.previous.take().unwrap_or_default();
                kept.previous = Some(std::mem::replace(&mut kept.value, earlier));
                Undone::Done
            }
            _ => Undone::Unkept,
        }
    }
}

/// The secret store at `path`, given a key where it has none yet, so every fingerprint
/// taken of what it holds or will hold is keyed.
///
/// # Errors
///
/// Where the store cannot be read, or the key cannot be written into it.
pub(crate) fn keyed_at(path: &Path, random: &dyn Random) -> Result<Secrets, Failure> {
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        if secrets.key.is_none() {
            secrets.key = keyed::minted(random);
        }
        Ok(())
    })?;
    Secrets::at(path)
}

pub(crate) struct Keeping<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: &'a str,
    pub(crate) from: Source,
    pub(crate) at: u64,
}

/// A secret about to be kept, by its name and the keyed fingerprints of the value it
/// replaces and of itself.
pub(crate) struct Replacing {
    pub(crate) name: String,
    pub(crate) previous: Option<String>,
    pub(crate) current: Option<String>,
}

pub(crate) fn kept(
    path: &Path,
    keeping: &[Keeping<'_>],
    random: &dyn Random,
    journalled: &dyn Fn(&[Replacing]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if let Some(spanning) = keeping.iter().find(|one| !is_one_line(one.value)) {
        return Err(spans(path, spanning.name));
    }
    keyed_at(path, random)?;
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        let replaced: Vec<Replacing> = keeping
            .iter()
            .map(|one| Replacing {
                name: one.name.to_owned(),
                previous: secrets.fingerprint(one.name),
                current: secrets.keyed(one.value),
            })
            .collect();
        journalled(&replaced)?;
        for one in keeping {
            secrets.keep(one.name, one.value, one.from, one.at);
        }
        Ok(())
    })
}

/// A secret about to be filed, with the keyed fingerprint of its value.
pub(crate) type Filing<'a, 'b> = (&'a Keeping<'b>, Option<String>);

pub(crate) fn filed(
    path: &Path,
    keeping: &[Keeping<'_>],
    random: &dyn Random,
    journalled: &dyn Fn(&[Filing<'_, '_>]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    keyed_at(path, random)?;
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        let absent: Vec<Filing<'_, '_>> = keeping
            .iter()
            .filter(|one| secrets.value(one.name).is_none() && is_one_line(one.value))
            .map(|one| (one, secrets.keyed(one.value)))
            .collect();
        journalled(&absent)?;
        for (one, _) in absent {
            secrets.keep(one.name, one.value, one.from, one.at);
        }
        Ok(())
    })
}

pub(crate) fn lapsing(path: &Path, name: &str, value: &str, at: u64) -> Result<(), Failure> {
    if !is_one_line(value) {
        return Err(spans(path, name));
    }
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        secrets.held.insert(
            name.to_owned(),
            Kept {
                value: value.to_owned(),
                from: Source::Operator,
                at,
                previous: None,
            },
        );
        Ok(())
    })
}

fn spans(path: &Path, name: &str) -> Failure {
    Failure::SpansLines {
        path: path.to_path_buf(),
        key: name.to_owned(),
    }
}

pub(crate) fn dropped(path: &Path, names: &[String]) -> Result<(), Failure> {
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        secrets.held.retain(|name, _| !names.contains(name));
        Ok(())
    })
}

pub(crate) fn forgotten(path: &Path, namespace: &str) -> Result<(), Failure> {
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        secrets
            .held
            .retain(|name, _| !super::in_namespace(name, namespace));
        Ok(())
    })
}

pub(crate) fn reheld(
    path: &Path,
    name: &str,
    previous: Option<&str>,
    wrote: Option<&str>,
    filed: &dyn Fn(&str) -> Result<(), Failure>,
) -> Result<Undone, Failure> {
    let mut undone = Undone::Done;
    super::kept::changed(path, &mut |secrets: &mut Secrets| {
        let value = secrets.value(name).map(str::to_owned);
        undone = secrets.reheld(name, previous, wrote);
        if let (Undone::Done, Some(value), None) = (&undone, value, previous) {
            filed(&value)?;
        }
        Ok(())
    })?;
    Ok(undone)
}

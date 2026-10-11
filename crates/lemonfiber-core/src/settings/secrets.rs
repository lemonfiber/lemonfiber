use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::undoing::Undone;
use crate::config::env::is_one_line;
use crate::config::store::Failure;
use crate::credential::fingerprint;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Secrets {
    #[serde(default)]
    held: BTreeMap<String, Kept>,
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
}

impl Source {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Recipe => "recipe",
        }
    }
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_set().entries(self.held.keys()).finish()
    }
}

impl std::fmt::Debug for Kept {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Kept")
            .field("from", &self.from)
            .field("at", &self.at)
            .finish_non_exhaustive()
    }
}

impl Secrets {
    pub(crate) fn at(path: &Path) -> Result<Self, Failure> {
        super::kept::read(path)
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.held.get(name).map(|kept| kept.value.as_str())
    }

    pub(crate) fn fingerprint(&self, name: &str) -> Option<String> {
        self.value(name).map(fingerprint)
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
        match self.held.get_mut(name) {
            Some(kept)
                if kept.previous.as_deref().map(fingerprint).as_deref() == Some(previous) =>
            {
                let earlier = kept.previous.take().unwrap_or_default();
                kept.previous = Some(std::mem::replace(&mut kept.value, earlier));
                Undone::Done
            }
            _ => Undone::Unkept,
        }
    }
}

pub(crate) struct Keeping<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: &'a str,
    pub(crate) from: Source,
    pub(crate) at: u64,
}

pub(crate) fn kept(
    path: &Path,
    keeping: &[Keeping<'_>],
    journalled: impl FnOnce(&[(String, Option<String>)]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if let Some(spanning) = keeping.iter().find(|one| !is_one_line(one.value)) {
        return Err(Failure::SpansLines {
            path: path.to_path_buf(),
            key: spanning.name.to_owned(),
        });
    }
    super::kept::changed(path, |secrets: &mut Secrets| {
        let replaced: Vec<(String, Option<String>)> = keeping
            .iter()
            .map(|one| (one.name.to_owned(), secrets.fingerprint(one.name)))
            .collect();
        journalled(&replaced)?;
        for one in keeping {
            secrets.keep(one.name, one.value, one.from, one.at);
        }
        Ok(())
    })
}

pub(crate) fn filed(
    path: &Path,
    keeping: &[Keeping<'_>],
    journalled: impl FnOnce(&[&Keeping<'_>]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    super::kept::changed(path, |secrets: &mut Secrets| {
        let absent: Vec<&Keeping<'_>> = keeping
            .iter()
            .filter(|one| secrets.value(one.name).is_none() && is_one_line(one.value))
            .collect();
        journalled(&absent)?;
        for one in absent {
            secrets.keep(one.name, one.value, one.from, one.at);
        }
        Ok(())
    })
}

pub(crate) fn forgotten(path: &Path, namespace: &str) -> Result<(), Failure> {
    super::kept::changed(path, |secrets: &mut Secrets| {
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
    filed: impl FnOnce(&str) -> Result<(), Failure>,
) -> Result<Undone, Failure> {
    super::kept::changed(path, |secrets: &mut Secrets| {
        let value = secrets.value(name).map(str::to_owned);
        let undone = secrets.reheld(name, previous, wrote);
        if let (Undone::Done, Some(value), None) = (&undone, value, previous) {
            filed(&value)?;
        }
        Ok(undone)
    })
}
